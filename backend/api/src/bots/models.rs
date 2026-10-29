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
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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
}
