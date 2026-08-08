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

pub async fn link_to_conversation_message(
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
        .link_to_conversation_message(&keys, message_id, workspace_id, user_id)
        .await
    {
        tracing::warn!(
            "failed to link attachments to conversation message {}: {}",
            message_id,
            e
        );
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
