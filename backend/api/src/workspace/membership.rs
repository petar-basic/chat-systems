use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::{Channel, ChannelMember, ChannelRole};
use super::repo::WorkspaceRepo;
use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::authz::Stopped;
use crate::hooks::models::DisabledHook;
use crate::messaging::publisher::EventPublisher;
use crate::state::AppState;
use crate::workflows::repo::StoppedWorkflow;

pub async fn add_to_channel(
    state: &AppState,
    channel: &Channel,
    user_id: Uuid,
) -> AppResult<ChannelMember> {
    let mut tx = state.pool.begin().await?;
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
pub async fn detach(state: &AppState, ws_id: Uuid, user_id: Uuid) -> AppResult<Vec<Uuid>> {
    let mut tx = state.pool.begin().await?;
    let dropped_channels = state
        .workspace_service
        .repo
        .remove_member_in(&mut tx, ws_id, user_id)
        .await?;
    let mut staged = Vec::with_capacity(dropped_channels.len() + 1);
    for channel_id in &dropped_channels {
        staged.push(
            state
                .publisher
                .stage_channel_member_removed(&mut tx, *channel_id, ws_id, user_id)
                .await?,
        );
    }
    staged.push(
        state
            .publisher
            .stage_workspace_member_removed(&mut tx, ws_id, user_id)
            .await?,
    );
    tx.commit().await?;

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

    state.publisher.dispatch_all(staged).await;
    stand_down(state, Some(ws_id), user_id, Stopped::LeftWorkspace).await;

    Ok(dropped_channels)
}

pub async fn stand_down(
    state: &AppState,
    workspace_id: Option<Uuid>,
    user_id: Uuid,
    why: Stopped,
) -> (Vec<DisabledHook>, Vec<StoppedWorkflow>) {
    let hooks = state
        .hook_repo
        .disable_by_creator(workspace_id, user_id, why.reason())
        .await
        .unwrap_or_else(|e| {
            tracing::warn!("could not turn off the hooks of a stopped creator: {}", e);
            Vec::new()
        });
    let because = match why {
        Stopped::Inactive => "the person who built it was suspended",
        Stopped::LeftWorkspace | Stopped::LostChannel => {
            "the person who built it left the workspace"
        }
        Stopped::Demoted => "the person who built it is no longer allowed to",
    };
    let workflows =
        crate::workflows::service::stop_creators_workflows(state, workspace_id, user_id, because)
            .await;
    (hooks, workflows)
}

pub async fn suspend_creations(state: &AppState, user_id: Uuid, actor: Uuid, ip: &ClientIp) {
    let (hooks, workflows) = stand_down(state, None, user_id, Stopped::Inactive).await;
    for hook in hooks {
        audit::record(
            state,
            AuditEntry::new(AuditAction::HookDisabled, actor)
                .workspace(hook.workspace_id)
                .resource(hook.id)
                .ip(ip)
                .details(serde_json::json!({
                    "name": hook.name,
                    "reason": Stopped::Inactive.reason(),
                })),
        )
        .await;
    }
    for workflow in workflows {
        audit::record(
            state,
            AuditEntry::new(AuditAction::WorkflowDisabled, actor)
                .workspace(workflow.workspace_id)
                .resource(workflow.id)
                .ip(ip)
                .details(serde_json::json!({
                    "name": workflow.name,
                    "reason": Stopped::Inactive.reason(),
                })),
        )
        .await;
    }
}
