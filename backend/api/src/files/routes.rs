use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::header;
use axum::response::Response;
use axum::routing::{delete, get, post};
use axum::{middleware, Json, Router};
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::{FileRecord, FileUploadResponse};
use super::repo::NewFile;
use crate::middleware::{auth_middleware, AuthUser};
use crate::state::AppState;
use crate::workspace::models::ChannelType;

const MAX_FILE_SIZE: usize = 100 * 1024 * 1024;

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
    Router::new()
        .route("/files/upload/:ws_id", post(upload_file))
        .route("/files/download/*key", get(download_file))
        .route("/files/:file_id", get(get_file_meta))
        .route("/files/:file_id", delete(delete_file))
        .route("/files/workspace/:ws_id", get(list_files))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::rate_limit::write_rate_limit,
        ))
        .layer(middleware::from_fn(auth_middleware))
        .with_state(state)
}

async fn upload_file(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    mut multipart: Multipart,
) -> AppResult<Json<Vec<FileUploadResponse>>> {
    require_workspace_member(&state, ws_id, auth.user_id).await?;

    let mut responses = Vec::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Invalid multipart: {}", e)))?
    {
        let raw_filename = field.file_name().unwrap_or("unnamed").to_string();
        let filename = sanitize_filename(&raw_filename);
        let content_type = field
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_string();
        let data = field
            .bytes()
            .await
            .map_err(|e| AppError::BadRequest(format!("Failed to read file data: {}", e)))?;

        if data.len() > MAX_FILE_SIZE {
            return Err(AppError::BadRequest(format!(
                "File too large: {} bytes (max {} bytes)",
                data.len(),
                MAX_FILE_SIZE
            )));
        }

        let size = data.len() as i64;
        let storage_key = format!("{}/{}/{}", ws_id, Uuid::new_v4(), filename);

        state
            .file_storage
            .upload(&storage_key, data.to_vec(), &content_type)
            .await?;

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
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

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
        .await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("File not found".into()))?;

    require_file_access(&state, &record, auth.user_id).await?;

    let (body, content_type) = state.file_storage.download(&key).await?;

    let disposition = format!("attachment; filename=\"{}\"", record.filename);

    let response = Response::builder()
        .header(header::CONTENT_TYPE, content_type)
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header(header::CONTENT_DISPOSITION, disposition)
        .body(Body::from(body))
        .map_err(|e| AppError::Internal(format!("Response build failed: {}", e)))?;

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
        .await
        .map_err(|e| AppError::Database(e.to_string()))?
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
    require_workspace_member(&state, ws_id, auth.user_id).await?;

    let limit = params
        .get("limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(50i64);
    let offset = params
        .get("offset")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0i64);
    let files = state
        .file_repo
        .list_by_workspace_for_user(ws_id, auth.user_id, limit, offset)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(Json(serde_json::json!({ "data": files })))
}

async fn delete_file(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(file_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let record = state
        .file_repo
        .find_by_id(file_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("File not found".into()))?;

    require_workspace_member(&state, record.workspace_id, auth.user_id).await?;

    if record.user_id != auth.user_id {
        return Err(AppError::Forbidden("Can only delete your own files".into()));
    }

    let _ = state.file_storage.delete(&record.storage_key).await;

    state
        .file_repo
        .delete(file_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(Json(serde_json::json!({ "status": "deleted" })))
}

async fn require_workspace_member(
    state: &AppState,
    workspace_id: Uuid,
    user_id: Uuid,
) -> AppResult<()> {
    state
        .workspace_service
        .repo
        .get_member(workspace_id, user_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| AppError::Forbidden("Not a member of this workspace".into()))?;
    Ok(())
}

async fn require_file_access(
    state: &AppState,
    record: &FileRecord,
    user_id: Uuid,
) -> AppResult<()> {
    require_workspace_member(state, record.workspace_id, user_id).await?;

    if let Some(message_id) = record.message_id {
        if let Some(channel_id) = state
            .file_repo
            .channel_id_for_message(message_id)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
        {
            require_channel_membership(state, channel_id, user_id).await?;
        }
    }

    Ok(())
}

async fn require_channel_membership(
    state: &AppState,
    channel_id: Uuid,
    user_id: Uuid,
) -> AppResult<()> {
    let channel = state
        .workspace_service
        .repo
        .find_channel_by_id(channel_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("Channel not found".into()))?;

    if channel.channel_type == ChannelType::Private || channel.channel_type == ChannelType::GroupDm
    {
        state
            .workspace_service
            .repo
            .get_channel_member(channel_id, user_id)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
            .ok_or_else(|| AppError::Forbidden("Not a member of this channel".into()))?;
    }

    Ok(())
}
