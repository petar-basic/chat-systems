use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use chrono::Utc;
use garde::Validate;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};
use shared_common::validation::custom_emoji_name;

use super::models::{SetStatusRequest, UpdateWorkspaceProfileRequest, WorkspaceProfile};
use crate::authz;
use crate::middleware::AuthUser;
use crate::state::AppState;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(get_workspace_profile, update_workspace_profile))
        .routes(routes!(set_workspace_status, clear_workspace_status))
}

async fn current(state: &AppState, ws_id: Uuid, user_id: Uuid) -> AppResult<WorkspaceProfile> {
    state
        .profile_repo
        .get(ws_id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Not a member of this workspace".into()))
}

#[utoipa::path(get, path = "/workspaces/{ws_id}/profile", tag = "users", responses((status = 200, body = WorkspaceProfile)))]
async fn get_workspace_profile(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<WorkspaceProfile>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    Ok(Json(current(&state, ws_id, auth.user_id).await?))
}

#[utoipa::path(patch, path = "/workspaces/{ws_id}/profile", tag = "users", request_body = UpdateWorkspaceProfileRequest, responses((status = 200, body = WorkspaceProfile)))]
async fn update_workspace_profile(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<UpdateWorkspaceProfileRequest>,
) -> AppResult<Json<WorkspaceProfile>> {
    req.validate()?;
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    state
        .profile_repo
        .update(
            ws_id,
            auth.user_id,
            req.display_name.as_deref(),
            req.avatar_url.as_deref(),
            req.bio.as_deref(),
        )
        .await?;
    Ok(Json(current(&state, ws_id, auth.user_id).await?))
}

#[utoipa::path(put, path = "/workspaces/{ws_id}/profile/status", tag = "users", request_body = SetStatusRequest, responses((status = 200, body = WorkspaceProfile)))]
async fn set_workspace_status(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<SetStatusRequest>,
) -> AppResult<Json<WorkspaceProfile>> {
    req.validate()?;
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    let emoji = req
        .emoji
        .as_deref()
        .map(str::trim)
        .filter(|e| !e.is_empty());
    let text = req.text.as_deref().map(str::trim).filter(|t| !t.is_empty());

    if emoji.is_none() && text.is_none() {
        return Err(AppError::Validation(
            "A status needs an emoji or some text".into(),
        ));
    }
    if req.expires_at.is_some_and(|at| at <= Utc::now()) {
        return Err(AppError::Validation(
            "A status cannot expire in the past".into(),
        ));
    }
    if let Some(name) = emoji.and_then(custom_emoji_name) {
        state
            .emoji_repo
            .find_by_name(ws_id, name)
            .await?
            .ok_or_else(|| {
                AppError::Validation(format!(":{name}: is not an emoji in this workspace"))
            })?;
    }

    state
        .profile_repo
        .set_status(ws_id, auth.user_id, emoji, text, req.expires_at)
        .await?;
    Ok(Json(current(&state, ws_id, auth.user_id).await?))
}

#[utoipa::path(delete, path = "/workspaces/{ws_id}/profile/status", tag = "users", responses((status = 200, body = WorkspaceProfile)))]
async fn clear_workspace_status(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<WorkspaceProfile>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    state
        .profile_repo
        .set_status(ws_id, auth.user_id, None, None, None)
        .await?;
    Ok(Json(current(&state, ws_id, auth.user_id).await?))
}
