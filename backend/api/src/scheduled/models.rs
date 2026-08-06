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

#[derive(Debug, Deserialize)]
pub struct CreateScheduledMessageRequest {
    pub channel_id: Option<Uuid>,
    pub conversation_id: Option<Uuid>,
    pub content: String,
    pub send_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct RescheduleRequest {
    pub send_at: DateTime<Utc>,
}
