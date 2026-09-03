use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
pub struct StartHuddleRequest {
    pub channel_id: Option<Uuid>,
    pub dm_partner_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct InviteRequest {
    pub user_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow, ToSchema)]
pub struct HuddleSession {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub channel_id: Option<Uuid>,
    pub dm_partner_id: Option<Uuid>,
    pub initiated_by: Uuid,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct IceServer {
    pub urls: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct IceServersResponse {
    pub ice_servers: Vec<IceServer>,
    pub ttl: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ActiveHuddle {
    pub huddle_id: Uuid,
    pub channel_id: Option<Uuid>,
    pub initiator_id: Uuid,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct HuddleStarted {
    pub huddle_id: Uuid,
}
