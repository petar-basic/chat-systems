use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::*;
use super::repo::NewSavedMessage;
use crate::authz;
use crate::dto::{DataList, StatusResponse};
use crate::middleware::AuthUser;
use crate::state::AppState;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(list_saved, save_message))
        .routes(routes!(unsave_message))
}

#[utoipa::path(get, path = "/workspaces/{ws_id}/saved", tag = "saved", responses((status = 200, body = DataList<SavedMessageDetail>)))]
async fn list_saved(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<DataList<SavedMessageDetail>>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    let saved = state.saved_repo.list(auth.user_id, ws_id).await?;
    Ok(Json(saved.into()))
}

#[utoipa::path(post, path = "/workspaces/{ws_id}/saved", tag = "saved", request_body = SaveMessageRequest, responses((status = 200, body = SavedMessage)))]
async fn save_message(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<SaveMessageRequest>,
) -> AppResult<Json<SavedMessage>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;

    // Saving is reading: whatever lets somebody open the message is what lets
    // them keep a pointer to it.
    let message = state
        .message_repo
        .find_by_id(req.message_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Message not found".into()))?;
    let channel = authz::require_channel_access(&state, message.channel_id, auth.user_id).await?;
    if channel.channel.workspace_id != ws_id {
        return Err(AppError::Validation(
            "That message is in another workspace".into(),
        ));
    }

    let note = req.note.as_deref().map(str::trim).filter(|n| !n.is_empty());
    if note.is_some_and(|n| n.len() > 500) {
        return Err(AppError::Validation(
            "A note is at most 500 characters".into(),
        ));
    }

    let saved = state
        .saved_repo
        .save(NewSavedMessage {
            user_id: auth.user_id,
            workspace_id: ws_id,
            message_id: req.message_id,
            note,
        })
        .await?;

    Ok(Json(saved))
}

#[utoipa::path(delete, path = "/saved/{id}", tag = "saved", responses((status = 200, body = StatusResponse)))]
async fn unsave_message(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<StatusResponse>> {
    let saved = state
        .saved_repo
        .find(id)
        .await?
        .ok_or_else(|| AppError::NotFound("Not saved".into()))?;
    if saved.user_id != auth.user_id {
        return Err(AppError::Forbidden("That is somebody else's list".into()));
    }
    state.saved_repo.delete(id).await?;
    Ok(Json(StatusResponse::new("removed")))
}
