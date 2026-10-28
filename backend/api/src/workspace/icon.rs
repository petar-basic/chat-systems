use axum::body::Bytes;
use uuid::Uuid;

use shared_common::errors::AppResult;

use crate::profile::avatar::{key_of, shrink};
use crate::state::AppState;

const PREFIX: &str = "icons/";

pub fn key(workspace_id: Uuid, extension: &str) -> String {
    format!("{PREFIX}{workspace_id}/{}.{extension}", Uuid::new_v4())
}

pub fn workspace_of(key: &str) -> Option<Uuid> {
    let (workspace_id, _) = key.strip_prefix(PREFIX)?.split_once('/')?;
    workspace_id.parse().ok()
}

pub async fn store(state: &AppState, workspace_id: Uuid, bytes: Bytes) -> AppResult<String> {
    let image = shrink(bytes).await?;
    let key = key(workspace_id, image.extension);
    state
        .file_storage
        .upload(&key, image.body, image.mime)
        .await?;
    Ok(key)
}

pub async fn forget(state: &AppState, workspace_id: Uuid, url: Option<&str>) {
    let Some(key) = url.and_then(key_of) else {
        return;
    };
    if workspace_of(key) != Some(workspace_id) {
        return;
    }
    if let Err(e) = state.file_storage.delete(key).await {
        tracing::warn!("could not delete the old icon {key}: {e}");
    }
}

pub async fn migrate(state: &AppState) {
    let icons = match state.workspace_service.repo.icons_outside_icons().await {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!("icon migration could not start: {e}");
            return;
        }
    };
    let mut moved = 0usize;
    for (workspace_id, url) in icons {
        let Some(from) = key_of(&url) else {
            continue;
        };
        let extension = from
            .rsplit_once('.')
            .map(|(_, ext)| ext)
            .filter(|ext| !ext.is_empty() && !ext.contains('/'))
            .unwrap_or("png");
        let to = key(workspace_id, extension);
        if let Err(e) = state.file_storage.copy(from, &to).await {
            tracing::warn!("icon of workspace {workspace_id} was not moved: {e}");
            continue;
        }
        let new_url = state.file_storage.public_url(&to);
        match state
            .workspace_service
            .repo
            .move_icon(workspace_id, &url, &new_url)
            .await
        {
            Ok(true) => moved += 1,
            outcome => {
                if let Err(e) = outcome {
                    tracing::warn!("icon of workspace {workspace_id} was not moved: {e}");
                }
                let _ = state.file_storage.delete(&to).await;
            }
        }
    }
    if moved > 0 {
        tracing::info!("icon migration moved {moved} workspace icons");
    }
}
