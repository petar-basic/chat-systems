use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::Json;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::repo::{ChannelOverride, RetentionPolicy, UpdateRetentionRequest};
use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::authz;
use crate::middleware::AuthUser;
use crate::state::AppState;
use crate::workspace::models::{Channel, ChannelType, WorkspaceRole};

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(get_policy, update_policy))
        .routes(routes!(preview_policy))
        .routes(routes!(
            get_channel_retention,
            set_channel_retention,
            clear_channel_retention
        ))
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct PolicyResponse {
    pub policy: Option<RetentionPolicy>,
}

#[derive(Debug, serde::Deserialize, utoipa::IntoParams)]
pub struct PreviewQuery {
    pub message_days: Option<i32>,
    pub file_days: Option<i32>,
    #[serde(default)]
    pub channel_overrides: bool,
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct RetentionPreview {
    pub messages: i64,
    pub files: i64,
}

/// What the next pass would remove under the periods asked about, counted by
/// the queries the purge itself uses. Shown before the owner saves.
#[utoipa::path(get, path = "/workspaces/{ws_id}/retention/preview", tag = "retention", params(PreviewQuery), responses((status = 200, body = RetentionPreview)))]
async fn preview_policy(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Query(query): Query<PreviewQuery>,
) -> AppResult<Json<RetentionPreview>> {
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Owner).await?;
    if [query.message_days, query.file_days]
        .into_iter()
        .flatten()
        .any(|days| days < 1)
    {
        return Err(AppError::BadRequest(
            "A retention period is at least one day".into(),
        ));
    }
    let messages = if query.message_days.is_some() || query.channel_overrides {
        state
            .retention_repo
            .count_messages_past(ws_id, query.message_days, query.channel_overrides)
            .await?
    } else {
        0
    };
    let files = match query.file_days {
        Some(days) => state.retention_repo.count_files_past(ws_id, days).await?,
        None => 0,
    };
    Ok(Json(RetentionPreview { messages, files }))
}

#[utoipa::path(get, path = "/workspaces/{ws_id}/retention", tag = "retention", responses((status = 200, body = PolicyResponse)))]
async fn get_policy(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<PolicyResponse>> {
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
    let policy = state.retention_repo.get(ws_id).await?;
    Ok(Json(PolicyResponse { policy }))
}

/// Owner-only, and audited with both ends of the change. Shortening retention
/// destroys data on the next nightly pass and there is no undo, so the record of
/// who asked for it matters more than usual.
#[utoipa::path(patch, path = "/workspaces/{ws_id}/retention", tag = "retention", request_body = UpdateRetentionRequest, responses((status = 200, body = PolicyResponse)))]
async fn update_policy(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<UpdateRetentionRequest>,
) -> AppResult<Json<PolicyResponse>> {
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Owner).await?;

    let before = state.retention_repo.get(ws_id).await?;
    let policy = state
        .retention_repo
        .upsert(ws_id, &req, auth.user_id)
        .await?;
    let policy = Some(policy);

    audit::record(
        &state,
        AuditEntry::new(AuditAction::RetentionChanged, auth.user_id)
            .workspace(ws_id)
            .resource(ws_id)
            .ip(&ip)
            .details(serde_json::json!({
                "from": before,
                "to": policy,
            })),
    )
    .await;

    Ok(Json(PolicyResponse { policy }))
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct ChannelRetention {
    pub overrides_allowed: bool,
    pub workspace_message_days: Option<i32>,
    pub channel: Option<ChannelOverride>,
}

#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
pub struct SetChannelRetention {
    pub message_days: Option<i32>,
}

async fn channel_retention(state: &AppState, channel: &Channel) -> AppResult<ChannelRetention> {
    let policy = state.retention_repo.get(channel.workspace_id).await?;
    Ok(ChannelRetention {
        overrides_allowed: policy.as_ref().is_some_and(|p| p.channel_overrides),
        workspace_message_days: policy.and_then(|p| p.message_days),
        channel: state.retention_repo.channel_override(channel.id).await?,
    })
}

async fn require_overridable(state: &AppState, channel: &Channel, user_id: Uuid) -> AppResult<()> {
    authz::require_channel_moderator(state, channel, user_id).await?;
    if matches!(channel.channel_type, ChannelType::Dm | ChannelType::GroupDm) {
        return Err(AppError::BadRequest(
            "Direct messages keep the workspace's period".into(),
        ));
    }
    Ok(())
}

fn describe(change: Option<&ChannelOverride>) -> String {
    match change.map(|c| c.message_days) {
        Some(Some(1)) => "set this channel to keep messages for 1 day".into(),
        Some(Some(days)) => format!("set this channel to keep messages for {days} days"),
        Some(None) => "set this channel to keep messages forever".into(),
        None => "set this channel to follow the workspace's message retention".into(),
    }
}

/// The channel hears about it from a message in its own history, and the audit
/// log keeps both ends: a shorter period here deletes on the next pass.
async fn announce(
    state: &AppState,
    channel: &Channel,
    user_id: Uuid,
    ip: &ClientIp,
    before: Option<ChannelOverride>,
    after: Option<ChannelOverride>,
) -> AppResult<()> {
    let mut tx = state.pool.begin().await?;
    let msg = state
        .message_repo
        .create_system_message_in(
            &mut tx,
            channel.id,
            user_id,
            &describe(after.as_ref()),
            serde_json::json!({
                "kind": "retention_changed",
                "message_days": after.as_ref().and_then(|c| c.message_days),
                "follows_workspace": after.is_none(),
            }),
        )
        .await?;
    let staged = state
        .publisher
        .stage_message_created(&mut tx, &msg, channel.workspace_id, &[])
        .await?;
    tx.commit().await?;
    state.publisher.dispatch(staged).await;

    audit::record(
        state,
        AuditEntry::new(AuditAction::RetentionChanged, user_id)
            .workspace(channel.workspace_id)
            .resource(channel.id)
            .ip(ip)
            .details(serde_json::json!({
                "channel_id": channel.id,
                "from": before,
                "to": after,
            })),
    )
    .await;
    Ok(())
}

#[utoipa::path(get, path = "/channels/{ch_id}/retention", tag = "retention", responses((status = 200, body = ChannelRetention)))]
async fn get_channel_retention(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ch_id): Path<Uuid>,
) -> AppResult<Json<ChannelRetention>> {
    let access = authz::require_channel_access(&state, ch_id, auth.user_id).await?;
    Ok(Json(channel_retention(&state, &access.channel).await?))
}

