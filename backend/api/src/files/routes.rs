use std::sync::Arc;

use axum::body::Body;
use axum::body::Bytes;
use axum::extract::multipart::Field;
use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::Response;
use axum::routing::get;
use axum::{Json, Router};
use utoipa_axum::router::{OpenApiRouter, UtoipaMethodRouterExt};
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::imaging;
use super::models::{
    ChannelFile, ChannelFilesQuery, FileRecord, FileSearchHit, FileSearchQuery, FileUploadResponse,
    UploadQuery,
};
use super::names;
use super::ranges::ByteRange;
use super::repo::{FileSearch, NewFile};
use super::service;
use super::storage::Opening;
use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::authz;
use crate::dto::{DataList, StatusResponse};
use crate::files::models::FileMeta;
use crate::middleware::AuthUser;
use crate::pagination::PageQuery;
use crate::state::AppState;
use crate::workspace::models::WorkspaceRole;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(upload_file).layer(DefaultBodyLimit::disable()))
        .routes(routes!(get_file_meta, delete_file))
        .routes(routes!(list_files))
        .routes(routes!(list_channel_files))
        .routes(routes!(search_files))
}

/// The one route with a wildcard segment, which the OpenAPI document does not
/// describe: it serves bytes, not a shape.
pub fn download_router() -> Router<Arc<AppState>> {
    Router::new().route("/files/download/{*key}", get(download_file))
}

#[utoipa::path(post, path = "/files/upload/{ws_id}", tag = "files", params(UploadQuery), request_body(content = String, content_type = "multipart/form-data", description = "One or more files as multipart fields"), responses((status = 200, body = Vec<FileUploadResponse>)))]
async fn upload_file(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Query(query): Query<UploadQuery>,
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
        let filename = names::display_name(&raw_filename);
        let declared_type = field
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_string();
        let max_bytes = state.config.max_upload_bytes;

        let mut head = Vec::new();
        let mut finished = false;
        while head.len() < SNIFF_BYTES {
            match next_chunk(&mut field).await? {
                Some(chunk) => head.extend_from_slice(&chunk),
                None => {
                    finished = true;
                    break;
                }
            }
        }
        refuse_oversized(head.len(), max_bytes)?;

        let stored = match imaging::sniff(&head) {
            Some(kind) => {
                let mut buffer = head;
                while !finished && buffer.len() <= imaging::MAX_IMAGE_BYTES {
                    match next_chunk(&mut field).await? {
                        Some(chunk) => {
                            buffer.extend_from_slice(&chunk);
                            refuse_oversized(buffer.len(), max_bytes)?;
                        }
                        None => finished = true,
                    }
                }
                if finished {
                    service::store_image(&state, ws_id, &filename, kind, buffer.into()).await?
                } else {
                    stream_rest(
                        &state,
                        ws_id,
                        &filename,
                        kind.mime(),
                        buffer,
                        &mut field,
                        max_bytes,
                    )
                    .await?
                }
            }
            None if finished => {
                let storage_key = service::object_key(ws_id, &filename);
                let size_bytes = head.len() as i64;
                state
                    .file_storage
                    .upload(&storage_key, head, &declared_type)
                    .await?;
                service::StoredObject {
                    storage_key,
                    mime_type: declared_type,
                    size_bytes,
                    thumbnail_key: None,
                    width: None,
                    height: None,
                }
            }
            None => {
                stream_rest(
                    &state,
                    ws_id,
                    &filename,
                    &declared_type,
                    head,
                    &mut field,
                    max_bytes,
                )
                .await?
            }
        };

        let record = state
            .file_repo
            .create(NewFile {
                user_id: auth.user_id,
                workspace_id: ws_id,
                message_id: None,
                filename: &filename,
                storage_key: &stored.storage_key,
                mime_type: &stored.mime_type,
                size_bytes: stored.size_bytes,
                draft_until: query.draft.then(service::draft_deadline),
                thumbnail_key: stored.thumbnail_key.as_deref(),
                width: stored.width,
                height: stored.height,
            })
            .await?;

        let attachment = service::attachment(&state, &record);
        responses.push(FileUploadResponse {
            id: record.id,
            url: attachment.url,
            filename: record.filename,
            mime_type: record.mime_type,
            size_bytes: record.size_bytes,
            thumbnail_url: attachment.thumbnail_url,
            width: record.width,
            height: record.height,
        });
    }

    Ok(Json(responses))
}

const SNIFF_BYTES: usize = 16;

fn refuse_oversized(len: usize, max_bytes: u64) -> AppResult<()> {
    if len as u64 > max_bytes {
        return Err(AppError::BadRequest(format!(
            "File too large (max {max_bytes} bytes)"
        )));
    }
    Ok(())
}

async fn next_chunk(field: &mut Field<'_>) -> AppResult<Option<Bytes>> {
    field
        .chunk()
        .await
        .map_err(|e| AppError::BadRequest(format!("Failed to read file data: {e}")))
}

