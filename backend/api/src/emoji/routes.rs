use std::sync::Arc;

use axum::extract::{Multipart, Path, State};
use axum::routing::get;
use axum::{Json, Router};
use uuid::Uuid;

use shared_common::errors::{is_unique_violation, AppError, AppResult};

use super::standard::shadows_a_standard_emoji;
use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::authz;
use crate::middleware::AuthUser;
use crate::state::AppState;
use crate::workspace::models::WorkspaceRole;

/// An emoji is a 128px image in a message list, not a file share. The limit is
/// what keeps somebody from using the picker as free storage.
const MAX_EMOJI_BYTES: u64 = 256 * 1024;
const ALLOWED_TYPES: &[&str] = &["image/png", "image/gif", "image/webp", "image/jpeg"];

pub fn router(state: Arc<AppState>) -> Router {
    let routes = Router::new()
        .route(
            "/workspaces/:ws_id/emojis",
            get(list_emojis).post(upload_emoji),
        )
        .route(
            "/workspaces/:ws_id/emojis/:emoji_id",
            axum::routing::delete(delete_emoji),
        );

    crate::protected(state, routes)
}

pub fn validate_name(name: &str) -> AppResult<String> {
    let name = name.trim().trim_matches(':').to_lowercase();

    if name.len() < 2 || name.len() > 32 {
        return Err(AppError::Validation(
            "An emoji name is 2 to 32 characters".into(),
        ));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
    {
        return Err(AppError::Validation(
            "An emoji name uses lowercase letters, digits, underscores and dashes".into(),
        ));
    }
    if shadows_a_standard_emoji(&name) {
        return Err(AppError::Validation(format!(
            ":{name}: is a standard emoji on this instance"
        )));
    }

    Ok(name)
}

async fn list_emojis(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;

    let emojis = state.emoji_repo.list(ws_id).await?;
    Ok(Json(serde_json::json!({
        "data": emojis
            .iter()
            .map(|e| serde_json::json!({
                "id": e.id,
                "name": e.name,
                "url": state.file_storage.public_url(&e.storage_key),
                "created_by": e.created_by,
                "created_at": e.created_at,
            }))
            .collect::<Vec<_>>(),
    })))
}

async fn upload_emoji(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(ws_id): Path<Uuid>,
    mut multipart: Multipart,
) -> AppResult<Json<serde_json::Value>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;

    let mut name: Option<String> = None;
    let mut image: Option<(Vec<u8>, String)> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Invalid multipart: {e}")))?
    {
        match field.name().unwrap_or_default() {
            "name" => {
                let raw = field
                    .text()
                    .await
                    .map_err(|e| AppError::BadRequest(format!("Invalid name field: {e}")))?;
                name = Some(validate_name(&raw)?);
            }
            "image" => {
                let content_type = field
                    .content_type()
                    .unwrap_or("application/octet-stream")
                    .to_string();
                if !ALLOWED_TYPES.contains(&content_type.as_str()) {
                    return Err(AppError::Validation(
                        "An emoji is a PNG, GIF, WebP or JPEG image".into(),
                    ));
                }
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|e| AppError::BadRequest(format!("Could not read the image: {e}")))?;
                if bytes.len() as u64 > MAX_EMOJI_BYTES {
                    return Err(AppError::Validation(format!(
                        "An emoji is at most {} KB",
                        MAX_EMOJI_BYTES / 1024
                    )));
                }
                image = Some((bytes.to_vec(), content_type));
            }
            _ => {}
        }
    }

    let name = name.ok_or_else(|| AppError::Validation("An emoji needs a name".into()))?;
    let (bytes, content_type) =
        image.ok_or_else(|| AppError::Validation("An emoji needs an image".into()))?;

    let storage_key = format!("emoji/{ws_id}/{}", Uuid::new_v4());
    state
        .file_storage
        .upload(&storage_key, bytes, &content_type)
        .await?;

    let emoji = match state
        .emoji_repo
        .create(ws_id, &name, &storage_key, auth.user_id)
        .await
    {
        Ok(emoji) => emoji,
        Err(e) => {
            // The object is orphaned otherwise: the row is what anything ever
            // looks the image up by.
            let _ = state.file_storage.delete(&storage_key).await;
            if is_unique_violation(&e) {
                return Err(AppError::Conflict(format!(
                    ":{name}: already exists in this workspace"
                )));
            }
            return Err(e.into());
        }
    };

    audit::record(
        &state,
        AuditEntry::new(AuditAction::EmojiCreated, auth.user_id)
            .workspace(ws_id)
            .resource(emoji.id)
            .ip(&ip)
            .details(serde_json::json!({ "name": name })),
    )
    .await;

    Ok(Json(serde_json::json!({
        "id": emoji.id,
        "name": emoji.name,
        "url": state.file_storage.public_url(&emoji.storage_key),
    })))
}

/// Whoever added it, or a workspace admin. An emoji is shared vocabulary, so
/// removing one is a workspace decision rather than a personal one.
async fn delete_emoji(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path((ws_id, emoji_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;

    let emoji = state
        .emoji_repo
        .find(emoji_id)
        .await?
        .filter(|e| e.workspace_id == ws_id)
        .ok_or_else(|| AppError::NotFound("No such emoji".into()))?;

    if emoji.created_by != auth.user_id {
        authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
    }

    state.emoji_repo.delete(emoji.id).await?;
    let _ = state.file_storage.delete(&emoji.storage_key).await;

    audit::record(
        &state,
        AuditEntry::new(AuditAction::EmojiDeleted, auth.user_id)
            .workspace(ws_id)
            .resource(emoji.id)
            .ip(&ip)
            .details(serde_json::json!({ "name": emoji.name })),
    )
    .await;

    Ok(Json(serde_json::json!({ "status": "deleted" })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_normalized_the_way_people_type_it() {
        assert_eq!(validate_name(":ShipIt:").expect("valid"), "shipit");
        assert_eq!(
            validate_name("  party_parrot  ").expect("valid"),
            "party_parrot"
        );
    }

    #[test]
    fn a_custom_emoji_cannot_shadow_one_the_composer_already_resolves() {
        assert!(validate_name("smile").is_err());
        assert!(validate_name(":joy:").is_err());
        assert!(validate_name("shipit").is_ok());
    }

    #[test]
    fn a_name_that_would_never_parse_as_a_shortcode_is_refused() {
        assert!(validate_name("a").is_err());
        assert!(validate_name("with space").is_err());
        assert!(validate_name("emoji!").is_err());
        assert!(validate_name(&"x".repeat(33)).is_err());
    }
}
