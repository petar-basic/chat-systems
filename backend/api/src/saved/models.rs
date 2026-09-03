use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SavedMessage {
    pub id: Uuid,
    pub user_id: Uuid,
    pub workspace_id: Uuid,
    pub message_id: Uuid,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// A saved row plus the message it points at, so the panel can render without a
/// second round trip per item.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SavedMessageDetail {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub message_id: Uuid,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
    pub channel_id: Uuid,
    pub author_id: Uuid,
    pub content: String,
    pub sent_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SaveMessageRequest {
    pub message_id: Uuid,
    pub note: Option<String>,
}
