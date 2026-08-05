use std::sync::Arc;

use tracing::{info, warn};
use uuid::Uuid;

use super::models::ScheduledMessage;
use crate::state::AppState;

const TICK_SECS: u64 = 15;

pub async fn start_dispatcher(state: Arc<AppState>) {
    info!("Scheduled message dispatcher started");

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(TICK_SECS)).await;

        let due = match state.scheduled_repo.claim_due().await {
            Ok(due) => due,
            Err(e) => {
                warn!("Failed to claim due scheduled messages: {}", e);
                continue;
            }
        };

        for scheduled in due {
            if let Err(e) = deliver(&state, &scheduled).await {
                warn!(
                    scheduled_id = %scheduled.id,
                    "Scheduled message delivery failed: {}", e
                );
                let _ = state
                    .scheduled_repo
                    .record_failure(scheduled.id, &e.to_string())
                    .await;
            }
        }
    }
}

#[cfg(test)]
pub(crate) async fn deliver_for_test(
    state: &AppState,
    scheduled: &ScheduledMessage,
) -> Result<(), String> {
    deliver(state, scheduled).await
}

async fn deliver(state: &AppState, scheduled: &ScheduledMessage) -> Result<(), String> {
    match (scheduled.channel_id, scheduled.conversation_id) {
        (Some(channel_id), None) => deliver_to_channel(state, scheduled, channel_id).await,
        (None, Some(conversation_id)) => {
            deliver_to_conversation(state, scheduled, conversation_id).await
        }
        _ => Err("scheduled message has no single target".into()),
    }
}

async fn deliver_to_channel(
    state: &AppState,
    scheduled: &ScheduledMessage,
    channel_id: Uuid,
) -> Result<(), String> {
    let message = state
        .message_repo
        .create_message(channel_id, scheduled.user_id, &scheduled.content, None)
        .await
        .map_err(|e| e.to_string())?;

    let mentioned = crate::messaging::routes::expand_mentions(
        state,
        channel_id,
        scheduled.user_id,
        &scheduled.content,
    )
    .await;

    let payload = serde_json::to_value(&message).map_err(|e| e.to_string())?;
    let _ = state
        .publisher
        .publish_message_created(&payload, scheduled.workspace_id, &mentioned)
        .await;

    Ok(())
}

async fn deliver_to_conversation(
    state: &AppState,
    scheduled: &ScheduledMessage,
    conversation_id: Uuid,
) -> Result<(), String> {
    let message = state
        .conversation_repo
        .create_message(
            Uuid::new_v4(),
            conversation_id,
            scheduled.user_id,
            &scheduled.content,
        )
        .await
        .map_err(|e| e.to_string())?;

    let participants = state
        .conversation_repo
        .participant_ids(conversation_id)
        .await
        .map_err(|e| e.to_string())?;

    let mut payload = serde_json::to_value(&message).map_err(|e| e.to_string())?;
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("conversation_id".into(), serde_json::json!(conversation_id));
        obj.insert(
            "workspace_id".into(),
            serde_json::json!(scheduled.workspace_id),
        );
        obj.insert("participant_ids".into(), serde_json::json!(participants));
    }
    let _ = state
        .publisher
        .publish("conversation.message.created", payload)
        .await;

    Ok(())
}
