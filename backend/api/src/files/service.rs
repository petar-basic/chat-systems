use chrono::Utc;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::imaging::{self, ImageKind};
use super::models::{Attachment, FileRecord};
use super::repo::NewFile;
use crate::state::AppState;

pub const MAX_ATTACHMENTS: usize = 10;
const DRAFT_SWEEP_BATCH: i64 = 200;
const DRAFT_SWEEP_INTERVAL: std::time::Duration = std::time::Duration::from_secs(15 * 60);

pub fn attachment(state: &AppState, record: &FileRecord) -> Attachment {
    Attachment {
        id: record.id,
        filename: record.filename.clone(),
        mime_type: record.mime_type.clone(),
        size_bytes: record.size_bytes,
        url: state.file_storage.public_url(&record.storage_key),
        thumbnail_url: record
            .thumbnail_key
            .as_deref()
            .map(|key| state.file_storage.public_url(key)),
        width: record.width,
        height: record.height,
    }
}

pub struct StoredObject {
    pub storage_key: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub thumbnail_key: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
}

pub fn object_key(workspace_id: Uuid, filename: &str) -> String {
    format!("{workspace_id}/{}/{filename}", Uuid::new_v4())
}

pub async fn store_image(
    state: &AppState,
    workspace_id: Uuid,
    filename: &str,
    kind: ImageKind,
    bytes: axum::body::Bytes,
) -> AppResult<StoredObject> {
    let storage_key = object_key(workspace_id, filename);
    let input = bytes.clone();
    let processed = tokio::task::spawn_blocking(move || imaging::process(kind, input)).await;
    let processed = match processed {
        Ok(Ok(processed)) => processed,
        Ok(Err(e)) => {
            tracing::info!("storing {filename} without processing: {e}");
            return store_unprocessed(state, storage_key, kind, bytes).await;
        }
        Err(e) => {
            tracing::warn!("image processing for {filename} did not finish: {e}");
            return store_unprocessed(state, storage_key, kind, bytes).await;
        }
    };

    let size_bytes = processed.original.len() as i64;
    state
        .file_storage
        .upload(&storage_key, processed.original, kind.mime())
        .await?;
    let thumbnail_key = match processed.thumbnail {
        Some(thumbnail) => {
            let key = object_key(workspace_id, &format!("thumbnail.{}", thumbnail.extension));
            if let Err(e) = state
                .file_storage
                .upload(&key, thumbnail.bytes, thumbnail.mime)
                .await
            {
                let _ = state.file_storage.delete(&storage_key).await;
                return Err(e);
            }
            Some(key)
        }
        None => None,
    };
    Ok(StoredObject {
        storage_key,
        mime_type: kind.mime().to_string(),
        size_bytes,
        thumbnail_key,
        width: i32::try_from(processed.width).ok(),
        height: i32::try_from(processed.height).ok(),
    })
}

async fn store_unprocessed(
    state: &AppState,
    storage_key: String,
    kind: ImageKind,
    bytes: axum::body::Bytes,
) -> AppResult<StoredObject> {
    let size_bytes = bytes.len() as i64;
    state
        .file_storage
        .upload(&storage_key, bytes.to_vec(), kind.mime())
        .await?;
    Ok(StoredObject {
        storage_key,
        mime_type: kind.mime().to_string(),
        size_bytes,
        thumbnail_key: None,
        width: None,
        height: None,
    })
}

pub fn attachments_json(state: &AppState, records: &[FileRecord]) -> AppResult<serde_json::Value> {
    let attachments: Vec<Attachment> = records.iter().map(|r| attachment(state, r)).collect();
    serde_json::to_value(attachments).map_err(|e| AppError::Internal(e.to_string()))
}

pub fn attachments_in(metadata: &serde_json::Value) -> Vec<Attachment> {
    metadata
        .get("attachments")
        .and_then(|value| serde_json::from_value(value.clone()).ok())
        .unwrap_or_default()
}

pub fn check_attachment_ids(ids: &[Uuid]) -> AppResult<()> {
    if ids.len() > MAX_ATTACHMENTS {
        return Err(AppError::Validation(format!(
            "A message can carry at most {MAX_ATTACHMENTS} files"
        )));
    }
    let mut seen = std::collections::HashSet::new();
    if !ids.iter().all(|id| seen.insert(*id)) {
        return Err(AppError::Validation(
            "The same file is attached twice".into(),
        ));
    }
    Ok(())
}

pub async fn claim_drafts_in(
    state: &AppState,
    conn: &mut sqlx::PgConnection,
    ids: &[Uuid],
    message_id: Uuid,
    workspace_id: Uuid,
    user_id: Uuid,
) -> AppResult<Vec<FileRecord>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut claimed = state
        .file_repo
        .claim_drafts_in(conn, ids, message_id, workspace_id, user_id)
        .await?;
    if claimed.len() != ids.len() {
        return Err(AppError::Validation(
            "Attach only files you uploaded here and have not sent yet".into(),
        ));
    }
    claimed.sort_by_key(|record| ids.iter().position(|id| *id == record.id));
    Ok(claimed)
}

