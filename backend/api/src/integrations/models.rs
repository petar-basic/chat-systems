use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Jira,
}

impl Provider {
    pub const ALL: [Provider; 1] = [Provider::Jira];

    pub fn as_str(self) -> &'static str {
        match self {
            Provider::Jira => "jira",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.as_str() == value)
    }

    pub fn name(self) -> &'static str {
        match self {
            Provider::Jira => "Jira Cloud",
        }
    }

    pub fn scopes(self) -> &'static [&'static str] {
        match self {
            Provider::Jira => &["read:jira-work", "read:me", "offline_access"],
        }
    }

    pub fn console_url(self) -> &'static str {
        match self {
            Provider::Jira => "https://developer.atlassian.com/console/myapps/",
        }
    }

    pub fn setup_steps(self) -> &'static [&'static str] {
        match self {
            Provider::Jira => &[
                "Open the Atlassian developer console and choose Create → OAuth 2.0 integration.",
                "Under Permissions, add the Jira API and grant read:jira-work; add the User identity API for read:me.",
                "Under Authorization, choose OAuth 2.0 (3LO) and paste the callback URL below.",
                "Under Settings, copy the Client ID and the Secret into the fields below and save.",
                "Press Connect my account to test it: Atlassian asks for consent and you come back connected.",
            ],
        }
    }
}

#[derive(Debug, Clone)]
pub struct WorkspaceApp {
    pub workspace_id: Uuid,
    pub provider: String,
    pub client_id: String,
    pub client_secret_encrypted: Vec<u8>,
    pub client_secret_nonce: Vec<u8>,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct AppConnection {
    pub external_account: String,
    pub access_token_encrypted: Vec<u8>,
    pub access_token_nonce: Vec<u8>,
    pub refresh_token_encrypted: Option<Vec<u8>>,
    pub refresh_token_nonce: Option<Vec<u8>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub resources: serde_json::Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AppStatus {
    pub provider: Provider,
    pub name: String,
    pub configured: bool,
    pub enabled: bool,
    pub client_id: Option<String>,
    pub redirect_url: String,
    pub scopes: Vec<String>,
    pub console_url: String,
    pub setup_steps: Vec<String>,
    pub connected_account: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AppList {
    pub data: Vec<AppStatus>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ConfigureAppRequest {
    pub client_id: String,
    pub client_secret: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ConnectAppRequest {
    pub return_to: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ConnectAppResponse {
    pub authorize_url: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CompleteConnectionRequest {
    pub code: String,
    pub state: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CompleteConnectionResponse {
    pub workspace_id: Uuid,
    pub provider: Provider,
    pub external_account: String,
    pub return_to: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingConnection {
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub provider: Provider,
    pub code_verifier: String,
    pub return_to: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JiraSite {
    pub id: String,
    pub url: String,
}
