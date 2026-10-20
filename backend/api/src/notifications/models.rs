use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq, ToSchema)]
#[sqlx(type_name = "notification_type", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum NotificationType {
    Mention,
    Dm,
    Reply,
    Reaction,
    Call,
    Reminder,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Notification {
    pub id: Uuid,
    pub user_id: Uuid,
    pub workspace_id: Uuid,
    pub notification_type: NotificationType,
    pub title: String,
    pub body: Option<String>,
    pub data: serde_json::Value,
    pub is_read: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MarkNotificationsReadRequest {
    pub notification_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SetDndRequest {
    pub dnd_until: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct EmailPreferenceRequest {
    pub mention_emails: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UpdatedCount {
    pub updated: u64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UnreadCountResponse {
    pub unread_count: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DndResponse {
    pub dnd_until: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct EmailPreference {
    pub mention_emails: bool,
    pub available: bool,
}
