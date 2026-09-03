use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::Json;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::AppResult;

use super::models::*;
use crate::authz;
use crate::dto::DataList;
use crate::middleware::AuthUser;
use crate::pagination::PageQuery;
use crate::state::AppState;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(list_notifications))
        .routes(routes!(mark_read))
        .routes(routes!(mark_all_read))
        .routes(routes!(mark_channel_read))
        .routes(routes!(unread_count))
        .routes(routes!(get_dnd, set_dnd))
        .routes(routes!(get_email_preference, set_email_preference))
}

#[utoipa::path(get, path = "/workspaces/{ws_id}/notifications", tag = "notifications", params(PageQuery), responses((status = 200, body = DataList<Notification>)))]
async fn list_notifications(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Query(params): Query<PageQuery>,
) -> AppResult<Json<DataList<Notification>>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    let notifications = state
        .notification_repo
        .list_for_user(auth.user_id, ws_id, params.limit(), params.offset())
        .await?;
    Ok(Json(notifications.into()))
}

#[utoipa::path(
    operation_id = "notifications_mark_read",
    post, path = "/notifications/read", tag = "notifications", request_body = MarkNotificationsReadRequest, responses((status = 200, body = UpdatedCount)))]
async fn mark_read(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(req): Json<MarkNotificationsReadRequest>,
) -> AppResult<Json<UpdatedCount>> {
    let count = state
        .notification_repo
        .mark_read(&req.notification_ids, auth.user_id)
        .await?;
    Ok(Json(UpdatedCount { updated: count }))
}

#[utoipa::path(post, path = "/workspaces/{ws_id}/notifications/read-all", tag = "notifications", responses((status = 200, body = UpdatedCount)))]
async fn mark_all_read(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<UpdatedCount>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    let count = state
        .notification_repo
        .mark_all_read(auth.user_id, ws_id)
        .await?;
    Ok(Json(UpdatedCount { updated: count }))
}

#[utoipa::path(post, path = "/workspaces/{ws_id}/channels/{ch_id}/notifications/read", tag = "notifications", responses((status = 200, body = UpdatedCount)))]
async fn mark_channel_read(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((ws_id, ch_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<UpdatedCount>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    let count = state
        .notification_repo
        .mark_channel_read(auth.user_id, ws_id, ch_id)
        .await?;
    Ok(Json(UpdatedCount { updated: count }))
}

#[utoipa::path(get, path = "/workspaces/{ws_id}/notifications/unread-count", tag = "notifications", responses((status = 200, body = UnreadCountResponse)))]
async fn unread_count(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<UnreadCountResponse>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    let count = state
        .notification_repo
        .unread_count(auth.user_id, ws_id)
        .await?;
    Ok(Json(UnreadCountResponse {
        unread_count: count,
    }))
}

#[utoipa::path(get, path = "/notifications/dnd", tag = "notifications", responses((status = 200, body = DndResponse)))]
async fn get_dnd(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> AppResult<Json<DndResponse>> {
    let dnd_until = state.notification_repo.get_dnd(auth.user_id).await?;
    Ok(Json(DndResponse { dnd_until }))
}

#[utoipa::path(patch, path = "/notifications/dnd", tag = "notifications", request_body = SetDndRequest, responses((status = 200, body = DndResponse)))]
async fn set_dnd(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(req): Json<SetDndRequest>,
) -> AppResult<Json<DndResponse>> {
    state
        .notification_repo
        .set_dnd(auth.user_id, req.dnd_until)
        .await?;
    Ok(Json(DndResponse {
        dnd_until: req.dnd_until,
    }))
}

/// Whether to email a mention that reached nobody. On by default: somebody who
/// never grants push permission is exactly who it is for, and they are not going
/// to go looking for a switch to turn it on.
#[utoipa::path(get, path = "/notifications/email", tag = "notifications", responses((status = 200, body = EmailPreference)))]
async fn get_email_preference(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> AppResult<Json<EmailPreference>> {
    let enabled = sqlx::query_scalar!(
        "SELECT mention_emails FROM users WHERE id = $1",
        auth.user_id
    )
    .fetch_optional(&state.pool)
    .await?;

    Ok(Json(EmailPreference {
        mention_emails: enabled.unwrap_or(true),
        available: state.auth_service.can_send_email(),
    }))
}

#[utoipa::path(patch, path = "/notifications/email", tag = "notifications", request_body = EmailPreferenceRequest, responses((status = 200, body = EmailPreference)))]
async fn set_email_preference(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(req): Json<EmailPreferenceRequest>,
) -> AppResult<Json<EmailPreference>> {
    sqlx::query!(
        "UPDATE users SET mention_emails = $2 WHERE id = $1",
        auth.user_id,
        req.mention_emails
    )
    .execute(&state.pool)
    .await?;

    Ok(Json(EmailPreference {
        mention_emails: req.mention_emails,
        available: state.auth_service.can_send_email(),
    }))
}