/// A channel's own period, `null` for forever. Only while the owner allows
/// channels their own; the row stays when that is switched off, and is ignored.
#[utoipa::path(put, path = "/channels/{ch_id}/retention", tag = "retention", request_body = SetChannelRetention, responses((status = 200, body = ChannelRetention)))]
async fn set_channel_retention(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(ch_id): Path<Uuid>,
    Json(req): Json<SetChannelRetention>,
) -> AppResult<Json<ChannelRetention>> {
    let channel = authz::find_channel(&state, ch_id).await?;
    require_overridable(&state, &channel, auth.user_id).await?;
    if req.message_days.is_some_and(|days| days < 1) {
        return Err(AppError::BadRequest(
            "A retention period is at least one day".into(),
        ));
    }
    let allowed = state
        .retention_repo
        .get(channel.workspace_id)
        .await?
        .is_some_and(|p| p.channel_overrides);
    if !allowed {
        return Err(AppError::Conflict(
            "The workspace owner has not allowed channels a period of their own".into(),
        ));
    }

    let before = state.retention_repo.channel_override(ch_id).await?;
    let after = state
        .retention_repo
        .set_channel_override(ch_id, req.message_days, auth.user_id)
        .await?;
    if before.as_ref().map(|b| b.message_days) != Some(after.message_days) {
        announce(&state, &channel, auth.user_id, &ip, before, Some(after)).await?;
    }
    Ok(Json(channel_retention(&state, &channel).await?))
}

#[utoipa::path(delete, path = "/channels/{ch_id}/retention", tag = "retention", responses((status = 200, body = ChannelRetention)))]
async fn clear_channel_retention(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(ch_id): Path<Uuid>,
) -> AppResult<Json<ChannelRetention>> {
    let channel = authz::find_channel(&state, ch_id).await?;
    require_overridable(&state, &channel, auth.user_id).await?;
    let before = state.retention_repo.channel_override(ch_id).await?;
    if before.is_some() && state.retention_repo.clear_channel_override(ch_id).await? {
        announce(&state, &channel, auth.user_id, &ip, before, None).await?;
    }
    Ok(Json(channel_retention(&state, &channel).await?))
}
