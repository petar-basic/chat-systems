use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::workspace::models::ChannelType;

/// A direct message is the two-person case of a group; both are channels of a
/// type nobody can browse or join, and this is the name the client knows them by.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ConversationKind {
    Direct,
    Group,
}

impl ConversationKind {
    pub fn channel_type(self) -> ChannelType {
        match self {
            Self::Direct => ChannelType::Dm,
            Self::Group => ChannelType::GroupDm,
        }
    }

    pub fn of(channel_type: &ChannelType) -> Option<Self> {
        match channel_type {
            ChannelType::Dm => Some(Self::Direct),
            ChannelType::GroupDm => Some(Self::Group),
            ChannelType::Public | ChannelType::Private => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Conversation {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub kind: ConversationKind,
    pub created_by: Option<Uuid>,
    pub last_message_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ConversationSummary {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub kind: ConversationKind,
    pub last_message_at: DateTime<Utc>,
    pub last_read_at: Option<DateTime<Utc>>,
    pub participant_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateConversationRequest {
    pub participant_ids: Vec<Uuid>,
}
