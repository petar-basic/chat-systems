use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ScheduledMessage {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub channel_id: Option<Uuid>,
    pub conversation_id: Option<Uuid>,
    pub content: String,
    pub send_at: DateTime<Utc>,
    pub sent_at: Option<DateTime<Utc>>,
    pub canceled_at: Option<DateTime<Utc>>,
    pub failure: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, garde::Validate)]
#[garde(allow_unvalidated)]
pub struct CreateScheduledMessageRequest {
    pub channel_id: Option<Uuid>,
    pub conversation_id: Option<Uuid>,
    #[garde(custom(shared_common::validation::rules::message_content))]
    pub content: String,
    #[garde(custom(send_at_in_window))]
    pub send_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, garde::Validate)]
#[garde(allow_unvalidated)]
pub struct RescheduleRequest {
    #[garde(custom(send_at_in_window))]
    pub send_at: DateTime<Utc>,
}

pub const MAX_SCHEDULE_AHEAD_DAYS: i64 = 120;

fn send_at_in_window(value: &DateTime<Utc>, _: &()) -> garde::Result {
    let now = Utc::now();
    if *value <= now {
        return Err(garde::Error::new("Scheduled time must be in the future"));
    }
    if *value > now + chrono::Duration::days(MAX_SCHEDULE_AHEAD_DAYS) {
        return Err(garde::Error::new(format!(
            "Messages can be scheduled at most {MAX_SCHEDULE_AHEAD_DAYS} days ahead"
        )));
    }
    Ok(())
}
