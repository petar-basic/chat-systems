use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Draft {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub channel_id: Uuid,
    pub thread_parent_id: Option<Uuid>,
    pub content: String,
    pub files_left_behind: bool,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, garde::Validate, ToSchema)]
#[garde(allow_unvalidated)]
pub struct SaveDraftRequest {
    pub channel_id: Uuid,
    pub thread_parent_id: Option<Uuid>,
    #[garde(custom(shared_common::validation::rules::message_body))]
    pub content: String,
    #[serde(default)]
    pub files_left_behind: bool,
    pub updated_at: DateTime<Utc>,
}
