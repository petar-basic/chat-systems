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
