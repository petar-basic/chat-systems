use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "hook_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum HookType {
    IncomingWebhook,
    OutgoingWebhook,
    Bot,
    SlashCommand,
    Scheduled,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Hook {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub created_by: Uuid,
    pub hook_type: HookType,
    pub name: String,
    pub description: Option<String>,
    pub config: serde_json::Value,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct HookExecution {
    pub id: Uuid,
    pub hook_id: Uuid,
    pub event_type: Option<String>,
    pub payload: Option<serde_json::Value>,
    pub response_status: Option<i32>,
    pub response_body: Option<String>,
    pub executed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Reminder {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub created_by: Uuid,
    pub target_user_id: Uuid,
    pub channel_id: Option<Uuid>,
    pub message_id: Option<Uuid>,
    pub content: String,
    pub remind_at: DateTime<Utc>,
    pub is_delivered: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateHookRequest {
    pub hook_type: HookType,
    pub name: String,
    pub description: Option<String>,
    pub config: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct CreateReminderRequest {
    pub target_user_id: Uuid,
    pub channel_id: Option<Uuid>,
    pub message_id: Option<Uuid>,
    pub content: String,
    pub remind_at: DateTime<Utc>,
}
