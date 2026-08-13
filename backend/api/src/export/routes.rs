use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::header;
use axum::response::Response;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use rand::RngCore;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::repo::{CreateExportRequest, ExportScope, NewExport};
use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::authz;
use crate::middleware::AuthUser;
use crate::state::AppState;
use crate::workspace::models::WorkspaceRole;

pub fn generate_download_token() -> String {
    use base64::Engine;
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

pub fn router(state: Arc<AppState>) -> Router {
    let protected = Router::new()
        .route("/workspaces/:ws_id/exports", post(create_workspace_export))
        .route("/users/:user_id/exports", post(create_user_export))
        .route("/exports/:id", get(get_export))
        .route("/admin/users/:user_id/data", delete(erase_user_data));

    // The download carries its own single-use credential, so it does not sit
    // behind a session: the recipient of an export is often not the requester.
    let public = Router::new().route("/exports/download/:token", get(download));

    Router::new()
        .merge(crate::protected(state.clone(), protected))
        .merge(public.with_state(state))
}

/// The most sensitive operation in the product: everything anyone said in a
/// workspace, in one file. Owner only.
async fn create_workspace_export(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<CreateExportRequest>,
) -> AppResult<Json<serde_json::Value>> {
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Owner).await?;

    let job = state
        .export_repo
        .create(NewExport {
            scope: ExportScope::Workspace,
            workspace_id: Some(ws_id),
            subject_user_id: None,
            requested_by: auth.user_id,
            include_dms: req.include_dms,
            since: req.since,
            until: req.until,
        })
        .await?;

    audit::record(
        &state,
        AuditEntry::new(AuditAction::ExportRequested, auth.user_id)
            .workspace(ws_id)
            .resource(job.id)
            .ip(&ip)
            .details(serde_json::json!({
                "scope": "workspace",
                // Recorded whichever way it went: opting into other people's
                // private conversations is the part somebody will be asked about.
                "include_dms": req.include_dms,
                "since": req.since,
                "until": req.until,
            })),
    )
    .await;

    Ok(Json(serde_json::json!({ "export": job })))
}

async fn create_user_export(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(user_id): Path<Uuid>,
    Json(req): Json<CreateExportRequest>,
) -> AppResult<Json<serde_json::Value>> {
    // Your own data, or an instance admin acting on a subject access request.
    if user_id != auth.user_id && !auth.is_instance_admin {
        return Err(AppError::Forbidden(
            "Only an instance admin can export somebody else's data".into(),
        ));
    }

    let job = state
        .export_repo
        .create(NewExport {
            scope: ExportScope::User,
            workspace_id: None,
            subject_user_id: Some(user_id),
            requested_by: auth.user_id,
            include_dms: true,
            since: req.since,
            until: req.until,
        })
        .await?;

    audit::record(
        &state,
        AuditEntry::new(AuditAction::ExportRequested, auth.user_id)
            .resource(job.id)
            .ip(&ip)
            .details(serde_json::json!({ "scope": "user", "subject_user_id": user_id })),
    )
    .await;

    Ok(Json(serde_json::json!({ "export": job })))
}

async fn get_export(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let job = state
        .export_repo
        .find(id)
        .await?
        .ok_or_else(|| AppError::NotFound("Export not found".into()))?;

    if job.requested_by != auth.user_id && !auth.is_instance_admin {
        return Err(AppError::Forbidden("Not your export".into()));
    }

    let download_url = job
        .download_token
        .as_ref()
        .map(|token| format!("{}/api/exports/download/{}", state.config.public_url, token));

    Ok(Json(
        serde_json::json!({ "export": job, "download_url": download_url }),
    ))
}

async fn download(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
) -> AppResult<Response> {
    let job = state
        .export_repo
        .claim_download(&token)
        .await?
        .ok_or_else(|| AppError::NotFound("This link has expired or was already used".into()))?;

    let key = job
        .storage_key
        .ok_or_else(|| AppError::Internal("Export has no archive".into()))?;
    let (body, _) = state.file_storage.download(&key).await?;

    Response::builder()
        .header(header::CONTENT_TYPE, "application/x-tar")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"export-{}.tar\"", job.id),
        )
        .body(Body::from(body))
        .map_err(|e| AppError::Internal(format!("Response build failed: {e}")))
}

#[derive(serde::Deserialize)]
struct EraseRequest {
    /// Anonymise by default. Hard-deleting one participant's messages makes
    /// every conversation they were in unreadable for everyone else, which is a
    /// destructive answer to a request that rarely asked for it.
    #[serde(default)]
    hard_delete: bool,
}

async fn erase_user_data(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(user_id): Path<Uuid>,
    Json(req): Json<EraseRequest>,
) -> AppResult<Json<serde_json::Value>> {
    if !auth.is_instance_admin {
        return Err(AppError::Forbidden("Requires an instance admin".into()));
    }

    let removed = if req.hard_delete {
        let messages = sqlx::query("DELETE FROM messages WHERE user_id = $1")
            .bind(user_id)
            .execute(&state.pool)
            .await?
            .rows_affected();
        let conversation_messages =
            sqlx::query("DELETE FROM conversation_messages WHERE user_id = $1")
                .bind(user_id)
                .execute(&state.pool)
                .await?
                .rows_affected();
        serde_json::json!({ "messages": messages, "conversation_messages": conversation_messages })
    } else {
        serde_json::json!({ "messages": 0, "conversation_messages": 0 })
    };

    // The account is tombstoned either way: the profile is what identifies a
    // person, and it goes even when their messages stay readable.
    sqlx::query(
        r"
        UPDATE users
           SET email = 'deleted-' || id || '@invalid',
               display_name = 'Deleted user',
               avatar_url = NULL,
               bio = NULL,
               password_hash = NULL,
               status = 'suspended',
               updated_at = NOW()
         WHERE id = $1
        ",
    )
    .bind(user_id)
    .execute(&state.pool)
    .await?;

    crate::sessions::revoke(
        &state,
        user_id,
        crate::sessions::SessionScope::All,
        "account erased",
    )
    .await?;

    audit::record(
        &state,
        AuditEntry::new(AuditAction::UserDataErased, auth.user_id)
            .resource(user_id)
            .ip(&ip)
            .details(serde_json::json!({ "hard_delete": req.hard_delete, "removed": removed })),
    )
    .await;

    Ok(Json(
        serde_json::json!({ "status": "erased", "hard_delete": req.hard_delete, "removed": removed }),
    ))
}
