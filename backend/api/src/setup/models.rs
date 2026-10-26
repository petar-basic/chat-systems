use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::models::AuthSession;

#[derive(Debug, Deserialize, ToSchema)]
pub struct SetupRequest {
    pub token: String,
    pub display_name: String,
    pub email: String,
    pub password: String,
    pub workspace_name: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SetupComplete {
    pub session: AuthSession,
    pub workspace_id: Uuid,
}
