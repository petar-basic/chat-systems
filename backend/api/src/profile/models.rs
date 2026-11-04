use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct WorkspaceProfile {
    pub workspace_id: Uuid,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub bio: Option<String>,
    pub status_emoji: Option<String>,
    pub status_text: Option<String>,
    pub status_expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, garde::Validate, ToSchema)]
#[garde(allow_unvalidated)]
pub struct UpdateWorkspaceProfileRequest {
    #[garde(inner(custom(shared_common::validation::rules::display_name_or_blank)))]
    pub display_name: Option<String>,
    #[garde(inner(custom(shared_common::validation::rules::avatar_url_or_empty)))]
    pub avatar_url: Option<String>,
    #[garde(inner(custom(shared_common::validation::rules::bio)))]
    pub bio: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct Presence {
    pub away: bool,
}

#[derive(Debug, Deserialize, garde::Validate, ToSchema)]
#[garde(allow_unvalidated)]
pub struct SetStatusRequest {
    #[garde(inner(custom(shared_common::validation::rules::status_emoji_or_blank)))]
    pub emoji: Option<String>,
    #[garde(inner(custom(shared_common::validation::rules::status_text_or_blank)))]
    pub text: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, garde::Validate, ToSchema)]
pub struct Theme {
    #[garde(custom(shared_common::validation::rules::hex_colour))]
    pub sidebar: String,
    #[garde(custom(shared_common::validation::rules::hex_colour))]
    pub selected: String,
    #[garde(custom(shared_common::validation::rules::hex_colour))]
    pub presence: String,
    #[garde(custom(shared_common::validation::rules::hex_colour))]
    pub badge: String,
}

impl Theme {
    pub fn normalized(self) -> Self {
        Self {
            sidebar: self.sidebar.to_ascii_lowercase(),
            selected: self.selected.to_ascii_lowercase(),
            presence: self.presence.to_ascii_lowercase(),
            badge: self.badge.to_ascii_lowercase(),
        }
    }
}

#[derive(Debug, Deserialize, garde::Validate, ToSchema)]
pub struct SetThemeRequest {
    #[garde(dive)]
    pub theme: Option<Theme>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct WorkspaceTheme {
    pub workspace_id: Uuid,
    pub theme: Option<Theme>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ColourMode {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Density {
    #[default]
    Clean,
    Compact,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Previews {
    #[default]
    Shown,
    Collapsed,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(default)]
pub struct Preferences {
    pub colour_mode: Option<ColourMode>,
    pub density: Density,
    pub previews: Previews,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdatePreferences {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour_mode: Option<ColourMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub density: Option<Density>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previews: Option<Previews>,
}
