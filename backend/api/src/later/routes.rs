use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::Json;
use chrono::Utc;
use garde::Validate;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::*;
use super::repo::{LaterChange, NewReminder};
use crate::authz;
use crate::dto::{DataList, StatusResponse};
use crate::middleware::AuthUser;
use crate::state::AppState;
use crate::workspace::models::WorkspaceRole;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(list_later, create_later))
        .routes(routes!(update_later, delete_later))
}

pub async fn announce(state: &AppState, user_id: Uuid, workspace_id: Uuid) {
    let payload = serde_json::json!({ "user_id": user_id, "workspace_id": workspace_id });
    if let Err(e) = state.publisher.publish("later.updated", payload).await {
        tracing::warn!("failed to announce a Later change: {e}");
    }
}

#[utoipa::path(get, path = "/workspaces/{ws_id}/later", tag = "later", params(LaterQuery), responses((status = 200, body = DataList<LaterItem>)))]
async fn list_later(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Query(query): Query<LaterQuery>,
) -> AppResult<Json<DataList<LaterItem>>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    let status = query.status.unwrap_or(LaterStatus::InProgress);
    let rows = state
        .later_repo
        .list(auth.user_id, ws_id, status.as_str())
        .await?;
    Ok(Json(DataList {
        data: rows.into_iter().map(LaterItem::from).collect(),
    }))
}

#[utoipa::path(post, path = "/workspaces/{ws_id}/later", tag = "later", request_body = CreateLaterRequest, responses((status = 200, body = LaterCreated)))]
async fn create_later(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<CreateLaterRequest>,
) -> AppResult<Json<LaterCreated>> {
    req.validate()?;
    let member = authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    if let Some(at) = req.remind_at {
        if at <= Utc::now() {
            return Err(AppError::Validation(
                "A reminder has to be in the future".into(),
            ));
        }
    }

    let (id, owner) = match (req.message_id, req.text.as_deref().map(str::trim)) {
        (Some(message_id), None) => {
            if req.for_user_id.is_some_and(|u| u != auth.user_id) {
                return Err(AppError::Validation(
                    "Only your own messages can be saved for later".into(),
                ));
            }
            let message = state
                .message_repo
                .find_by_id(message_id)
                .await?
                .filter(|m| m.deleted_at.is_none())
                .ok_or_else(|| AppError::NotFound("Message not found".into()))?;
            let access =
                authz::require_channel_access(&state, message.channel_id, auth.user_id).await?;
            if access.channel.workspace_id != ws_id {
                return Err(AppError::NotFound("Message not found".into()));
            }
            let id = state
                .later_repo
                .save_message(
                    ws_id,
                    auth.user_id,
                    message_id,
                    message.channel_id,
                    req.remind_at,
                )
                .await?;
            (id, auth.user_id)
        }
        (None, Some(text)) if !text.is_empty() => {
            let remind_at = req
                .remind_at
                .ok_or_else(|| AppError::Validation("A reminder needs a time".into()))?;
            let target = req.for_user_id.unwrap_or(auth.user_id);
            if target != auth.user_id {
                if !member.role.has_at_least(&WorkspaceRole::Admin) {
                    return Err(AppError::Forbidden(
                        "Only an admin can set a reminder for somebody else".into(),
                    ));
                }
                authz::require_workspace_member(&state, ws_id, target)
                    .await
                    .map_err(|_| {
                        AppError::Forbidden("That person is not in this workspace".into())
                    })?;
            }
            if let Some(channel_id) = req.channel_id {
                let access = authz::require_channel_access(&state, channel_id, target).await?;
                if access.channel.workspace_id != ws_id {
                    return Err(AppError::NotFound("Channel not found".into()));
                }
            }
            let id = state
                .later_repo
                .create_reminder(NewReminder {
                    workspace_id: ws_id,
                    user_id: target,
                    created_by: auth.user_id,
                    channel_id: req.channel_id,
                    text,
                    remind_at,
                })
                .await?;
            (id, target)
        }
        _ => {
            return Err(AppError::Validation(
                "Save a message, or write what to be reminded of".into(),
            ))
        }
    };

    announce(&state, owner, ws_id).await;
    Ok(Json(LaterCreated { id }))
}

async fn owned(state: &AppState, id: Uuid, user_id: Uuid) -> AppResult<LaterOwner> {
    state
        .later_repo
        .owner(id)
        .await?
        .filter(|item| item.user_id == user_id)
        .ok_or_else(|| AppError::NotFound("Not found".into()))
}

#[utoipa::path(patch, path = "/later/{id}", tag = "later", request_body = UpdateLaterRequest, responses((status = 200, body = StatusResponse)))]
async fn update_later(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateLaterRequest>,
) -> AppResult<Json<StatusResponse>> {
    req.validate()?;
    let item = owned(&state, id, auth.user_id).await?;
    if let Some(at) = req.remind_at {
        if at <= Utc::now() {
            return Err(AppError::Validation(
                "A reminder has to be in the future".into(),
            ));
        }
    }
    let note = req.note.as_deref().map(str::trim);
    state
        .later_repo
        .update(
            id,
            LaterChange {
                status: req.status.map(LaterStatus::as_str),
                set_reminder: req.remind_at.is_some() || req.clear_reminder,
                remind_at: req.remind_at.filter(|_| !req.clear_reminder),
                set_note: note.is_some(),
                note: note.filter(|n| !n.is_empty()),
            },
        )
        .await?;
    if let (Some(LaterStatus::Completed), Some(list_item_id)) = (req.status, item.list_item_id) {
        crate::lists::service::check_off(&state, auth.user_id, list_item_id).await?;
    }
    announce(&state, auth.user_id, item.workspace_id).await;
    Ok(Json(StatusResponse::new("updated")))
}

#[utoipa::path(delete, path = "/later/{id}", tag = "later", responses((status = 200, body = StatusResponse)))]
async fn delete_later(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<StatusResponse>> {
    let item = owned(&state, id, auth.user_id).await?;
    state.later_repo.delete(id).await?;
    announce(&state, auth.user_id, item.workspace_id).await;
    Ok(Json(StatusResponse::new("deleted")))
}
