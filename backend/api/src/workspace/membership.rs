use uuid::Uuid;

use shared_common::errors::AppResult;

use crate::state::AppState;

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

    Ok(dropped_channels)
}
