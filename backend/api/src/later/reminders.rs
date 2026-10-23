use std::sync::Arc;

use tracing::{info, warn};

use super::models::DueReminder;
use crate::state::AppState;

const CHECK_EVERY: std::time::Duration = std::time::Duration::from_secs(30);

/// A reminder names a channel, and the target may have lost access to it since
/// it was written. Delivering it anyway leaks the channel's existence and, with
/// a message link, a route into a conversation they can no longer read.
pub async fn is_deliverable(state: &AppState, reminder: &DueReminder) -> bool {
    let Some(channel_id) = reminder.channel_id else {
        return true;
    };
    crate::authz::require_channel_access(state, channel_id, reminder.user_id)
        .await
        .is_ok()
}

pub async fn start_checker(state: Arc<AppState>) {
    info!("Reminder checker started");
    loop {
        tokio::time::sleep(CHECK_EVERY).await;
        let due = match state.later_repo.claim_due().await {
            Ok(due) => due,
            Err(e) => {
                warn!("Failed to claim due reminders: {e}");
                continue;
            }
        };
        for reminder in due {
            deliver(&state, &reminder).await;
        }
    }
}

async fn deliver(state: &AppState, reminder: &DueReminder) {
    if !is_deliverable(state, reminder).await {
        warn!(
            reminder_id = %reminder.id,
            "reminder dropped: the target no longer has access to the channel"
        );
        return;
    }
    let body = reminder
        .text
        .clone()
        .unwrap_or_else(|| "A message you saved for later".to_string());
    let event = serde_json::json!({
        "user_id": reminder.user_id.to_string(),
        "workspace_id": reminder.workspace_id.to_string(),
        "channel_id": reminder.channel_id,
        "message_id": reminder.message_id,
        "later_id": reminder.id,
        "title": "Reminder",
        "body": body,
        "kind": "reminder",
    });
    // The claim already stamped it; put it back if it never went out, so a
    // claim followed by a failure is a retry rather than a loss.
    if let Err(e) = state.publisher.publish("notification.push", event).await {
        warn!("failed to publish reminder notification: {e}");
        if let Err(e) = state.later_repo.release(reminder.id).await {
            warn!("failed to release undelivered reminder: {e}");
        }
        return;
    }
    super::routes::announce(state, reminder.user_id, reminder.workspace_id).await;
}
