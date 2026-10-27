use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

pub use crate::dto::{DataList, StatusResponse};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
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
    pub hook_id: Option<Uuid>,
    pub workflow_id: Option<Uuid>,
    pub name: String,
    pub icon_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MessageMetadata {
    pub kind: Option<String>,
    pub huddle_id: Option<Uuid>,
    pub initiator_id: Option<Uuid>,
    pub bot: Option<BotIdentity>,
    pub unfurls: Option<Vec<Unfurl>>,
    pub unfurls_hidden: Option<bool>,
    pub app_prompts: Option<Vec<String>>,
    pub attachments: Option<Vec<crate::files::models::Attachment>>,
    pub poll: Option<crate::polls::models::PollSummary>,
    pub form: Option<crate::workflows::models::FormRequest>,
    pub fields: Option<Vec<MessageField>>,
    pub also_in_channel: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct MessageField {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Unfurl {
    pub url: String,
    pub title: String,
    pub description: Option<String>,
    pub site_name: Option<String>,
    pub image_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<UnfurlField>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fetched_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embed: Option<VideoEmbed>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct VideoEmbed {
    pub provider: String,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct UnfurlField {
    pub label: String,
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UnfurlTarget {
    pub workspace_id: Uuid,
    pub poster_id: Uuid,
    pub content: String,
    pub metadata: serde_json::Value,
    pub previews_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MessageEdit {
    pub id: Uuid,
    pub message_id: Uuid,
    pub previous_content: String,
    pub edited_by: Uuid,
    pub edited_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
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
    #[garde(custom(shared_common::validation::rules::message_body))]
    pub content: String,
    pub thread_parent_id: Option<Uuid>,
    #[serde(default)]
    pub also_to_channel: bool,
    /// The sender's own id for this send, used only to make a retry idempotent.
    /// It is not the message id — the server owns that.
    #[garde(inner(custom(shared_common::validation::rules::client_message_id)))]
    pub client_message_id: Option<Uuid>,
    #[serde(default)]
    pub attachment_ids: Vec<Uuid>,
    pub copy_attachments_from: Option<Uuid>,
}

#[derive(Debug, Deserialize, garde::Validate, ToSchema)]
#[garde(allow_unvalidated)]
pub struct UpdateMessageRequest {
    #[garde(custom(shared_common::validation::rules::message_body))]
    pub content: String,
    pub attachment_ids: Option<Vec<Uuid>>,
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

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ReadState {
    pub channel_id: Uuid,
    pub unread_count: i32,
    pub mention_count: i32,
    pub last_read_message_id: Option<Uuid>,
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
    #[param(inline)]
    pub has: Option<SearchHas>,
    pub after: Option<DateTime<Utc>>,
    pub before: Option<DateTime<Utc>>,
}

impl SearchQuery {
    pub fn has_filter(&self) -> bool {
        self.channel_id.is_some()
            || self.user_id.is_some()
            || self.has.is_some()
            || self.after.is_some()
            || self.before.is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum SearchHas {
    File,
    Image,
    Link,
}

impl SearchHas {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Image => "image",
            Self::Link => "link",
        }
    }
}

/// A direct message is a channel too, but somebody searching "the channels" is
/// not asking to see their private threads next to them, so the scope picks
/// which channel types the hits may come from. Filtering by one channel or
/// conversation names the place, whatever the scope says.
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
pub struct UnreadsQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct UnreadChannelRow {
    pub channel_id: Uuid,
    pub channel_name: Option<String>,
    pub channel_type: crate::workspace::models::ChannelType,
    pub muted: bool,
    pub unread_count: i32,
    pub mention_count: i32,
    pub last_activity_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UnreadGroup {
    pub channel_id: Uuid,
    pub channel_name: Option<String>,
    pub channel_type: crate::workspace::models::ChannelType,
    pub muted: bool,
    pub unread_count: i32,
    pub mention_count: i32,
    pub last_activity_at: DateTime<Utc>,
    pub messages: Vec<Message>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UnreadsPage {
    pub data: Vec<UnreadGroup>,
    pub next_offset: Option<i64>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListMessagesQuery {
    pub limit: Option<i64>,
    pub cursor: Option<Uuid>,
    pub mark_read: Option<bool>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ThreadQuery {
    pub limit: Option<i64>,
    pub cursor: Option<Uuid>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MessagePage {
    pub data: Vec<MessageWithReactions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unread_notifications: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MessageWithReactions {
    #[serde(flatten)]
    pub message: Message,
    pub reactions: Vec<Reaction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum LocationKind {
    Channel,
    Dm,
    GroupDm,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MessageLocation {
    pub workspace_id: Uuid,
    pub channel_id: Uuid,
    pub kind: LocationKind,
    pub thread_parent_id: Option<Uuid>,
    pub joined: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SearchResponse {
    pub data: Vec<Message>,
}

pub struct NewMessage<'a> {
    pub channel_id: Uuid,
    pub user_id: Uuid,
    pub content: &'a str,
    pub thread_parent_id: Option<Uuid>,
    pub also_in_channel: bool,
    pub client_message_id: Option<Uuid>,
    pub mentioned: &'a [Uuid],
}
