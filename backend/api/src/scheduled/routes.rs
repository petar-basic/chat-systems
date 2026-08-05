use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::{delete, get, patch};
use axum::{middleware, Json, Router};
use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};
use shared_common::validation;

use super::models::*;
use super::repo::NewScheduledMessage;
use crate::middleware::{auth_middleware, AuthUser};
use crate::state::AppState;

const MAX_SCHEDULE_AHEAD_DAYS: i64 = 120;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route(
            "/workspaces/:ws_id/scheduled-messages",
            get(list_scheduled).post(create_scheduled),
        )
        .route("/scheduled-messages/:id", patch(reschedule))
        .route("/scheduled-messages/:id", delete(cancel_scheduled))
        .layer(middleware::from_fn(auth_middleware))
        .with_state(state)
}

fn validate_send_at(send_at: DateTime<Utc>) -> AppResult<()> {
    let now = Utc::now();
    if send_at <= now {
        return Err(AppError::Validation(
            "Scheduled time must be in the future".into(),
        ));
    }
    if send_at > now + Duration::days(MAX_SCHEDULE_AHEAD_DAYS) {
        return Err(AppError::Validation(format!(
            "Messages can be scheduled at most {MAX_SCHEDULE_AHEAD_DAYS} days ahead"
        )));
    }
    Ok(())
}

async fn require_target_access(
    state: &AppState,
    auth: &AuthUser,
    workspace_id: Uuid,
    channel_id: Option<Uuid>,
    conversation_id: Option<Uuid>,
) -> AppResult<()> {
    match (channel_id, conversation_id) {
        (Some(channel_id), None) => {
            let channel = state
                .workspace_service
                .repo
                .find_channel_by_id(channel_id)
                .await?
                .ok_or_else(|| AppError::NotFound("Channel not found".into()))?;
            if channel.workspace_id != workspace_id {
                return Err(AppError::Validation(
                    "Channel does not belong to this workspace".into(),
                ));
            }
            crate::messaging::routes::require_channel_access(state, channel_id, auth.user_id)
                .await?;
            Ok(())
        }
        (None, Some(conversation_id)) => {
            let conversation = state
                .conversation_repo
                .find_by_id(conversation_id)
                .await?
                .ok_or_else(|| AppError::NotFound("Conversation not found".into()))?;
            if conversation.workspace_id != workspace_id {
                return Err(AppError::Validation(
                    "Conversation does not belong to this workspace".into(),
                ));
            }
            if !state
                .conversation_repo
                .is_participant(conversation_id, auth.user_id)
                .await?
            {
                return Err(AppError::Forbidden(
                    "Not a participant in this conversation".into(),
                ));
            }
            Ok(())
        }
        _ => Err(AppError::Validation(
            "Schedule to exactly one channel or conversation".into(),
        )),
    }
}

async fn list_scheduled(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let messages = state
        .scheduled_repo
        .list_pending_for_user(ws_id, auth.user_id)
        .await?;
    Ok(Json(serde_json::json!({ "data": messages })))
}

async fn create_scheduled(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<CreateScheduledMessageRequest>,
) -> AppResult<Json<ScheduledMessage>> {
    validation::validate_message_content(&req.content)?;
    validate_send_at(req.send_at)?;
    require_target_access(&state, &auth, ws_id, req.channel_id, req.conversation_id).await?;

    let scheduled = state
        .scheduled_repo
        .create(NewScheduledMessage {
            workspace_id: ws_id,
            user_id: auth.user_id,
            channel_id: req.channel_id,
            conversation_id: req.conversation_id,
            content: &req.content,
            send_at: req.send_at,
        })
        .await?;

    Ok(Json(scheduled))
}

async fn owned_pending(state: &AppState, id: Uuid, user_id: Uuid) -> AppResult<ScheduledMessage> {
    let scheduled = state
        .scheduled_repo
        .find_by_id(id)
        .await?
        .ok_or_else(|| AppError::NotFound("Scheduled message not found".into()))?;
    if scheduled.user_id != user_id {
        return Err(AppError::Forbidden(
            "Can only manage your own scheduled messages".into(),
        ));
    }
    if scheduled.sent_at.is_some() {
        return Err(AppError::Conflict(
            "That message has already been sent".into(),
        ));
    }
    if scheduled.canceled_at.is_some() {
        return Err(AppError::Conflict(
            "That message was already canceled".into(),
        ));
    }
    Ok(scheduled)
}

async fn reschedule(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<RescheduleRequest>,
) -> AppResult<Json<ScheduledMessage>> {
    validate_send_at(req.send_at)?;
    owned_pending(&state, id, auth.user_id).await?;
    let updated = state.scheduled_repo.reschedule(id, req.send_at).await?;
    Ok(Json(updated))
}

async fn cancel_scheduled(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    owned_pending(&state, id, auth.user_id).await?;
    state.scheduled_repo.cancel(id).await?;
    Ok(Json(serde_json::json!({ "status": "canceled" })))
}
