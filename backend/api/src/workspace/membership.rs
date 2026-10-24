use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::{Channel, ChannelMember, ChannelRole};
use super::repo::WorkspaceRepo;
use crate::messaging::publisher::EventPublisher;
use crate::state::AppState;

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
    crate::workflows::service::creator_left(state, ws_id, user_id).await;

    Ok(dropped_channels)
}
