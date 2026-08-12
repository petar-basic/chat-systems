use std::sync::Arc;

use tracing::{info, warn};
use uuid::Uuid;

use super::models::ScheduledMessage;
use crate::authz;
use crate::notifications::models::NotificationType;
use crate::state::AppState;

const TICK_SECS: u64 = 15;

/// Why a claimed message never went out. The reason reaches the author's client,
/// so it is a stable slug rather than a formatted error — the text of a database
/// message is not something a UI should be translating.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeliveryFailure {
    NotAuthorized,
    ChannelArchived,
    WorkspaceUnavailable,
    Internal(String),
}

impl DeliveryFailure {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NotAuthorized => "not_authorized",
            Self::ChannelArchived => "channel_archived",
            Self::WorkspaceUnavailable => "workspace_unavailable",
            Self::Internal(_) => "internal_error",
        }
    }

    fn author_message(&self) -> &'static str {
        match self {
            Self::NotAuthorized => "You no longer have access to the destination.",
            Self::ChannelArchived => "The channel has been archived.",
            Self::WorkspaceUnavailable => "The workspace is no longer available.",
            Self::Internal(_) => "The server could not deliver it.",
        }
    }

    /// An authorization failure is terminal: the claim already marked the row
    /// sent, and retrying a permission the author does not have never succeeds.
    fn tell_the_author(&self) -> bool {
        !matches!(self, Self::Internal(_))
    }
}

impl std::fmt::Display for DeliveryFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Internal(detail) => write!(f, "internal_error: {detail}"),
            other => write!(f, "{}", other.as_str()),
        }
    }
}

fn internal(e: impl std::fmt::Display) -> DeliveryFailure {
    DeliveryFailure::Internal(e.to_string())
}

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
            if let Err(failure) = deliver(&state, &scheduled).await {
                warn!(
                    scheduled_id = %scheduled.id,
                    "Scheduled message delivery failed: {}", failure
                );
                let _ = state
                    .scheduled_repo
                    .record_failure(scheduled.id, failure.as_str())
                    .await;
                if failure.tell_the_author() {
                    notify_author(&state, &scheduled, &failure).await;
                }
            }
        }
    }
}

#[cfg(test)]
pub(crate) async fn deliver_for_test(
    state: &AppState,
    scheduled: &ScheduledMessage,
) -> Result<(), DeliveryFailure> {
    let result = deliver(state, scheduled).await;
    if let Err(failure) = &result {
        let _ = state
            .scheduled_repo
            .record_failure(scheduled.id, failure.as_str())
            .await;
        if failure.tell_the_author() {
            notify_author(state, scheduled, failure).await;
        }
    }
    result
}

async fn deliver(state: &AppState, scheduled: &ScheduledMessage) -> Result<(), DeliveryFailure> {
    match (scheduled.channel_id, scheduled.conversation_id) {
        (Some(channel_id), None) => deliver_to_channel(state, scheduled, channel_id).await,
        (None, Some(conversation_id)) => {
            deliver_to_conversation(state, scheduled, conversation_id).await
        }
        _ => Err(DeliveryFailure::Internal(
            "scheduled message has no single target".into(),
        )),
    }
}

/// The message was authorized when it was scheduled, possibly days ago. Between
/// then and now the author may have left the channel, the workspace or the
/// company — so the same predicate the interactive path runs has to run again
/// here, at the moment of the effect.
async fn authorize_channel(
    state: &AppState,
    scheduled: &ScheduledMessage,
    channel_id: Uuid,
) -> Result<(), DeliveryFailure> {
    let access = authz::require_channel_access(state, channel_id, scheduled.user_id)
        .await
        .map_err(|_| DeliveryFailure::NotAuthorized)?;

    if access.channel.is_archived {
        return Err(DeliveryFailure::ChannelArchived);
    }

    let workspace = state
        .workspace_service
        .repo
        .find_workspace_by_id(access.channel.workspace_id)
        .await
        .map_err(internal)?;
    match workspace {
        Some(ws) if ws.is_active => Ok(()),
        _ => Err(DeliveryFailure::WorkspaceUnavailable),
    }
}

async fn deliver_to_channel(
    state: &AppState,
    scheduled: &ScheduledMessage,
    channel_id: Uuid,
) -> Result<(), DeliveryFailure> {
    authorize_channel(state, scheduled, channel_id).await?;

    let mentioned = crate::messaging::routes::expand_mentions(
        state,
        channel_id,
        scheduled.user_id,
        &scheduled.content,
    )
    .await;

    let message = state
        .message_repo
        .create_message(
            channel_id,
            scheduled.user_id,
            &scheduled.content,
            None,
            &mentioned,
        )
        .await
        .map_err(internal)?;

    crate::files::service::link_to_channel_message(
        state,
        &scheduled.content,
        message.id,
        scheduled.workspace_id,
        scheduled.user_id,
    )
    .await;

    let payload = serde_json::to_value(&message).map_err(internal)?;
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
) -> Result<(), DeliveryFailure> {
    authz::require_conversation_participant(state, conversation_id, scheduled.user_id)
        .await
        .map_err(|_| DeliveryFailure::NotAuthorized)?;

    let message = state
        .conversation_repo
        .create_message(
            Uuid::new_v4(),
            conversation_id,
            scheduled.user_id,
            &scheduled.content,
            None,
        )
        .await
        .map_err(internal)?;

    crate::files::service::link_to_conversation_message(
        state,
        &scheduled.content,
        message.id,
        scheduled.workspace_id,
        scheduled.user_id,
    )
    .await;

    let participants = state
        .conversation_repo
        .participant_ids(conversation_id)
        .await
        .map_err(internal)?;

    let mut payload = serde_json::to_value(&message).map_err(internal)?;
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

/// A message that silently evaporates is worse than one that fails loudly. The
/// author wrote it and expects it to exist somewhere.
async fn notify_author(state: &AppState, scheduled: &ScheduledMessage, failure: &DeliveryFailure) {
    let data = serde_json::json!({
        "scheduled_message_id": scheduled.id,
        "channel_id": scheduled.channel_id,
        "conversation_id": scheduled.conversation_id,
        "failure": failure.as_str(),
    });

    let result = state
        .notification_repo
        .create(
            scheduled.user_id,
            scheduled.workspace_id,
            &NotificationType::System,
            "Scheduled message was not delivered",
            Some(failure.author_message()),
            &data,
        )
        .await;

    if let Err(e) = result {
        warn!(
            scheduled_id = %scheduled.id,
            "could not tell the author their scheduled message failed: {}", e
        );
    }

    let event = serde_json::json!({
        "user_id": scheduled.user_id.to_string(),
        "workspace_id": scheduled.workspace_id.to_string(),
        "channel_id": scheduled.channel_id,
        "title": "Scheduled message was not delivered",
        "body": failure.author_message(),
        "priority": "mention",
    });
    let _ = state.publisher.publish("notification.push", event).await;
}
