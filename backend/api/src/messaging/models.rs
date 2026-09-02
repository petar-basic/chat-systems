use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::conversations::models::ConversationMessage;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct Message {
    pub id: Uuid,
    pub channel_id: Uuid,
    pub user_id: Uuid,
    pub client_message_id: Option<Uuid>,
    pub content: String,
    #[schema(value_type = MessageMetadata)]
    pub metadata: serde_json::Value,
    pub thread_parent_id: Option<Uuid>,
    pub reply_count: i32,
    pub is_pinned: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BotIdentity {
    pub hook_id: Uuid,
    pub name: String,
    pub icon_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MessageMetadata {
    pub kind: Option<String>,
    pub huddle_id: Option<Uuid>,
    pub initiator_id: Option<Uuid>,
    pub bot: Option<BotIdentity>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct MessageEdit {
    pub id: Uuid,
    pub message_id: Uuid,
    pub previous_content: String,
    pub edited_by: Uuid,
    pub edited_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct Reaction {
    pub id: Uuid,
    pub message_id: Uuid,
    pub user_id: Uuid,
    pub emoji: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, garde::Validate, ToSchema)]
#[garde(allow_unvalidated)]
pub struct SendMessageRequest {
    #[garde(custom(shared_common::validation::rules::message_content))]
    pub content: String,
    pub thread_parent_id: Option<Uuid>,
    /// The sender's own id for this send, used only to make a retry idempotent.
    /// It is not the message id — the server owns that.
    #[garde(inner(custom(shared_common::validation::rules::client_message_id)))]
    pub client_message_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, garde::Validate, ToSchema)]
#[garde(allow_unvalidated)]
pub struct UpdateMessageRequest {
    #[garde(custom(shared_common::validation::rules::message_content))]
    pub content: String,
}

#[derive(Debug, Deserialize, garde::Validate, ToSchema)]
#[garde(allow_unvalidated)]
pub struct AddReactionRequest {
    #[garde(custom(shared_common::validation::rules::reaction_emoji))]
    pub emoji: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MarkReadRequest {
    pub message_id: Uuid,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct SearchQuery {
    pub q: Option<String>,
    pub workspace_id: Uuid,
    pub channel_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub scope: Option<SearchScope>,
}

/// Channels and conversations are different resources with different visibility
/// rules, so they are returned as two lists rather than merged into one. A
/// caller that filters by channel is asking about channels and gets nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum SearchScope {
    Channels,
    Conversations,
    All,
}

impl SearchScope {
    pub fn includes_channels(self) -> bool {
        matches!(self, Self::Channels | Self::All)
    }

    pub fn includes_conversations(self) -> bool {
        matches!(self, Self::Conversations | Self::All)
    }
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListMessagesQuery {
    pub limit: Option<i64>,
    pub cursor: Option<Uuid>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MessageWithReactions {
    #[serde(flatten)]
    pub message: Message,
    pub reactions: Vec<Reaction>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DataList<T: ToSchema> {
    pub data: Vec<T>,
}

impl<T: ToSchema> From<Vec<T>> for DataList<T> {
    fn from(data: Vec<T>) -> Self {
        Self { data }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct StatusResponse {
    pub status: &'static str,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SearchResponse {
    pub data: Vec<Message>,
    pub conversations: Vec<ConversationMessage>,
}
