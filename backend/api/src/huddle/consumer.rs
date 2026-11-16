use std::sync::Arc;

use tracing::{info, warn};
use uuid::Uuid;

use shared_common::errors::AppResult;

use crate::messaging::stream_group::StreamGroup;
use crate::state::AppState;
use crate::threads::models::ThreadState;
use crate::threads::routes::announce;

pub async fn start_consumer(state: Arc<AppState>) {
    let Some(mut group) = StreamGroup::connect(&state.config.redis_url, "huddle").await else {
        return;
    };
    info!("Huddle consumer started");
    loop {
        for delivery in group.next_batch().await {
            if let Err(e) =
                handle(&state, &delivery.event.event_type, &delivery.event.payload).await
            {
                warn!(event_id = %delivery.event.id, "huddle consumer: {e}");
            }
            group.ack(&delivery.key, &delivery.id).await;
        }
    }
}

pub async fn handle(
    state: &AppState,
    event_type: &str,
    payload: &serde_json::Value,
) -> AppResult<()> {
    let field = |name: &str| {
        payload
            .get(name)
            .and_then(|v| v.as_str())
            .and_then(|v| v.parse::<Uuid>().ok())
    };
    let (Some(huddle_id), Some(user_id)) = (field("huddle_id"), field("user_id")) else {
        return Ok(());
    };
    match event_type {
        "huddle.member_joined" => joined(state, huddle_id, user_id).await,
        "huddle.member_left" => left(state, huddle_id, user_id).await,
        _ => Ok(()),
    }
}

async fn joined(state: &AppState, huddle_id: Uuid, user_id: Uuid) -> AppResult<()> {
    state.huddle_repo.record_join(huddle_id, user_id).await?;
    let Some(session) = state.huddle_repo.find(huddle_id).await? else {
        return Ok(());
    };
    let Some(root) = session.message_id else {
        return Ok(());
    };
    let turned_on = state
        .thread_repo
        .follow_from_huddle(user_id, session.workspace_id, root)
        .await?;
    state
        .huddle_repo
        .set_followed_on_join(huddle_id, user_id, turned_on)
        .await?;
    if turned_on {
        let thread = ThreadState {
            root_message_id: root,
            following: true,
            unread_count: 0,
        };
        announce(state, user_id, session.workspace_id, &thread).await;
    }
    Ok(())
}

async fn left(state: &AppState, huddle_id: Uuid, user_id: Uuid) -> AppResult<()> {
    let remaining = state.huddle_repo.record_leave(huddle_id, user_id).await?;
    if let Some(session) = state.huddle_repo.find(huddle_id).await? {
        if let Some(root) = session.message_id {
            if state
                .huddle_repo
                .followed_on_join(huddle_id, user_id)
                .await?
            {
                if let Some(thread) = state
                    .thread_repo
                    .unfollow_unless_replied(user_id, root)
                    .await?
                {
                    announce(state, user_id, session.workspace_id, &thread).await;
                }
            }
        }
    }
    if remaining == 0 {
        end(state, huddle_id).await?;
    }
    Ok(())
}

async fn end(state: &AppState, huddle_id: Uuid) -> AppResult<()> {
    let Some(session) = state.huddle_repo.end_session(huddle_id).await? else {
        return Ok(());
    };
    if let (Some(root), Some(ended_at)) = (session.message_id, session.ended_at) {
        let participants = state.huddle_repo.participant_ids(huddle_id).await?;
        let mut tx = state.pool.begin().await?;
        let message = state
            .message_repo
            .end_huddle_in(&mut tx, root, ended_at, &participants)
            .await?;
        let staged = state
            .publisher
            .stage_message_updated(&mut tx, &message, session.workspace_id)
            .await?;
        tx.commit().await?;
        state.publisher.dispatch(staged).await;
    }
    state
        .publisher
        .publish(
            "huddle.ended",
            serde_json::json!({
                "huddle_id": session.id,
                "workspace_id": session.workspace_id,
                "channel_id": session.channel_id,
                "dm_partner_id": session.dm_partner_id,
                "initiator_id": session.initiated_by,
            }),
        )
        .await
}
