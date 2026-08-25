use std::sync::Arc;

use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, State};
use axum::http::header;
use axum::response::Response;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::{FileRecord, FileUploadResponse};
use super::repo::NewFile;
use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::authz;
use crate::middleware::AuthUser;
use crate::state::AppState;
use crate::workspace::models::WorkspaceRole;

fn sanitize_filename(name: &str) -> String {
    let basename = name.rsplit(['/', '\\']).next().unwrap_or("");

    let cleaned: String = basename
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();

    if cleaned.is_empty() || cleaned == "." || cleaned == ".." || cleaned.contains("..") {
        "file".to_string()
    } else {
        cleaned
    }
}

pub fn router(state: Arc<AppState>) -> Router {
    let routes = Router::new()
        .route(
            "/files/upload/{ws_id}",
            // The generous body limit belongs here and nowhere else.
            post(upload_file).layer(DefaultBodyLimit::disable()),
        )
        .route("/files/download/{*key}", get(download_file))
        .route("/files/{file_id}", get(get_file_meta))
        .route("/files/{file_id}", delete(delete_file))
        .route("/files/workspace/{ws_id}", get(list_files));

    crate::protected(state, routes)
}

async fn upload_file(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    mut multipart: Multipart,
) -> AppResult<Json<Vec<FileUploadResponse>>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;

    let mut responses = Vec::new();

    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Invalid multipart: {e}")))?
    {
        let raw_filename = field.file_name().unwrap_or("unnamed").to_string();
        let filename = sanitize_filename(&raw_filename);
        let content_type = field
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_string();
        let storage_key = format!("{}/{}/{}", ws_id, Uuid::new_v4(), filename);
        let max_bytes = state.config.max_upload_bytes;

        // Stream to storage while counting. Reading the field into memory first
        // and checking the size afterwards meant the check never rejected
        // anything the router had already accepted, and a handful of concurrent
        // uploads could take the process out on memory alone.
        let mut sink = state
            .file_storage
            .begin_upload(&storage_key, &content_type)
            .await?;
        let mut written: u64 = 0;
        loop {
            let chunk = match field.chunk().await {
                Ok(Some(chunk)) => chunk,
                Ok(None) => break,
                Err(e) => {
                    let _ = sink.abort().await;
                    return Err(AppError::BadRequest(format!(
                        "Failed to read file data: {e}"
                    )));
                }
            };

            written += chunk.len() as u64;
            if written > max_bytes {
                let _ = sink.abort().await;
                return Err(AppError::BadRequest(format!(
                    "File too large (max {max_bytes} bytes)"
                )));
            }

            if let Err(e) = sink.write_chunk(chunk).await {
                let _ = sink.abort().await;
                return Err(e);
            }
        }

        let size = match sink.finish().await {
            Ok(size) => size as i64,
            Err(e) => {
                let _ = state.file_storage.delete(&storage_key).await;
                return Err(e);
            }
        };

        let record = state
            .file_repo
            .create(NewFile {
                user_id: auth.user_id,
                workspace_id: ws_id,
                message_id: None,
                filename: &filename,
                storage_key: &storage_key,
                mime_type: &content_type,
                size_bytes: size,
            })
            .await?;

        let url = state.file_storage.public_url(&storage_key);

        responses.push(FileUploadResponse {
            id: record.id,
            url,
            filename: record.filename,
            mime_type: record.mime_type,
            size_bytes: record.size_bytes,
        });
    }

    Ok(Json(responses))
}

async fn download_file(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(key): Path<String>,
) -> AppResult<Response> {
    let record = state
        .file_repo
        .find_by_storage_key(&key)
        .await?
        .ok_or_else(|| AppError::NotFound("File not found".into()))?;

    require_file_access(&state, &record, auth.user_id).await?;

    let (body, content_type) = state.file_storage.download(&key).await?;

    let disposition = format!("attachment; filename=\"{}\"", record.filename);

    let response = Response::builder()
        .header(header::CONTENT_TYPE, content_type)
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header(header::CONTENT_DISPOSITION, disposition)
        .body(Body::from(body))
        .map_err(|e| AppError::Internal(format!("Response build failed: {e}")))?;

    Ok(response)
}

