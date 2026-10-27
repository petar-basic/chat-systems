use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::Json;
use garde::Validate;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

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
        .routes(routes!(
            get_notification_schedule,
            set_notification_schedule
        ))
        .routes(routes!(get_email_preference, set_email_preference))
        .routes(routes!(set_channel_notify_level))
        .routes(routes!(get_notify_default, set_notify_default))
        .routes(routes!(get_keywords, set_keywords))
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

#[utoipa::path(get, path = "/workspaces/{ws_id}/notifications/dnd", tag = "notifications", responses((status = 200, body = DndStatus)))]
async fn get_dnd(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<DndStatus>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    let status = state
        .notification_repo
        .dnd_status(ws_id, auth.user_id, chrono::Utc::now())
        .await?;
    Ok(Json(status))
}

#[utoipa::path(patch, path = "/workspaces/{ws_id}/notifications/dnd", tag = "notifications", request_body = SetDndRequest, responses((status = 200, body = DndStatus)))]
async fn set_dnd(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<SetDndRequest>,
) -> AppResult<Json<DndStatus>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    let now = chrono::Utc::now();
    state
        .notification_repo
        .set_dnd_at(ws_id, auth.user_id, req.dnd_until, now)
        .await?;
    let status = state
        .notification_repo
        .dnd_status(ws_id, auth.user_id, now)
        .await?;
    Ok(Json(status))
}

#[utoipa::path(get, path = "/workspaces/{ws_id}/notifications/schedule", tag = "notifications", responses((status = 200, body = NotificationSchedule)))]
async fn get_notification_schedule(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<NotificationSchedule>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    Ok(Json(
        state
            .notification_repo
            .schedule(ws_id, auth.user_id)
            .await?,
    ))
}

#[utoipa::path(put, path = "/workspaces/{ws_id}/notifications/schedule", tag = "notifications", request_body = SetScheduleRequest, responses((status = 200, body = NotificationSchedule)))]
async fn set_notification_schedule(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<SetScheduleRequest>,
) -> AppResult<Json<NotificationSchedule>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    req.validate()?;
    state
        .notification_repo
        .set_schedule(
            ws_id,
            auth.user_id,
            req.enabled,
            &req.days(),
            req.start_minute,
            req.end_minute,
        )
        .await?;
    Ok(Json(
        state
            .notification_repo
            .schedule(ws_id, auth.user_id)
            .await?,
    ))
}

/// Whether to email a mention that reached nobody. On by default: somebody who
/// never grants push permission is exactly who it is for, and they are not going
/// to go looking for a switch to turn it on.
#[utoipa::path(get, path = "/workspaces/{ws_id}/notifications/email", tag = "notifications", responses((status = 200, body = EmailPreference)))]
async fn get_email_preference(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<EmailPreference>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    Ok(Json(EmailPreference {
        mention_emails: state
            .notification_repo
            .mention_emails(ws_id, auth.user_id)
            .await?,
        available: state.auth_service.can_send_email(),
    }))
}

#[utoipa::path(patch, path = "/workspaces/{ws_id}/notifications/email", tag = "notifications", request_body = EmailPreferenceRequest, responses((status = 200, body = EmailPreference)))]
async fn set_email_preference(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<EmailPreferenceRequest>,
) -> AppResult<Json<EmailPreference>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    state
        .notification_repo
        .set_mention_emails(ws_id, auth.user_id, req.mention_emails)
        .await?;
    Ok(Json(EmailPreference {
        mention_emails: req.mention_emails,
        available: state.auth_service.can_send_email(),
    }))
}

#[utoipa::path(put, path = "/channels/{ch_id}/notify-level", tag = "notifications", request_body = SetNotifyLevelRequest, responses((status = 200, body = ChannelNotifyLevel)))]
async fn set_channel_notify_level(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ch_id): Path<Uuid>,
    Json(req): Json<SetNotifyLevelRequest>,
) -> AppResult<Json<ChannelNotifyLevel>> {
    authz::find_channel(&state, ch_id).await?;
    let level = state
        .notification_repo
        .set_channel_level(ch_id, auth.user_id, req.level)
        .await?
        .ok_or_else(|| AppError::Forbidden("Not a member of this channel".into()))?;
    Ok(Json(level))
}

#[utoipa::path(get, path = "/workspaces/{ws_id}/notify-default", tag = "notifications", responses((status = 200, body = NotifyDefault)))]
async fn get_notify_default(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<NotifyDefault>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    let notify_default = state
        .notification_repo
        .get_default(ws_id, auth.user_id)
        .await?
        .ok_or_else(|| AppError::Forbidden("Not a member of this workspace".into()))?;
    Ok(Json(NotifyDefault { notify_default }))
}

#[utoipa::path(put, path = "/workspaces/{ws_id}/notify-default", tag = "notifications", request_body = SetNotifyDefaultRequest, responses((status = 200, body = NotifyDefault)))]
async fn set_notify_default(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<SetNotifyDefaultRequest>,
) -> AppResult<Json<NotifyDefault>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    state
        .notification_repo
        .set_default(ws_id, auth.user_id, req.level)
        .await?;
    Ok(Json(NotifyDefault {
        notify_default: req.level,
    }))
}

#[utoipa::path(get, path = "/workspaces/{ws_id}/notification-keywords", tag = "notifications", responses((status = 200, body = KeywordList)))]
async fn get_keywords(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<KeywordList>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    let keywords = state
        .notification_repo
        .keywords(ws_id, auth.user_id)
        .await?;
    Ok(Json(KeywordList { keywords }))
}

#[utoipa::path(put, path = "/workspaces/{ws_id}/notification-keywords", tag = "notifications", request_body = SetKeywordsRequest, responses((status = 200, body = KeywordList)))]
async fn set_keywords(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<SetKeywordsRequest>,
) -> AppResult<Json<KeywordList>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    req.validate()?;
    let keywords = state
        .notification_repo
        .set_keywords(ws_id, auth.user_id, &req.keywords)
        .await?;
    Ok(Json(KeywordList { keywords }))
}
