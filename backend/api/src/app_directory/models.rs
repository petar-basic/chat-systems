use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum InstalledKind {
    ConnectedApp,
    BotApp,
    IncomingWebhook,
    OutgoingWebhook,
    SlashCommand,
    Workflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum HealthState {
    Ok,
    Off,
    Disabled,
    Failing,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Health {
    pub state: HealthState,
    pub reason: Option<String>,
    pub since: Option<DateTime<Utc>>,
}

impl Health {
    pub fn ok() -> Self {
        Health {
            state: HealthState::Ok,
            reason: None,
            since: None,
        }
    }

    pub fn off() -> Self {
        Health {
            state: HealthState::Off,
            reason: None,
            since: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Permission {
    pub key: String,
    pub text: String,
}

impl Permission {
    pub fn new(key: &str, text: impl Into<String>) -> Self {
        Permission {
            key: key.to_string(),
            text: text.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DirectoryChannel {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct InstalledApp {
    pub kind: InstalledKind,
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub added_by: Option<Uuid>,
    pub added_by_name: Option<String>,
    pub added_at: DateTime<Utc>,
    pub permissions: Vec<Permission>,
    pub every_channel: bool,
    pub channels: Vec<DirectoryChannel>,
    pub hidden_channels: i64,
    pub health: Health,
    pub app_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AvailableApp {
    pub provider: String,
    pub name: String,
    pub summary: String,
    pub permissions: Vec<Permission>,
    pub installed: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AppDirectory {
    pub can_manage: bool,
    pub available: Vec<AvailableApp>,
    pub installed: Vec<InstalledApp>,
    pub bot_scopes: Vec<Permission>,
}

#[derive(Debug, Deserialize, garde::Validate, ToSchema)]
pub struct AppRequest {
    #[garde(custom(shared_common::validation::rules::hook_name))]
    pub name: String,
    #[garde(inner(custom(shared_common::validation::rules::description)))]
    pub reason: Option<String>,
}

pub struct ConnectedAppRow {
    pub provider: String,
    pub enabled: bool,
    pub configured_by: Option<Uuid>,
    pub configured_by_name: Option<String>,
    pub configured_at: DateTime<Utc>,
}

pub struct BotAppRow {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub scopes: Vec<String>,
    pub created_by: Option<Uuid>,
    pub created_by_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub failing_since: Option<DateTime<Utc>>,
    pub last_failure: Option<String>,
    pub channel_ids: Vec<Uuid>,
}

pub struct HookRow {
    pub id: Uuid,
    pub hook_type: crate::hooks::models::HookType,
    pub name: String,
    pub description: Option<String>,
    pub config: serde_json::Value,
    pub is_active: bool,
    pub disabled_reason: Option<String>,
    pub app_id: Option<Uuid>,
    pub created_by: Uuid,
    pub created_by_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct WorkflowRow {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub definition: serde_json::Value,
    pub enabled: bool,
    pub created_by: Uuid,
    pub created_by_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_status: Option<crate::workflows::models::RunStatus>,
    pub last_error: Option<String>,
    pub last_started_at: Option<DateTime<Utc>>,
}

pub struct ChannelRow {
    pub id: Uuid,
    pub name: String,
    pub visible: bool,
}
