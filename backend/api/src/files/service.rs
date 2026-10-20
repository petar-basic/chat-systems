use uuid::Uuid;

use crate::state::AppState;

const DOWNLOAD_MARKER: &str = "/api/files/download/";

pub fn extract_file_keys(content: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let mut rest = content;
    while let Some(pos) = rest.find(DOWNLOAD_MARKER) {
        rest = &rest[pos + DOWNLOAD_MARKER.len()..];
        let end = rest.find([')', ']', ' ', '"', '\n']).unwrap_or(rest.len());
        let key = &rest[..end];
        if !key.is_empty() {
            keys.push(key.to_string());
        }
        rest = &rest[end..];
    }
    keys
}

pub async fn link_to_channel_message(
    state: &AppState,
    content: &str,
    message_id: Uuid,
    workspace_id: Uuid,
    user_id: Uuid,
) {
    let keys = extract_file_keys(content);
    if keys.is_empty() {
        return;
    }
    if let Err(e) = state
        .file_repo
        .link_to_channel_message(&keys, message_id, workspace_id, user_id)
        .await
    {
        tracing::warn!(
            "failed to link attachments to message {}: {}",
            message_id,
            e
        );
    }
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

/// An edit that removes the link to an attachment removes the attachment. The
/// alternative leaves a file nobody can find but anybody with the key can read.
pub async fn release_unlinked_from_channel_message(
    state: &AppState,
    content: &str,
    message_id: Uuid,
) {
    let kept = extract_file_keys(content);
    match state
        .file_repo
        .delete_unlinked_from_channel_message(message_id, &kept)
        .await
    {
        Ok(records) => purge_objects(state, &records).await,
        Err(e) => tracing::warn!(
            "failed to release attachments of message {}: {}",
            message_id,
            e
        ),
    }
}

async fn purge_objects(state: &AppState, records: &[crate::files::models::FileRecord]) {
    for record in records {
        if let Err(e) = state.file_storage.delete(&record.storage_key).await {
            tracing::warn!("orphaned object {}: {}", record.storage_key, e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pulls_every_key_out_of_a_message_body() {
        let content =
            "see [a](/api/files/download/ws/id/a.png) and /api/files/download/ws/id/b.pdf done";
        assert_eq!(
            extract_file_keys(content),
            vec!["ws/id/a.png".to_string(), "ws/id/b.pdf".to_string()]
        );
    }

    #[test]
    fn finds_nothing_in_a_plain_message() {
        assert!(extract_file_keys("just talking about /api/files").is_empty());
    }
}
