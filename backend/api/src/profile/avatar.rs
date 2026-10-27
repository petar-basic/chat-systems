use axum::body::Bytes;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use crate::files::imaging::{self, ImageKind};
use crate::state::AppState;

pub const MAX_AVATAR_BYTES: usize = 10 * 1024 * 1024;
const PREFIX: &str = "avatars/";
const DOWNLOAD_PATH: &str = "/api/files/download/";

pub fn key(user_id: Uuid, extension: &str) -> String {
    format!("{PREFIX}{user_id}/{}.{extension}", Uuid::new_v4())
}

pub fn owner(key: &str) -> Option<Uuid> {
    let (user_id, _) = key.strip_prefix(PREFIX)?.split_once('/')?;
    user_id.parse().ok()
}

pub fn key_of(url: &str) -> Option<&str> {
    url.find(DOWNLOAD_PATH)
        .map(|at| &url[at + DOWNLOAD_PATH.len()..])
        .filter(|key| !key.is_empty())
}

fn extension(kind: ImageKind) -> &'static str {
    match kind {
        ImageKind::Png => "png",
        ImageKind::Jpeg => "jpg",
        ImageKind::Gif => "gif",
        ImageKind::Webp => "webp",
    }
}

pub async fn store(state: &AppState, user_id: Uuid, bytes: Bytes) -> AppResult<String> {
    if bytes.len() > MAX_AVATAR_BYTES {
        return Err(AppError::Validation(format!(
            "A photo is at most {} MB",
            MAX_AVATAR_BYTES / 1024 / 1024
        )));
    }
    let kind = imaging::sniff(&bytes)
        .ok_or_else(|| AppError::Validation("A photo is a PNG, JPEG, GIF or WebP image".into()))?;
    let processed = tokio::task::spawn_blocking(move || imaging::process(kind, bytes))
        .await
        .map_err(|e| AppError::Internal(format!("Photo processing did not finish: {e}")))?
        .map_err(|e| AppError::Validation(format!("That photo could not be read: {e}")))?;

    let (body, mime, extension) = match processed.thumbnail {
        Some(small) => (small.bytes, small.mime, small.extension),
        None => (processed.original, kind.mime(), extension(kind)),
    };
    let key = key(user_id, extension);
    state.file_storage.upload(&key, body, mime).await?;
    Ok(key)
}

pub async fn forget(state: &AppState, user_id: Uuid, url: Option<&str>) {
    let Some(key) = url.and_then(key_of) else {
        return;
    };
    if owner(key) != Some(user_id) {
        return;
    }
    if let Err(e) = state.file_storage.delete(key).await {
        tracing::warn!("could not delete the old photo {key}: {e}");
    }
}

pub async fn migrate(state: &AppState) {
    let pending = match state.profile_repo.photos_outside_avatars().await {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!("photo migration could not start: {e}");
            return;
        }
    };

    let mut moved = 0usize;
    for (user_id, url) in pending {
        let Some(from) = key_of(&url) else {
            continue;
        };
        let extension = from
            .rsplit_once('.')
            .map(|(_, ext)| ext)
            .filter(|ext| !ext.is_empty() && !ext.contains('/'))
            .unwrap_or("png");
        let to = key(user_id, extension);
        if let Err(e) = state.file_storage.copy(from, &to).await {
            tracing::warn!("photo of {user_id} was not moved: {e}");
            continue;
        }
        let new_url = state.file_storage.public_url(&to);
        match state.profile_repo.move_photo(user_id, &url, &new_url).await {
            Ok(true) => moved += 1,
            Ok(false) => {
                let _ = state.file_storage.delete(&to).await;
            }
            Err(e) => {
                tracing::warn!("photo of {user_id} was not moved: {e}");
                let _ = state.file_storage.delete(&to).await;
            }
        }
    }

    if moved > 0 {
        tracing::info!("photo migration moved {moved} account photos");
    }
}
