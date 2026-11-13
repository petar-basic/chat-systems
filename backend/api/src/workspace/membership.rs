use shared_events::frames::Departure;
use sqlx::PgConnection;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::RevokedInvite;
use super::models::{Channel, ChannelMember, ChannelRole, ChannelType, WorkspaceRole};
use super::repo::WorkspaceRepo;
use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::authz::{self, Stopped};
use crate::bots::models::DisabledApp;
use crate::hooks::models::DisabledHook;
use crate::messaging::publisher::{EventPublisher, Staged};
use crate::state::AppState;
use crate::workflows::repo::StoppedWorkflow;

pub async fn invite(
    state: &AppState,
    channel: &Channel,
    actor: Uuid,
    user_id: Uuid,
    ip: &ClientIp,
) -> AppResult<ChannelMember> {
    authz::require_not_conversation(channel)?;
    authz::require_not_archived(channel)?;
    authz::require_workspace_role(state, channel.workspace_id, actor, &WorkspaceRole::Member)
        .await?;
    if channel.channel_type != ChannelType::Public
        && !authz::can_moderate_channel(state, channel, actor).await
    {
        state
            .workspace_service
            .repo
            .get_channel_member(channel.id, actor)
            .await?
            .ok_or_else(|| {
                AppError::Forbidden("Only members of this channel can add people to it".into())
            })?;
    }
    authz::require_workspace_member(state, channel.workspace_id, user_id).await?;
    let member = add_to_channel(state, channel, user_id).await?;

    audit::record(
        state,
        AuditEntry::new(AuditAction::ChannelMemberAdded, actor)
            .workspace(channel.workspace_id)
            .resource(channel.id)
            .ip(ip)
            .details(serde_json::json!({ "user_id": user_id })),
    )
    .await;

    Ok(member)
}

pub async fn add_to_channel(
    state: &AppState,
    channel: &Channel,
    user_id: Uuid,
) -> AppResult<ChannelMember> {
    authz::require_not_conversation(channel)?;
    let mut tx = state.pool.begin().await?;
    crate::guests::service::require_room_in(&mut tx, channel.workspace_id, &[user_id], channel.id)
        .await?;
    let Some(member) =
        WorkspaceRepo::add_channel_member_tx(&mut tx, channel.id, user_id, &ChannelRole::Member)
            .await?
    else {
        tx.rollback().await?;
        return state
            .workspace_service
            .repo
            .get_channel_member(channel.id, user_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Channel member not found".into()));
    };
    let staged = EventPublisher::stage_channel_member_joined(
        &mut tx,
        channel.id,
        channel.workspace_id,
        user_id,
    )
    .await?;
    tx.commit().await?;
    state.publisher.dispatch(staged).await;
    Ok(member)
}

/// Removing somebody from a workspace is four things, not one: the rows go, the
/// messages they had queued stop, and both the channel and workspace sides of
/// the realtime gateway are told. Callers that do only the first leave a person
/// who still receives everything.
///
/// Returns the channels they were dropped from, for callers that audit them.
pub async fn detach(
    state: &AppState,
    ws_id: Uuid,
    user_id: Uuid,
    reason: Departure,
) -> AppResult<Vec<Uuid>> {
    let mut tx = state.pool.begin().await?;
    let detached = detach_in(state, &mut tx, ws_id, user_id, reason).await?;
    tx.commit().await?;
    Ok(detached.settle(state, ws_id, user_id).await)
}

pub struct Detached {
    channels: Vec<Uuid>,
    staged: Vec<Staged>,
}

pub async fn detach_in(
    state: &AppState,
    conn: &mut PgConnection,
    ws_id: Uuid,
    user_id: Uuid,
    reason: Departure,
) -> AppResult<Detached> {
    let channels = state
        .workspace_service
        .repo
        .remove_member_in(conn, ws_id, user_id)
        .await?;
    let mut staged = Vec::with_capacity(channels.len() + 1);
    for channel_id in &channels {
        staged.push(
            state
                .publisher
                .stage_channel_member_removed(conn, *channel_id, ws_id, user_id)
                .await?,
        );
    }
    staged.push(
        state
            .publisher
            .stage_workspace_member_removed(conn, ws_id, user_id, reason)
            .await?,
    );
    Ok(Detached { channels, staged })
}

impl Detached {
    pub async fn settle(self, state: &AppState, ws_id: Uuid, user_id: Uuid) -> Vec<Uuid> {
        if let Err(e) = state
            .scheduled_repo
            .cancel_pending_for_workspace(ws_id, user_id)
            .await
        {
            tracing::warn!(
                "failed to cancel scheduled messages of a removed member: {}",
                e
            );
        }
        state.publisher.dispatch_all(self.staged).await;
        stand_down(state, Some(ws_id), user_id, Stopped::LeftWorkspace)
            .await
            .audit(state, Stopped::LeftWorkspace, None, &ClientIp(None))
            .await;
        self.channels
    }
}

pub struct StoodDown {
    pub hooks: Vec<DisabledHook>,
    pub workflows: Vec<StoppedWorkflow>,
    pub apps: Vec<DisabledApp>,
    pub invites: Vec<RevokedInvite>,
}

pub async fn stand_down(
    state: &AppState,
    workspace_id: Option<Uuid>,
    user_id: Uuid,
    why: Stopped,
) -> StoodDown {
    let hooks = state
        .hook_repo
        .disable_by_creator(workspace_id, user_id, why.reason())
        .await
        .unwrap_or_else(|e| {
            tracing::warn!("could not turn off the hooks of a stopped creator: {}", e);
            Vec::new()
        });
    let workflows =
        crate::workflows::service::stop_creators_workflows(state, workspace_id, user_id, why).await;
    let (apps, invites) = if matches!(why, Stopped::Inactive | Stopped::LeftWorkspace) {
        let apps = state
            .bot_repo
            .disable_by_creator(workspace_id, user_id, why.reason())
            .await
            .unwrap_or_else(|e| {
                tracing::warn!("could not turn off the apps of a stopped creator: {}", e);
                Vec::new()
            });
        let invites = state
            .workspace_service
            .repo
            .delete_link_invites_by(workspace_id, user_id)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!(
                    "could not revoke the invite links of a stopped creator: {}",
                    e
                );
                Vec::new()
            });
        (apps, invites)
    } else {
        (Vec::new(), Vec::new())
    };
    StoodDown {
        hooks,
        workflows,
        apps,
        invites,
    }
}

