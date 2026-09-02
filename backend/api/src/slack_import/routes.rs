use std::sync::Arc;

use axum::extract::{DefaultBodyLimit, Multipart, Path, State};
use axum::routing::get;
use axum::{Json, Router};
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::authz;
use crate::middleware::AuthUser;
use crate::state::AppState;
use crate::workspace::models::WorkspaceRole;

/// What fits through a browser, an nginx in front of it, and this process's
/// memory at once. An export past this is what `chat-import` is for, and the
/// error says so rather than failing at the proxy.
const MAX_ARCHIVE_BYTES: usize = 512 * 1024 * 1024;

pub fn router(state: Arc<AppState>) -> Router {
    let routes = Router::new()
        .route(
            "/workspaces/{ws_id}/slack-imports",
            get(list_imports).post(start_import),
        )
        .route("/slack-imports", axum::routing::post(start_import_into_new))
        .route("/slack-imports/{import_id}", get(get_import))
        .layer(DefaultBodyLimit::max(MAX_ARCHIVE_BYTES));

    crate::protected(state, routes)
}

async fn list_imports(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
    let runs = state.slack_import_repo.list_runs(ws_id).await?;
    Ok(Json(serde_json::json!({ "data": runs })))
}

async fn get_import(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(import_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let run = state
        .slack_import_repo
        .find_run(import_id)
        .await?
        .ok_or_else(|| AppError::NotFound("No such import".into()))?;

    authz::require_workspace_role(
        &state,
        run.workspace_id,
        auth.user_id,
        &WorkspaceRole::Admin,
    )
    .await?;

    Ok(Json(serde_json::json!({ "data": run })))
}

/// The export is the whole workspace, so this is the case where there is nothing
/// to import *into* yet. The name cannot come from the archive — Slack does not
/// put it there, not even in the manifest — so the caller sends one, and the app
/// suggests it from the file name.
async fn start_import_into_new(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    multipart: Multipart,
) -> AppResult<Json<serde_json::Value>> {
    let upload = read_upload(multipart).await?;
    let name = upload
        .workspace_name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| AppError::Validation("Name the workspace to import into".into()))?;
    shared_common::validation::validate_workspace_name(name)?;

    let workspace = state
        .workspace_service
        .create_workspace(name, Some("Imported from Slack"), auth.user_id)
        .await?;

    audit::record(
        &state,
        AuditEntry::new(AuditAction::WorkspaceCreated, auth.user_id)
            .workspace(workspace.id)
            .resource(workspace.id)
            .ip(&ip),
    )
    .await;

    queue(&state, workspace.id, auth.user_id, ip, upload).await
}

/// Takes the archive and queues the work. The response is the run, not the
/// result: an import takes minutes to hours, and the client watches the row.
async fn start_import(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(ws_id): Path<Uuid>,
    multipart: Multipart,
) -> AppResult<Json<serde_json::Value>> {
    // An import writes history into every channel and creates accounts. Nothing
    // short of a workspace admin has any business starting one.
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;

    let upload = read_upload(multipart).await?;
    queue(&state, ws_id, auth.user_id, ip, upload).await
}

struct Upload {
    filename: String,
    bytes: Vec<u8>,
    dry_run: bool,
    workspace_name: Option<String>,
}

async fn read_upload(mut multipart: Multipart) -> AppResult<Upload> {
    let mut archive: Option<(String, Vec<u8>)> = None;
    let mut dry_run = false;
    let mut workspace_name = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Invalid multipart: {e}")))?
    {
        match field.name().unwrap_or_default() {
            "dry_run" => {
                let raw = field
                    .text()
                    .await
                    .map_err(|e| AppError::BadRequest(format!("Invalid dry_run field: {e}")))?;
                dry_run = raw == "true" || raw == "1";
            }
            "workspace_name" => {
                workspace_name = Some(field.text().await.map_err(|e| {
                    AppError::BadRequest(format!("Invalid workspace_name field: {e}"))
                })?);
            }
            "archive" => {
                let filename = field
                    .file_name()
                    .unwrap_or("slack-export.zip")
                    .trim()
                    .to_string();
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|e| AppError::BadRequest(format!("Invalid archive: {e}")))?;
                if bytes.len() > MAX_ARCHIVE_BYTES {
                    return Err(AppError::Validation(format!(
                        "That export is over {} MB. Import it with the chat-import CLI instead",
                        MAX_ARCHIVE_BYTES / (1024 * 1024)
                    )));
                }
                archive = Some((filename, bytes.to_vec()));
            }
            _ => {}
        }
    }

    let (filename, bytes) =
        archive.ok_or_else(|| AppError::Validation("Attach the export as `archive`".into()))?;

    // A zip starts with "PK\x03\x04"; anything else is not going to be readable
    // by the worker either, and finding out now is better than in a job.
    if !bytes.starts_with(b"PK\x03\x04") {
        return Err(AppError::Validation(
            "That is not a zip. Slack's export is the .zip it hands you".into(),
        ));
    }

    Ok(Upload {
        filename,
        bytes,
        dry_run,
        workspace_name,
    })
}

async fn queue(
    state: &Arc<AppState>,
    ws_id: Uuid,
    user_id: Uuid,
    ip: ClientIp,
    upload: Upload,
) -> AppResult<Json<serde_json::Value>> {
    let storage_key = format!("slack-imports/{ws_id}/{}.zip", Uuid::new_v4());
    state
        .file_storage
        .upload(&storage_key, upload.bytes, "application/zip")
        .await?;

    let run = state
        .slack_import_repo
        .queue_run(
            ws_id,
            &upload.filename,
            &storage_key,
            user_id,
            upload.dry_run,
        )
        .await?;

    audit::record(
        state,
        AuditEntry::new(AuditAction::SlackImportStarted, user_id)
            .workspace(ws_id)
            .resource(run.id)
            .ip(&ip)
            .details(serde_json::json!({ "source": upload.filename, "dry_run": upload.dry_run })),
    )
    .await;

    Ok(Json(serde_json::json!({ "data": run })))
}
