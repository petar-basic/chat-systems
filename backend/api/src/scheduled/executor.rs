use std::sync::Arc;

use tracing::{info, warn};
use uuid::Uuid;

use super::models::ScheduledMessage;
use crate::authz;
use crate::messaging::models::NewMessage;
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
    deliver_to_channel(state, scheduled, scheduled.channel_id).await
}

/// The message was authorized when it was scheduled, possibly days ago. Between
/// then and now the author may have left the channel, the workspace or the
/// company, or the channel may have been made announcement-only — so the same
/// predicate the interactive path runs has to run again here, at the moment of
/// the effect.
async fn authorize_channel(
    state: &AppState,
    scheduled: &ScheduledMessage,
    channel_id: Uuid,
) -> Result<(), DeliveryFailure> {
    let access = authz::require_channel_post(state, channel_id, scheduled.user_id)
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

    let mut tx = state.pool.begin().await.map_err(internal)?;
    let message = state
        .message_repo
        .create_message_in(
            &mut tx,
            NewMessage {
                channel_id,
                user_id: scheduled.user_id,
                content: &scheduled.content,
                thread_parent_id: None,
                client_message_id: None,
                mentioned: &mentioned,
            },
        )
        .await
        .map_err(internal)?;
    let staged = state
        .publisher
        .stage_message_created(&mut tx, &message, scheduled.workspace_id, &mentioned)
        .await
        .map_err(internal)?;
    tx.commit().await.map_err(internal)?;

    crate::files::service::link_to_channel_message(
        state,
        &scheduled.content,
        message.id,
        scheduled.workspace_id,
        scheduled.user_id,
    )
    .await;

    state.publisher.dispatch(staged).await;

    Ok(())
}

/// A message that silently evaporates is worse than one that fails loudly. The
/// author wrote it and expects it to exist somewhere.
async fn notify_author(state: &AppState, scheduled: &ScheduledMessage, failure: &DeliveryFailure) {
    let data = serde_json::json!({
        "scheduled_message_id": scheduled.id,
        "channel_id": scheduled.channel_id,
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
