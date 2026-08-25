use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::*;
use super::repo::NewSavedMessage;
use crate::authz;
use crate::middleware::AuthUser;
use crate::state::AppState;

pub fn router(state: Arc<AppState>) -> Router {
    let routes = Router::new()
        .route("/workspaces/{ws_id}/saved", get(list_saved))
        .route("/workspaces/{ws_id}/saved", post(save_message))
        .route("/saved/{id}", delete(unsave_message));

    crate::protected(state, routes)
}

async fn list_saved(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    let saved = state.saved_repo.list(auth.user_id, ws_id).await?;
    Ok(Json(serde_json::json!({ "data": saved })))
}

async fn save_message(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<SaveMessageRequest>,
) -> AppResult<Json<SavedMessage>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;

    // Saving is reading: whatever lets somebody open the message is what lets
    // them keep a pointer to it.
    match (req.message_id, req.conversation_message_id) {
        (Some(message_id), None) => {
            let message = state
                .message_repo
                .find_by_id(message_id)
                .await?
                .ok_or_else(|| AppError::NotFound("Message not found".into()))?;
            let channel =
                authz::require_channel_access(&state, message.channel_id, auth.user_id).await?;
            if channel.channel.workspace_id != ws_id {
                return Err(AppError::Validation(
                    "That message is in another workspace".into(),
                ));
            }
        }
        (None, Some(conversation_message_id)) => {
            let message = state
                .conversation_repo
                .find_message(conversation_message_id)
                .await?
                .ok_or_else(|| AppError::NotFound("Message not found".into()))?;
            let conversation = authz::require_conversation_participant(
                &state,
                message.conversation_id,
                auth.user_id,
            )
            .await?;
            if conversation.workspace_id != ws_id {
                return Err(AppError::Validation(
                    "That message is in another workspace".into(),
                ));
            }
        }
        _ => {
            return Err(AppError::Validation(
                "Save exactly one channel message or one conversation message".into(),
            ))
        }
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
            conversation_message_id: req.conversation_message_id,
            note,
        })
        .await?;

    Ok(Json(saved))
}

async fn unsave_message(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let saved = state
        .saved_repo
        .find(id)
        .await?
        .ok_or_else(|| AppError::NotFound("Not saved".into()))?;
    if saved.user_id != auth.user_id {
        return Err(AppError::Forbidden("That is somebody else's list".into()));
    }
    state.saved_repo.delete(id).await?;
    Ok(Json(serde_json::json!({ "status": "removed" })))
}
