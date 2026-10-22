use chrono::Utc;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

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
    }
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
}

pub async fn copy_objects(
    state: &AppState,
    sources: Vec<FileRecord>,
    workspace_id: Uuid,
) -> AppResult<Vec<CopiedObject>> {
    let mut copied: Vec<CopiedObject> = Vec::with_capacity(sources.len());
    for source in sources {
        let name = source
            .storage_key
            .rsplit('/')
            .next()
            .unwrap_or("file")
            .to_string();
        let storage_key = format!("{workspace_id}/{}/{name}", Uuid::new_v4());
        if let Err(e) = state
            .file_storage
            .copy(&source.storage_key, &storage_key)
            .await
        {
            discard_copies(state, &copied).await;
            return Err(e);
        }
        copied.push(CopiedObject {
            source,
            storage_key,
        });
    }
    Ok(copied)
}

pub async fn discard_copies(state: &AppState, copied: &[CopiedObject]) {
    for copy in copied {
        if let Err(e) = state.file_storage.delete(&copy.storage_key).await {
            tracing::warn!("orphaned copy {}: {}", copy.storage_key, e);
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
        if let Err(e) = state.file_storage.delete(&record.storage_key).await {
            tracing::warn!("orphaned object {}: {}", record.storage_key, e);
        }
    }
}
