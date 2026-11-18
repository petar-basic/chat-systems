use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::blocks::models::HomeView;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AppHome {
    pub id: Uuid,
    pub app_id: Uuid,
    pub hash: String,
    #[serde(flatten)]
    pub view: HomeView,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct HomeTab {
    pub app_id: Uuid,
    /// What the app last published for whoever asks; nothing until it has.
    pub home: Option<AppHome>,
}

pub struct StoredHome {
    pub id: Uuid,
    pub app_id: Uuid,
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub view: serde_json::Value,
    pub hash: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PublishHomeRequest {
    /// Whose Home this is; nobody else sees it.
    pub user_id: Uuid,
    #[schema(value_type = HomeView)]
    pub view: serde_json::Value,
    /// The `hash` the app last saw; the publish is refused when this person's
    /// Home has changed since.
    #[serde(default)]
    pub hash: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct HomeOpenedRequest {
    /// The conversation with the app whose Home tab was opened.
    pub channel_id: Uuid,
}
