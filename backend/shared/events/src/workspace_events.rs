use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceCreated {
    pub workspace_id: Uuid,
    pub name: String,
    pub owner_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberInvited {
    pub workspace_id: Uuid,
    pub email: String,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberJoined {
    pub workspace_id: Uuid,
    pub user_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelCreated {
    pub channel_id: Uuid,
    pub workspace_id: Uuid,
    pub name: Option<String>,
    pub channel_type: String,
}