impl StoodDown {
    pub async fn audit(self, state: &AppState, why: Stopped, actor: Option<Uuid>, ip: &ClientIp) {
        let entry = |action| match actor {
            Some(actor) => AuditEntry::new(action, actor),
            None => AuditEntry::machine(action),
        };
        let reason = why.reason();
        for hook in self.hooks {
            audit::record(
                state,
                entry(AuditAction::HookDisabled)
                    .workspace(hook.workspace_id)
                    .resource(hook.id)
                    .ip(ip)
                    .details(serde_json::json!({ "name": hook.name, "reason": reason })),
            )
            .await;
        }
        for workflow in self.workflows {
            audit::record(
                state,
                entry(AuditAction::WorkflowDisabled)
                    .workspace(workflow.workspace_id)
                    .resource(workflow.id)
                    .ip(ip)
                    .details(serde_json::json!({ "name": workflow.name, "reason": reason })),
            )
            .await;
        }
        for app in self.apps {
            audit::record(
                state,
                entry(AuditAction::BotAppDisabled)
                    .workspace(app.workspace_id)
                    .resource(app.id)
                    .ip(ip)
                    .details(serde_json::json!({ "name": app.name, "reason": reason })),
            )
            .await;
        }
        for invite in self.invites {
            audit::record(
                state,
                entry(AuditAction::InviteRevoked)
                    .workspace(invite.workspace_id)
                    .resource(invite.id)
                    .ip(ip)
                    .details(serde_json::json!({ "reason": reason })),
            )
            .await;
        }
    }
}

pub async fn suspend_creations(state: &AppState, user_id: Uuid, actor: Uuid, ip: &ClientIp) {
    stand_down(state, None, user_id, Stopped::Inactive)
        .await
        .audit(state, Stopped::Inactive, Some(actor), ip)
        .await;
}
