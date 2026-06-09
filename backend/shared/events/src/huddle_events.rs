use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HuddleStarted {
    pub huddle_id: Uuid,
    pub workspace_id: Uuid,
    pub channel_id: Option<Uuid>,
    pub dm_partner_id: Option<Uuid>,
    pub initiator_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HuddleEnded {
    pub huddle_id: Uuid,
    pub workspace_id: Uuid,
    pub channel_id: Option<Uuid>,
    pub dm_partner_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HuddleRing {
    pub huddle_id: Uuid,
    pub workspace_id: Uuid,
    pub from_user_id: Uuid,
    pub to_user_id: Uuid,
    pub channel_id: Option<Uuid>,
}
