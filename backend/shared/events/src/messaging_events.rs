use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageCreated {
    pub message_id: Uuid,
    pub channel_id: Uuid,
    pub user_id: Uuid,
    pub content: String,
    pub workspace_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageUpdated {
    pub message_id: Uuid,
    pub channel_id: Uuid,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageDeleted {
    pub message_id: Uuid,
    pub channel_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactionAdded {
    pub message_id: Uuid,
    pub channel_id: Uuid,
    pub user_id: Uuid,
    pub emoji: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactionRemoved {
    pub message_id: Uuid,
    pub channel_id: Uuid,
    pub user_id: Uuid,
    pub emoji: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagePinned {
    pub message_id: Uuid,
    pub channel_id: Uuid,
    pub pinned: bool,
}
