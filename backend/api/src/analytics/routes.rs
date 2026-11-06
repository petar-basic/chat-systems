use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::Json;
use chrono::{Duration, NaiveDate, Utc};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::{
    AnalyticsQuery, ArchiveChannelsRequest, ArchivedChannels, ChannelActivity, ChannelAnalytics,
    MemberActivity, QuietChannel, QuietQuery, WorkspaceAnalytics, MAX_ARCHIVED_AT_ONCE,
    QUIET_THRESHOLDS, WINDOWS,
};
use crate::audit::ClientIp;
use crate::authz;
use crate::dto::DataList;
use crate::middleware::AuthUser;
use crate::state::AppState;
use crate::workspace::models::{ChannelType, WorkspaceRole};

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(workspace_analytics))
        .routes(routes!(member_analytics))
        .routes(routes!(channel_list_analytics))
        .routes(routes!(quiet_channels))
        .routes(routes!(archive_quiet_channels))
        .routes(routes!(channel_analytics))
}

const CHANNEL_DAYS: i32 = 30;

fn is_a_channel(channel_type: &ChannelType) -> bool {
    matches!(channel_type, ChannelType::Public | ChannelType::Private)
}

fn window(days: i32) -> AppResult<(NaiveDate, NaiveDate)> {
    if !WINDOWS.contains(&days) {
        return Err(AppError::BadRequest(
            "Analytics cover the last 7, 30 or 90 days".into(),
        ));
    }
    let to = Utc::now().date_naive();
    Ok((to - Duration::days(i64::from(days) - 1), to))
}

#[utoipa::path(get, path = "/workspaces/{ws_id}/analytics", tag = "analytics", params(AnalyticsQuery), responses((status = 200, body = WorkspaceAnalytics)))]
async fn workspace_analytics(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Query(query): Query<AnalyticsQuery>,
) -> AppResult<Json<WorkspaceAnalytics>> {
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
    let (from, to) = window(query.days)?;
    let repo = &state.analytics_repo;
    let daily = repo.daily(ws_id, from, to, query.guests).await?;
    let weekly = repo.weekly(ws_id, from, to, query.guests).await?;
    let active_members = repo.active_between(ws_id, from, to, query.guests).await?;
    let members = daily.last().map(|d| d.members).unwrap_or(0);
    Ok(Json(WorkspaceAnalytics {
        from,
        to,
        members,
        active_members,
        daily,
        weekly,
    }))
}

#[utoipa::path(get, path = "/workspaces/{ws_id}/analytics/members", tag = "analytics", params(AnalyticsQuery), responses((status = 200, body = DataList<MemberActivity>)))]
async fn member_analytics(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Query(query): Query<AnalyticsQuery>,
) -> AppResult<Json<DataList<MemberActivity>>> {
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
    let (from, to) = window(query.days)?;
    let members = state
        .analytics_repo
        .members(ws_id, from, to, query.guests)
        .await?;
    Ok(Json(members.into()))
}

#[utoipa::path(get, path = "/workspaces/{ws_id}/analytics/channels", tag = "analytics", params(AnalyticsQuery), responses((status = 200, body = DataList<ChannelActivity>)))]
async fn channel_list_analytics(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Query(query): Query<AnalyticsQuery>,
) -> AppResult<Json<DataList<ChannelActivity>>> {
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
    let (from, to) = window(query.days)?;
    let channels = state
        .analytics_repo
        .channels(ws_id, from, to, query.guests, None)
        .await?;
    Ok(Json(channels.into()))
}

#[utoipa::path(get, path = "/workspaces/{ws_id}/analytics/quiet-channels", tag = "analytics", params(QuietQuery), responses((status = 200, body = DataList<QuietChannel>)))]
async fn quiet_channels(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Query(query): Query<QuietQuery>,
) -> AppResult<Json<DataList<QuietChannel>>> {
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
    if !QUIET_THRESHOLDS.contains(&query.days) {
        return Err(AppError::BadRequest(
            "A quiet channel has had no message for 30, 60, 90 or 180 days".into(),
        ));
    }
    let channels = state
        .analytics_repo
        .quiet_channels(ws_id, query.days)
        .await?;
    Ok(Json(channels.into()))
}

#[utoipa::path(post, path = "/workspaces/{ws_id}/analytics/quiet-channels/archive", tag = "analytics", request_body = ArchiveChannelsRequest, responses((status = 200, body = ArchivedChannels)))]
async fn archive_quiet_channels(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<ArchiveChannelsRequest>,
) -> AppResult<Json<ArchivedChannels>> {
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
    let mut ids = req.channel_ids;
    ids.sort();
    ids.dedup();
    if ids.is_empty() || ids.len() > MAX_ARCHIVED_AT_ONCE {
        return Err(AppError::BadRequest(format!(
            "Archive between 1 and {MAX_ARCHIVED_AT_ONCE} channels at once"
        )));
    }

    let mut channels = Vec::with_capacity(ids.len());
    for id in &ids {
        let channel = authz::find_channel(&state, *id).await?;
        if channel.workspace_id != ws_id || !is_a_channel(&channel.channel_type) {
            return Err(AppError::NotFound("Channel not found".into()));
        }
        if channel.is_default {
            return Err(AppError::BadRequest(format!(
                "#{} is the workspace's default channel and cannot be archived",
                channel.name.as_deref().unwrap_or("channel")
            )));
        }
        channels.push(channel);
    }

    let mut archived = Vec::with_capacity(channels.len());
    for channel in channels.iter().filter(|c| !c.is_archived) {
        crate::workspace::routes::set_archived(&state, channel, auth.user_id, &ip, true).await?;
        archived.push(channel.id);
    }
    Ok(Json(ArchivedChannels { archived }))
}

#[utoipa::path(get, path = "/channels/{ch_id}/analytics", tag = "analytics", responses((status = 200, body = ChannelAnalytics)))]
async fn channel_analytics(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ch_id): Path<Uuid>,
) -> AppResult<Json<ChannelAnalytics>> {
    let channel = authz::find_channel(&state, ch_id).await?;
    if !is_a_channel(&channel.channel_type) {
        return Err(AppError::NotFound("Channel not found".into()));
    }
    authz::require_channel_moderator(&state, &channel, auth.user_id).await?;
    let (from, to) = window(CHANNEL_DAYS)?;
    let channel = state
        .analytics_repo
        .channels(channel.workspace_id, from, to, true, Some(ch_id))
        .await?
        .pop()
        .ok_or_else(|| AppError::NotFound("Channel not found".into()))?;
    Ok(Json(ChannelAnalytics { from, to, channel }))
}
