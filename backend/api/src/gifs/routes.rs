use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::{gifs_on, GifProvider, GifRating, GifSettings};
use crate::authz;
use crate::middleware::AuthUser;
use crate::state::AppState;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new().routes(routes!(gif_settings))
}

#[utoipa::path(get, path = "/workspaces/{ws_id}/gifs", tag = "messages", responses((status = 200, body = GifSettings)))]
async fn gif_settings(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<GifSettings>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    available(&state, ws_id).await?.map(Json).ok_or_else(off)
}

pub async fn are_on(state: &AppState, ws_id: Uuid) -> AppResult<bool> {
    Ok(available(state, ws_id).await?.is_some())
}

pub async fn require_on(state: &AppState, ws_id: Uuid) -> AppResult<()> {
    if are_on(state, ws_id).await? {
        Ok(())
    } else {
        Err(off())
    }
}

async fn available(state: &AppState, ws_id: Uuid) -> AppResult<Option<GifSettings>> {
    let Some(api_key) = state.instance_settings_repo.giphy_api_key().await? else {
        return Ok(None);
    };
    let workspace = state
        .workspace_service
        .repo
        .find_workspace_by_id(ws_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Workspace not found".into()))?;
    if !gifs_on(&workspace.settings) {
        return Ok(None);
    }
    Ok(Some(GifSettings {
        provider: GifProvider::Giphy,
        api_key,
        rating: GifRating::of(&workspace.settings),
    }))
}

fn off() -> AppError {
    AppError::NotFound("GIFs are off in this workspace".into())
}
