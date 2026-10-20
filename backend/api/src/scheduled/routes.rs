use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use garde::Validate;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::*;
use super::repo::NewScheduledMessage;
use crate::authz;
use crate::dto::{DataList, StatusResponse};
use crate::middleware::AuthUser;
use crate::state::AppState;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(list_scheduled, create_scheduled))
        .routes(routes!(reschedule, cancel_scheduled))
}

async fn require_target_access(
    state: &AppState,
    auth: &AuthUser,
    workspace_id: Uuid,
    channel_id: Uuid,
) -> AppResult<()> {
    let channel = authz::find_channel(state, channel_id).await?;
    if channel.workspace_id != workspace_id {
        return Err(AppError::Validation(
            "Channel does not belong to this workspace".into(),
        ));
    }
    crate::authz::require_channel_post(state, channel_id, auth.user_id).await?;
    Ok(())
}

#[utoipa::path(get, path = "/workspaces/{ws_id}/scheduled-messages", tag = "scheduled", responses((status = 200, body = DataList<ScheduledMessage>)))]
async fn list_scheduled(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<DataList<ScheduledMessage>>> {
    let messages = state
        .scheduled_repo
        .list_pending_for_user(ws_id, auth.user_id)
        .await?;
    Ok(Json(messages.into()))
}

#[utoipa::path(post, path = "/workspaces/{ws_id}/scheduled-messages", tag = "scheduled", request_body = CreateScheduledMessageRequest, responses((status = 200, body = ScheduledMessage)))]
async fn create_scheduled(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<CreateScheduledMessageRequest>,
) -> AppResult<Json<ScheduledMessage>> {
    req.validate()?;
    require_target_access(&state, &auth, ws_id, req.channel_id).await?;

    let scheduled = state
        .scheduled_repo
        .create(NewScheduledMessage {
            workspace_id: ws_id,
            user_id: auth.user_id,
            channel_id: req.channel_id,
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

#[utoipa::path(patch, path = "/scheduled-messages/{id}", tag = "scheduled", request_body = RescheduleRequest, responses((status = 200, body = ScheduledMessage)))]
async fn reschedule(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<RescheduleRequest>,
) -> AppResult<Json<ScheduledMessage>> {
    req.validate()?;
    owned_pending(&state, id, auth.user_id).await?;
    let updated = state.scheduled_repo.reschedule(id, req.send_at).await?;
    Ok(Json(updated))
}

#[utoipa::path(delete, path = "/scheduled-messages/{id}", tag = "scheduled", responses((status = 200, body = StatusResponse)))]
async fn cancel_scheduled(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<StatusResponse>> {
    owned_pending(&state, id, auth.user_id).await?;
    state.scheduled_repo.cancel(id).await?;
    Ok(Json(StatusResponse::new("canceled")))
}
