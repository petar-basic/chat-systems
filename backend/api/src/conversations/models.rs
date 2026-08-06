use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "conversation_kind", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ConversationKind {
    Direct,
    Group,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Conversation {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub kind: ConversationKind,
    pub created_by: Option<Uuid>,
    pub last_message_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ConversationSummary {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub kind: ConversationKind,
    pub last_message_at: DateTime<Utc>,
    pub last_read_at: Option<DateTime<Utc>>,
    pub participant_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ConversationMessage {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub user_id: Uuid,
    pub content: String,
    pub edited_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ConversationReaction {
    pub id: Uuid,
    pub message_id: Uuid,
    pub user_id: Uuid,
    pub emoji: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateConversationRequest {
    pub participant_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct SendConversationMessageRequest {
    pub content: String,
    pub id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct EditConversationMessageRequest {
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct AddConversationReactionRequest {
    pub emoji: String,
}
