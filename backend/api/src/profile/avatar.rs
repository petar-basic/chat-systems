use axum::body::Bytes;
use axum::extract::Multipart;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use crate::files::imaging::{self, ImageKind};
use crate::state::AppState;

pub const MAX_AVATAR_BYTES: usize = 10 * 1024 * 1024;
const PREFIX: &str = "avatars/";
const DOWNLOAD_PATH: &str = "/api/files/download/";

pub fn key(user_id: Uuid, workspace_id: Option<Uuid>, extension: &str) -> String {
    let name = Uuid::new_v4();
    match workspace_id {
        Some(workspace_id) => format!("{PREFIX}{user_id}/{workspace_id}/{name}.{extension}"),
        None => format!("{PREFIX}{user_id}/{name}.{extension}"),
    }
}

pub fn owner(key: &str) -> Option<Uuid> {
    let (user_id, _) = key.strip_prefix(PREFIX)?.split_once('/')?;
    user_id.parse().ok()
}

pub fn workspace_of(key: &str) -> Option<Uuid> {
    let (_, rest) = key.strip_prefix(PREFIX)?.split_once('/')?;
    let (workspace_id, _) = rest.split_once('/')?;
    workspace_id.parse().ok()
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

pub struct Shrunk {
    pub body: Vec<u8>,
    pub mime: &'static str,
    pub extension: &'static str,
}

pub async fn shrink(bytes: Bytes) -> AppResult<Shrunk> {
    if bytes.len() > MAX_AVATAR_BYTES {
        return Err(AppError::Validation(format!(
            "An image is at most {} MB",
            MAX_AVATAR_BYTES / 1024 / 1024
        )));
    }
    let kind = imaging::sniff(&bytes)
        .ok_or_else(|| AppError::Validation("An image is a PNG, JPEG, GIF or WebP file".into()))?;
    let processed = tokio::task::spawn_blocking(move || imaging::process(kind, bytes))
        .await
        .map_err(|e| AppError::Internal(format!("Image processing did not finish: {e}")))?
        .map_err(|e| AppError::Validation(format!("That image could not be read: {e}")))?;

    Ok(match processed.thumbnail {
        Some(small) => Shrunk {
            body: small.bytes,
            mime: small.mime,
            extension: small.extension,
        },
        None => Shrunk {
            body: processed.original,
            mime: kind.mime(),
            extension: extension(kind),
        },
    })
}

pub async fn store(
    state: &AppState,
    user_id: Uuid,
    workspace_id: Option<Uuid>,
    bytes: Bytes,
) -> AppResult<String> {
    let image = shrink(bytes).await?;
    let key = key(user_id, workspace_id, image.extension);
    state
        .file_storage
        .upload(&key, image.body, image.mime)
        .await?;
    Ok(key)
}

pub async fn image_field(mut multipart: Multipart) -> AppResult<Bytes> {
    let mut image = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Invalid multipart: {e}")))?
    {
        if field.name() == Some("file") {
            image = Some(
                field
                    .bytes()
                    .await
                    .map_err(|e| AppError::BadRequest(format!("Could not read the image: {e}")))?,
            );
        }
    }
    image.ok_or_else(|| AppError::Validation("Choose an image to upload".into()))
}

pub async fn forget(
    state: &AppState,
    user_id: Uuid,
    workspace_id: Option<Uuid>,
    url: Option<&str>,
) {
    let Some(key) = url.and_then(key_of) else {
        return;
    };
    if owner(key) != Some(user_id) || workspace_of(key) != workspace_id {
        return;
    }
    if let Err(e) = state.file_storage.delete(key).await {
        tracing::warn!("could not delete the old photo {key}: {e}");
    }
}

pub async fn migrate(state: &AppState) {
    let accounts = match state.profile_repo.photos_outside_avatars().await {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!("photo migration could not start: {e}");
            return;
        }
    };
    let mut moved = 0usize;
    for (user_id, url) in accounts {
        let Some(to) = copy_out(state, user_id, None, &url).await else {
            continue;
        };
        let new_url = state.file_storage.public_url(&to);
        match state.profile_repo.move_photo(user_id, &url, &new_url).await {
            Ok(true) => moved += 1,
            outcome => discard(state, &to, user_id, outcome.err()).await,
        }
    }
    if moved > 0 {
        tracing::info!("photo migration moved {moved} account photos");
    }

    let workspaces = match state.profile_repo.workspace_photos_outside_avatars().await {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!("workspace photo migration could not start: {e}");
            return;
        }
    };
    let mut moved = 0usize;
    for (workspace_id, user_id, url) in workspaces {
        let Some(to) = copy_out(state, user_id, Some(workspace_id), &url).await else {
            continue;
        };
        let new_url = state.file_storage.public_url(&to);
        match state
            .profile_repo
            .move_workspace_photo(workspace_id, user_id, &url, &new_url)
            .await
        {
            Ok(true) => moved += 1,
            outcome => discard(state, &to, user_id, outcome.err()).await,
        }
    }
    if moved > 0 {
        tracing::info!("photo migration moved {moved} workspace photos");
    }
}

async fn copy_out(
    state: &AppState,
    user_id: Uuid,
    workspace_id: Option<Uuid>,
    url: &str,
) -> Option<String> {
    let from = key_of(url)?;
    let extension = from
        .rsplit_once('.')
        .map(|(_, ext)| ext)
        .filter(|ext| !ext.is_empty() && !ext.contains('/'))
        .unwrap_or("png");
    let to = key(user_id, workspace_id, extension);
    match state.file_storage.copy(from, &to).await {
        Ok(()) => Some(to),
        Err(e) => {
            match workspace_id {
                Some(_) => tracing::warn!("workspace photo of {user_id} was not moved: {e}"),
                None => tracing::warn!("photo of {user_id} was not moved: {e}"),
            }
            None
        }
    }
}

async fn discard(state: &AppState, key: &str, user_id: Uuid, error: Option<sqlx::Error>) {
    if let Some(e) = error {
        tracing::warn!("photo of {user_id} was not moved: {e}");
    }
    let _ = state.file_storage.delete(key).await;
}
