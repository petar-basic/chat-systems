use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::workspace_bots::models::WorkspaceBot;

pub const MAX_RESPONSES: i64 = 500;
pub const MAX_TRIGGERS: usize = 20;
pub const MAX_ANSWERS: usize = 20;
pub const MAX_TRIGGER: usize = 100;
pub const MAX_ANSWER: usize = 4000;
pub const COOL_DOWN_SECONDS: i32 = 60;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CustomResponse {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub triggers: Vec<String>,
    pub answers: Vec<String>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CustomResponses {
    pub data: Vec<CustomResponse>,
    pub can_edit: bool,
    pub can_rename: bool,
    pub bot: WorkspaceBot,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct NewCustomResponse {
    pub triggers: Vec<String>,
    pub answers: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateCustomResponse {
    pub triggers: Option<Vec<String>>,
    pub answers: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RenameResponsesBot {
    pub name: String,
}

pub struct HeardMessage {
    pub message_id: Uuid,
    pub workspace_id: Uuid,
    pub channel_id: Uuid,
    pub author_id: Uuid,
    pub content: String,
    pub thread_parent_id: Option<Uuid>,
    pub automated: bool,
}