pub struct CopiedObject {
    pub source: FileRecord,
    pub storage_key: String,
    pub thumbnail_key: Option<String>,
}

pub async fn copy_objects(
    state: &AppState,
    sources: Vec<FileRecord>,
    workspace_id: Uuid,
) -> AppResult<Vec<CopiedObject>> {
    let mut copied: Vec<CopiedObject> = Vec::with_capacity(sources.len());
    for source in sources {
        match copy_one(state, &source, workspace_id).await {
            Ok((storage_key, thumbnail_key)) => copied.push(CopiedObject {
                source,
                storage_key,
                thumbnail_key,
            }),
            Err(e) => {
                discard_copies(state, &copied).await;
                return Err(e);
            }
        }
    }
    Ok(copied)
}

fn last_segment(key: &str) -> &str {
    key.rsplit('/').next().unwrap_or("file")
}

async fn copy_one(
    state: &AppState,
    source: &FileRecord,
    workspace_id: Uuid,
) -> AppResult<(String, Option<String>)> {
    let storage_key = object_key(workspace_id, last_segment(&source.storage_key));
    state
        .file_storage
        .copy(&source.storage_key, &storage_key)
        .await?;
    let Some(thumbnail) = source.thumbnail_key.as_deref() else {
        return Ok((storage_key, None));
    };
    let thumbnail_key = object_key(workspace_id, last_segment(thumbnail));
    if let Err(e) = state.file_storage.copy(thumbnail, &thumbnail_key).await {
        let _ = state.file_storage.delete(&storage_key).await;
        return Err(e);
    }
    Ok((storage_key, Some(thumbnail_key)))
}

pub async fn discard_copies(state: &AppState, copied: &[CopiedObject]) {
    for copy in copied {
        for key in std::iter::once(&copy.storage_key).chain(copy.thumbnail_key.iter()) {
            if let Err(e) = state.file_storage.delete(key).await {
                tracing::warn!("orphaned copy {}: {}", key, e);
            }
        }
    }
}

pub async fn record_copies_in(
    state: &AppState,
    conn: &mut sqlx::PgConnection,
    copied: &[CopiedObject],
    message_id: Uuid,
    workspace_id: Uuid,
    user_id: Uuid,
) -> AppResult<Vec<FileRecord>> {
    let mut records = Vec::with_capacity(copied.len());
    for copy in copied {
        records.push(
            state
                .file_repo
                .create_in(
                    conn,
                    NewFile {
                        user_id,
                        workspace_id,
                        message_id: Some(message_id),
                        filename: &copy.source.filename,
                        storage_key: &copy.storage_key,
                        mime_type: &copy.source.mime_type,
                        size_bytes: copy.source.size_bytes,
                        draft_until: None,
                        thumbnail_key: copy.thumbnail_key.as_deref(),
                        width: copy.source.width,
                        height: copy.source.height,
                    },
                )
                .await?,
        );
    }
    Ok(records)
}

/// Deleting the message deletes what was attached to it. An attachment whose
/// message is gone is unreachable through the UI but still downloadable by
/// storage key, so leaving it behind means "deleted" only ever meant "hidden".
pub async fn delete_for_channel_message(state: &AppState, message_id: Uuid) {
    match state.file_repo.delete_for_channel_message(message_id).await {
        Ok(records) => purge_objects(state, &records).await,
        Err(e) => tracing::warn!(
            "failed to drop attachments of message {}: {}",
            message_id,
            e
        ),
    }
}

pub async fn sweep_expired_drafts(state: &AppState) -> usize {
    let mut swept = 0;
    loop {
        match state
            .file_repo
            .delete_expired_drafts(DRAFT_SWEEP_BATCH)
            .await
        {
            Ok(records) if records.is_empty() => return swept,
            Ok(records) => {
                swept += records.len();
                purge_objects(state, &records).await;
            }
            Err(e) => {
                tracing::warn!("failed to sweep expired drafts: {e}");
                return swept;
            }
        }
    }
}

pub async fn start_draft_sweeper(state: std::sync::Arc<AppState>) {
    loop {
        tokio::time::sleep(DRAFT_SWEEP_INTERVAL).await;
        let swept = sweep_expired_drafts(&state).await;
        if swept > 0 {
            tracing::info!(swept, "Expired draft uploads deleted");
        }
    }
}

pub fn draft_deadline() -> chrono::DateTime<Utc> {
    Utc::now() + chrono::Duration::hours(24)
}

pub async fn purge_objects(state: &AppState, records: &[FileRecord]) {
    for record in records {
        for key in std::iter::once(&record.storage_key).chain(record.thumbnail_key.iter()) {
            if let Err(e) = state.file_storage.delete(key).await {
                tracing::warn!("orphaned object {}: {}", key, e);
            }
        }
    }
}
