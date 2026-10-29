use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Jira,
    Github,
    GoogleDrive,
    Figma,
    Confluence,
}

impl Provider {
    pub const ALL: [Provider; 5] = [
        Provider::Jira,
        Provider::Confluence,
        Provider::Github,
        Provider::GoogleDrive,
        Provider::Figma,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Provider::Jira => "jira",
            Provider::Github => "github",
            Provider::GoogleDrive => "google_drive",
            Provider::Figma => "figma",
            Provider::Confluence => "confluence",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.as_str() == value)
    }

    pub fn name(self) -> &'static str {
        match self {
            Provider::Jira => "Jira Cloud",
            Provider::Github => "GitHub",
            Provider::GoogleDrive => "Google Drive",
            Provider::Figma => "Figma",
            Provider::Confluence => "Confluence",
        }
    }

    pub fn summary(self) -> &'static str {
        match self {
            Provider::Jira => "Issues with their status, priority and assignee",
            Provider::Github => "Pull requests, issues, commits and repositories",
            Provider::GoogleDrive => "Docs, Sheets, Slides and files with their owner",
            Provider::Figma => "Designs, FigJam boards and Slides with a thumbnail",
            Provider::Confluence => "Pages and blog posts with their space, owner and last edit",
        }
    }

    pub fn scopes(self) -> &'static [&'static str] {
        match self {
            Provider::Jira => &["read:jira-work", "read:me", "offline_access"],
            Provider::Github => &[],
            Provider::GoogleDrive => &[super::google::SCOPE],
            Provider::Figma => &super::figma::SCOPES,
            Provider::Confluence => &super::confluence::SCOPES,
        }
    }

    pub fn permissions(self) -> &'static [&'static str] {
        match self {
            Provider::Jira => &[
                "See the issues and projects each person who connects can see",
                "See the name of the account that connected",
                "Stay connected until that person disconnects",
            ],
            Provider::Github => &[
                "See the pull requests, issues, commits and repositories each person who connects can see",
                "See the name of the account that connected",
            ],
            Provider::GoogleDrive => &[
                "See the names, owners and thumbnails of the files each person who connects can open, never what is in them",
            ],
            Provider::Figma => &[
                "See the names and thumbnails of the files each person who connects can open",
                "See the name of the account that connected",
            ],
            Provider::Confluence => &[
                "See the pages, blog posts and spaces each person who connects can see",
                "See who wrote and last edited them",
                "See the name of the account that connected",
                "Stay connected until that person disconnects",
            ],
        }
    }

    pub fn console_url(self) -> &'static str {
        match self {
            Provider::Jira => "https://developer.atlassian.com/console/myapps/",
            Provider::Github => "https://github.com/settings/apps/new",
            Provider::GoogleDrive => "https://console.cloud.google.com/apis/credentials",
            Provider::Figma => "https://www.figma.com/developers/apps",
            Provider::Confluence => "https://developer.atlassian.com/console/myapps/",
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
            Provider::Github => &[
                "Open GitHub → Settings → Developer settings → GitHub Apps → New GitHub App (under your organization's settings for an app the organization owns).",
                "Name it, set the Homepage URL to this instance and paste the callback URL below. Keep Expire user authorization tokens ticked.",
                "Under Webhook, untick Active — cards do not need events.",
                "Repository permissions: Contents, Issues, Pull requests and Metadata, all Read-only. Nothing else.",
                "Create the app, choose Generate a new client secret, and copy the Client ID and the secret into the fields below.",
                "Choose Install App and install it on your organization: private repositories get cards only where it is installed. Public repositories get cards without it.",
                "Press Connect my account to test it.",
            ],
            Provider::GoogleDrive => &[
                "In the Google Cloud console, create a project for this instance (or pick one) and enable the Google Drive API.",
                "Configure the OAuth consent screen. Choose Internal if your organization uses Google Workspace — no review needed and only your organization can connect. Add the scope below.",
                "Credentials → Create credentials → OAuth client ID → Web application, and add the callback URL below under Authorized redirect URIs.",
                "Copy the Client ID and Client secret into the fields below.",
                "Press Connect my account to test it.",
            ],
            Provider::Figma => &[
                "Open Figma's developer apps page and create a new app inside your organization's team.",
                "Under OAuth credentials, add the callback URL below as a redirect URL and copy the Client ID and Client secret into the fields below.",
                "Under OAuth scopes, tick the two scopes below and nothing else.",
                "Under Publish, choose Private: no Figma review, and only people in your organization can connect.",
                "Press Connect my account to test it.",
            ],
            Provider::Confluence => &[
                "If Jira is set up here, open that same app in the Atlassian developer console. Otherwise create an OAuth 2.0 integration and, under Authorization, paste the callback URL below.",
                "Under Permissions, add the Confluence API and, on its Granular scopes tab, grant read:page:confluence, read:space:confluence and read:user:confluence. Keep the User identity API (read:me).",
                "Back here, tick Use the Jira app — or paste the Client ID and Secret of a separate app — and save.",
                "Press Connect my account to test it: Atlassian asks each person to consent to Confluence separately from Jira.",
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
    pub client_secret_key_id: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct AppConnection {
    pub external_account: String,
    pub access_token_encrypted: Vec<u8>,
    pub access_token_nonce: Vec<u8>,
    pub access_token_key_id: Option<String>,
    pub refresh_token_encrypted: Option<Vec<u8>>,
    pub refresh_token_nonce: Option<Vec<u8>>,
    pub refresh_token_key_id: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub resources: serde_json::Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AppStatus {
    pub provider: Provider,
    pub name: String,
    pub summary: String,
    pub configured: bool,
    pub enabled: bool,
    pub client_id: Option<String>,
    pub redirect_url: String,
    pub scopes: Vec<String>,
    pub console_url: String,
    pub setup_steps: Vec<String>,
    pub connected_account: Option<String>,
    pub prompts_off: bool,
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
    #[serde(default)]
    pub reuse_from: Option<Provider>,
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
pub struct AtlassianSite {
    pub id: String,
    pub url: String,
}