/// Stream to storage while counting. Reading the field into memory first and
/// checking the size afterwards meant the check never rejected anything the
/// router had already accepted, and a handful of concurrent uploads could take
/// the process out on memory alone.
async fn stream_rest(
    state: &AppState,
    workspace_id: Uuid,
    filename: &str,
    content_type: &str,
    already_read: Vec<u8>,
    field: &mut Field<'_>,
    max_bytes: u64,
) -> AppResult<service::StoredObject> {
    let storage_key = service::object_key(workspace_id, filename);
    let mut sink = state
        .file_storage
        .begin_upload(&storage_key, content_type)
        .await?;
    let mut written = already_read.len() as u64;
    if let Err(e) = sink.write_chunk(Bytes::from(already_read)).await {
        let _ = sink.abort().await;
        return Err(e);
    }
    loop {
        let chunk = match next_chunk(field).await {
            Ok(Some(chunk)) => chunk,
            Ok(None) => break,
            Err(e) => {
                let _ = sink.abort().await;
                return Err(e);
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
    let size_bytes = match sink.finish().await {
        Ok(size) => size as i64,
        Err(e) => {
            let _ = state.file_storage.delete(&storage_key).await;
            return Err(e);
        }
    };
    Ok(service::StoredObject {
        storage_key,
        mime_type: content_type.to_string(),
        size_bytes,
        thumbnail_key: None,
        width: None,
        height: None,
    })
}

async fn download_file(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(key): Path<String>,
    headers: HeaderMap,
) -> AppResult<Response> {
    let filename = if let Some(owner) = crate::profile::avatar::owner(&key) {
        let allowed = owner == auth.user_id
            || match crate::profile::avatar::workspace_of(&key) {
                Some(workspace_id) => state
                    .workspace_service
                    .repo
                    .get_member(workspace_id, auth.user_id)
                    .await?
                    .is_some(),
                None => {
                    state
                        .profile_repo
                        .shares_a_workspace(auth.user_id, owner)
                        .await?
                }
            };
        if !allowed {
            return Err(AppError::Forbidden("Not allowed to see this photo".into()));
        }
        "photo".to_string()
    } else if let Some(workspace_id) = crate::workspace::icon::workspace_of(&key) {
        authz::require_workspace_member(&state, workspace_id, auth.user_id).await?;
        "icon".to_string()
    } else {
        match state.file_repo.find_by_storage_key(&key).await? {
            Some(record) => {
                require_file_access(&state, &record, auth.user_id).await?;
                record.filename
            }
            None => {
                let emoji = state
                    .emoji_repo
                    .find_by_storage_key(&key)
                    .await?
                    .ok_or_else(|| AppError::NotFound("File not found".into()))?;
                authz::require_workspace_member(&state, emoji.workspace_id, auth.user_id).await?;
                emoji.name
            }
        }
    };

    let requested = headers
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok())
        .and_then(ByteRange::parse);
    let opened = match state.file_storage.open(&key, requested).await? {
        Opening::Ready(opened) => opened,
        Opening::Unsatisfiable { total } => {
            return Response::builder()
                .status(StatusCode::RANGE_NOT_SATISFIABLE)
                .header(header::CONTENT_RANGE, format!("bytes */{total}"))
                .header(header::ACCEPT_RANGES, "bytes")
                .body(Body::empty())
                .map_err(|e| AppError::Internal(format!("Response build failed: {e}")));
        }
    };

    let mut response = Response::builder()
        .header(header::CONTENT_TYPE, opened.content_type)
        .header(
            header::CONTENT_LENGTH,
            opened.range.end - opened.range.start,
        )
        .header(header::ACCEPT_RANGES, "bytes")
        .header(
            header::CACHE_CONTROL,
            "private, max-age=31536000, immutable",
        )
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header(
            header::CONTENT_DISPOSITION,
            names::content_disposition(&filename),
        );
    if requested.is_some() {
        response = response.status(StatusCode::PARTIAL_CONTENT).header(
            header::CONTENT_RANGE,
            format!(
                "bytes {}-{}/{}",
                opened.range.start,
                opened.range.end.saturating_sub(1),
                opened.total
            ),
        );
    }
    response
        .body(Body::from_stream(opened.stream))
        .map_err(|e| AppError::Internal(format!("Response build failed: {e}")))
}

#[utoipa::path(get, path = "/files/{file_id}", tag = "files", responses((status = 200, body = FileMeta)))]
async fn get_file_meta(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(file_id): Path<Uuid>,
) -> AppResult<Json<FileMeta>> {
    let record = state
        .file_repo
        .find_by_id(file_id)
        .await?
        .ok_or_else(|| AppError::NotFound("File not found".into()))?;

    require_file_access(&state, &record, auth.user_id).await?;

    let url = state.file_storage.public_url(&record.storage_key);
    Ok(Json(FileMeta { file: record, url }))
}

#[utoipa::path(get, path = "/files/workspace/{ws_id}", tag = "files", params(PageQuery), responses((status = 200, body = DataList<FileRecord>)))]
async fn list_files(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Query(params): Query<PageQuery>,
) -> AppResult<Json<DataList<FileRecord>>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;

    let files = state
        .file_repo
        .list_by_workspace_for_user(ws_id, auth.user_id, params.limit(), params.offset())
        .await?;
    Ok(Json(files.into()))
}

#[utoipa::path(
    operation_id = "files_list_channel_files",
    get, path = "/channels/{ch_id}/files", tag = "files",
    params(ChannelFilesQuery),
    responses((status = 200, body = DataList<ChannelFile>)))]
