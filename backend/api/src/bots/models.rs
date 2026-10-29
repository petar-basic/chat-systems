use chrono::{DateTime, Utc};
use garde::Validate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub enum Scope {
    #[serde(rename = "chat:write")]
    ChatWrite,
    #[serde(rename = "chat:write.public")]
    ChatWritePublic,
    #[serde(rename = "channels:read")]
    ChannelsRead,
    #[serde(rename = "channels:history")]
    ChannelsHistory,
    #[serde(rename = "groups:history")]
    GroupsHistory,
    #[serde(rename = "im:write")]
    ImWrite,
    #[serde(rename = "reactions:write")]
    ReactionsWrite,
    #[serde(rename = "files:write")]
    FilesWrite,
    #[serde(rename = "users:read")]
    UsersRead,
    #[serde(rename = "commands")]
    Commands,
}

impl Scope {
    pub const ALL: [Scope; 10] = [
        Scope::ChatWrite,
        Scope::ChatWritePublic,
        Scope::ChannelsRead,
        Scope::ChannelsHistory,
        Scope::GroupsHistory,
        Scope::ImWrite,
        Scope::ReactionsWrite,
        Scope::FilesWrite,
        Scope::UsersRead,
        Scope::Commands,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Scope::ChatWrite => "chat:write",
            Scope::ChatWritePublic => "chat:write.public",
            Scope::ChannelsRead => "channels:read",
            Scope::ChannelsHistory => "channels:history",
            Scope::GroupsHistory => "groups:history",
            Scope::ImWrite => "im:write",
            Scope::ReactionsWrite => "reactions:write",
            Scope::FilesWrite => "files:write",
            Scope::UsersRead => "users:read",
            Scope::Commands => "commands",
        }
    }

    pub fn parse(value: &str) -> Option<Scope> {
        Scope::ALL.into_iter().find(|s| s.as_str() == value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AppEvent {
    Message,
    AppMention,
    ReactionAdded,
    ReactionRemoved,
    MemberJoinedChannel,
    ChannelCreated,
    ChannelArchived,
}

impl AppEvent {
    pub const ALL: [AppEvent; 7] = [
        AppEvent::Message,
        AppEvent::AppMention,
        AppEvent::ReactionAdded,
        AppEvent::ReactionRemoved,
        AppEvent::MemberJoinedChannel,
        AppEvent::ChannelCreated,
        AppEvent::ChannelArchived,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            AppEvent::Message => "message",
            AppEvent::AppMention => "app_mention",
            AppEvent::ReactionAdded => "reaction_added",
            AppEvent::ReactionRemoved => "reaction_removed",
            AppEvent::MemberJoinedChannel => "member_joined_channel",
            AppEvent::ChannelCreated => "channel_created",
            AppEvent::ChannelArchived => "channel_archived",
        }
    }

    pub fn parse(value: &str) -> Option<AppEvent> {
        AppEvent::ALL.into_iter().find(|e| e.as_str() == value)
    }
}

#[derive(Debug, Clone)]
pub struct BotToken {
    pub app_id: Uuid,
    pub workspace_id: Uuid,
    pub scopes: Vec<Scope>,
}

impl BotToken {
    pub fn allows(&self, scope: Scope) -> bool {
        self.scopes.contains(&scope)
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct BotApp {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub bot_user_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub icon_url: Option<String>,
    pub scopes: Vec<Scope>,
    pub events: Vec<AppEvent>,
    pub request_url: Option<String>,
    pub socket_mode: bool,
    pub has_app_token: bool,
    pub connected_sockets: i64,
    pub failing_since: Option<DateTime<Utc>>,
    pub last_failure: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AppToken {
    pub app_id: Uuid,
    pub app_token: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SigningSecret {
    pub app_id: Uuid,
    pub signing_secret: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SocketUrl {
    pub url: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BotAppWithToken {
    #[serde(flatten)]
    pub app: BotApp,
    pub token: String,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateBotAppRequest {
    #[garde(custom(shared_common::validation::rules::hook_name))]
    pub name: String,
    #[garde(inner(custom(shared_common::validation::rules::description)))]
    pub description: Option<String>,
    #[garde(inner(custom(shared_common::validation::rules::icon_url_or_empty)))]
    pub icon_url: Option<String>,
    #[garde(skip)]
    pub scopes: Vec<Scope>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UpdateBotAppRequest {
    #[garde(inner(custom(shared_common::validation::rules::hook_name)))]
    pub name: Option<String>,
    #[garde(inner(custom(shared_common::validation::rules::description)))]
    pub description: Option<String>,
    #[garde(inner(custom(shared_common::validation::rules::icon_url_or_empty)))]
    pub icon_url: Option<String>,
    #[garde(skip)]
    pub scopes: Option<Vec<Scope>>,
    #[garde(skip)]
    pub events: Option<Vec<AppEvent>>,
    #[garde(inner(length(max = 2048)))]
    pub request_url: Option<String>,
    #[garde(skip)]
    pub socket_mode: Option<bool>,
}