async fn get_file_meta(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(file_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let record = state
        .file_repo
        .find_by_id(file_id)
        .await?
        .ok_or_else(|| AppError::NotFound("File not found".into()))?;

    require_file_access(&state, &record, auth.user_id).await?;

    let url = state.file_storage.public_url(&record.storage_key);
    Ok(Json(serde_json::json!({
        "file": record,
        "url": url,
    })))
}

async fn list_files(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> AppResult<Json<serde_json::Value>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;

    let limit = params
        .get("limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(50i64)
        .clamp(1, 200);
    let offset = params
        .get("offset")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0i64)
        .max(0);
    let files = state
        .file_repo
        .list_by_workspace_for_user(ws_id, auth.user_id, limit, offset)
        .await?;
    Ok(Json(serde_json::json!({ "data": files })))
}

async fn delete_file(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(file_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let record = state
        .file_repo
        .find_by_id(file_id)
        .await?
        .ok_or_else(|| AppError::NotFound("File not found".into()))?;

    let member = authz::require_workspace_member(&state, record.workspace_id, auth.user_id).await?;
    let moderated = record.user_id != auth.user_id;
    if moderated {
        require_file_moderator(&state, &record, auth.user_id, &member).await?;
    }

    let _ = state.file_storage.delete(&record.storage_key).await;

    state.file_repo.delete(file_id).await?;

    audit::record(
        &state,
        AuditEntry::new(AuditAction::FileDeleted, auth.user_id)
            .workspace(record.workspace_id)
            .resource(file_id)
            .ip(&ip)
            .details(serde_json::json!({
                "filename": record.filename,
                "uploader_id": record.user_id,
                "moderated": moderated,
            })),
    )
    .await;

    Ok(Json(serde_json::json!({ "status": "deleted" })))
}

/// Somebody else's file. A workspace admin answers for everything posted in the
/// workspace, and a channel moderator for everything posted in their channel —
/// so both can take a file down without waiting for the uploader.
async fn require_file_moderator(
    state: &AppState,
    record: &FileRecord,
    user_id: Uuid,
    member: &crate::workspace::models::WorkspaceMember,
) -> AppResult<()> {
    if member.role.has_at_least(&WorkspaceRole::Admin) {
        return Ok(());
    }

    if let Some(message_id) = record.message_id {
        if let Some(channel_id) = state.file_repo.channel_id_for_message(message_id).await? {
            let channel = authz::find_channel(state, channel_id).await?;
            return authz::require_channel_moderator(state, &channel, user_id).await;
        }
    }

    Err(AppError::Forbidden(
        "Requires the uploader, a channel admin or a workspace admin".into(),
    ))
}

async fn require_file_access(
    state: &AppState,
    record: &FileRecord,
    user_id: Uuid,
) -> AppResult<()> {
    if let Some(message_id) = record.message_id {
        if let Some(channel_id) = state.file_repo.channel_id_for_message(message_id).await? {
            authz::require_channel_access(state, channel_id, user_id).await?;
            return Ok(());
        }
    }

    if let Some(message_id) = record.conversation_message_id {
        if let Some(conversation_id) = state
            .file_repo
            .conversation_id_for_message(message_id)
            .await?
        {
            authz::require_conversation_participant(state, conversation_id, user_id).await?;
            return Ok(());
        }
    }

    // Nothing owns it: a draft nobody posted, or an avatar. A draft belongs to
    // whoever wrote it — workspace membership alone is not a reason to read
    // somebody else's file. An avatar is the exception: it exists to be shown.
    if record.user_id == user_id || state.file_repo.is_avatar(&record.storage_key).await? {
        authz::require_workspace_member(state, record.workspace_id, user_id).await?;
        return Ok(());
    }
    Err(AppError::Forbidden("Not allowed to read this file".into()))
}