async fn list_channel_files(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ch_id): Path<Uuid>,
    Query(query): Query<ChannelFilesQuery>,
) -> AppResult<Json<DataList<ChannelFile>>> {
    authz::require_channel_access(&state, ch_id, auth.user_id).await?;
    let records = state
        .file_repo
        .for_channel(
            ch_id,
            query.kind.map(|kind| kind.as_str()),
            query.cursor,
            query.limit.unwrap_or(50).clamp(1, 100),
        )
        .await?;
    let data = records
        .iter()
        .filter_map(|record| {
            Some(ChannelFile {
                attachment: service::attachment(&state, record),
                message_id: record.message_id?,
                user_id: record.user_id,
                created_at: record.created_at,
            })
        })
        .collect();
    Ok(Json(DataList { data }))
}

#[utoipa::path(get, path = "/search/files", tag = "files",
    params(FileSearchQuery),
    responses((status = 200, body = DataList<FileSearchHit>)))]
async fn search_files(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Query(params): Query<FileSearchQuery>,
) -> AppResult<Json<DataList<FileSearchHit>>> {
    let query = params.q.as_deref().unwrap_or_default().trim().to_string();
    if query.is_empty() && !params.has_filter() {
        return Err(AppError::Validation(
            "Search needs words or at least one filter".into(),
        ));
    }

    let member = authz::require_workspace_member(&state, params.workspace_id, auth.user_id).await?;
    if let Some(ch_id) = params.channel_id {
        authz::require_channel_access(&state, ch_id, auth.user_id).await?;
    }

    let matches = state
        .file_repo
        .search(FileSearch {
            query: &query,
            workspace_id: params.workspace_id,
            requester_id: auth.user_id,
            requester_is_guest: member.role == WorkspaceRole::Guest,
            channel_id: params.channel_id,
            uploader_id: params.user_id,
            kind: params.kind,
            after: params.after,
            before: params.before,
            limit: params.limit.unwrap_or(20).clamp(1, 100),
            offset: params.offset.unwrap_or(0).max(0),
        })
        .await?;
    let ids: Vec<Uuid> = matches.iter().map(|found| found.channel_id).collect();
    let names = state.workspace_service.repo.channel_names(&ids).await?;
    let data = matches
        .into_iter()
        .map(|found| FileSearchHit {
            attachment: service::attachment(&state, &found.record),
            kind: found.kind,
            message_id: found.message_id,
            channel_id: found.channel_id,
            channel_name: names.get(&found.channel_id).cloned(),
            user_id: found.record.user_id,
            created_at: found.record.created_at,
            snippet: found.snippet,
        })
        .collect();
    Ok(Json(DataList { data }))
}

#[utoipa::path(delete, path = "/files/{file_id}", tag = "files", responses((status = 200, body = StatusResponse)))]
async fn delete_file(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(file_id): Path<Uuid>,
) -> AppResult<Json<StatusResponse>> {
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
    require_open_channel(&state, &record).await?;

    service::purge_objects(&state, std::slice::from_ref(&record)).await;

    state.file_repo.delete(file_id).await?;

    if let Some(message_id) = record.message_id {
        detach_from_message(&state, message_id, file_id).await?;
    }

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

    Ok(Json(StatusResponse::new("deleted")))
}

async fn require_open_channel(state: &AppState, record: &FileRecord) -> AppResult<()> {
    if let Some(message_id) = record.message_id {
        if let Some(channel_id) = state.file_repo.channel_id_for_message(message_id).await? {
            authz::require_not_archived(&authz::find_channel(state, channel_id).await?)?;
        }
    }
    Ok(())
}

async fn detach_from_message(state: &AppState, message_id: Uuid, file_id: Uuid) -> AppResult<()> {
    let Some(message) = state.message_repo.find_by_id(message_id).await? else {
        return Ok(());
    };
    let remaining: Vec<_> = super::service::attachments_in(&message.metadata)
        .into_iter()
        .filter(|a| a.id != file_id)
        .collect();
    let channel = authz::find_channel(state, message.channel_id).await?;
    let attachments =
        serde_json::to_value(&remaining).map_err(|e| AppError::Internal(e.to_string()))?;
    let mut tx = state.pool.begin().await?;
    let message = state
        .message_repo
        .set_attachments_in(&mut tx, message_id, &attachments)
        .await?;
    let staged = state
        .publisher
        .stage_message_updated(&mut tx, &message, channel.workspace_id)
        .await?;
    tx.commit().await?;
    state.publisher.dispatch(staged).await;
    Ok(())
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

    // Nothing owns it: a draft nobody posted. A draft belongs to whoever wrote
    // it — workspace membership alone is not a reason to read somebody else's file.
    if record.user_id == user_id {
        authz::require_workspace_member(state, record.workspace_id, user_id).await?;
        return Ok(());
    }
    Err(AppError::Forbidden("Not allowed to read this file".into()))
}
