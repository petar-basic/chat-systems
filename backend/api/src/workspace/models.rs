use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Workspace {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub icon_url: Option<String>,
    pub owner_id: Uuid,
    pub settings: serde_json::Value,
    pub is_active: bool,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteWorkspaceRequest {
    pub hard: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq, PartialOrd)]
#[sqlx(type_name = "workspace_role", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum WorkspaceRole {
    Guest,
    Member,
    Admin,
    Owner,
}

impl WorkspaceRole {
    pub fn level(&self) -> u8 {
        match self {
            WorkspaceRole::Guest => 10,
            WorkspaceRole::Member => 20,
            WorkspaceRole::Admin => 40,
            WorkspaceRole::Owner => 50,
        }
    }

    pub fn has_at_least(&self, minimum: &WorkspaceRole) -> bool {
        self.level() >= minimum.level()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WorkspaceMember {
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub role: WorkspaceRole,
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MemberWithUser {
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub role: WorkspaceRole,
    pub joined_at: DateTime<Utc>,
    pub email: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WorkspaceInvite {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub created_by: Uuid,
    pub email: Option<String>,
    pub role: WorkspaceRole,
    pub token: String,
    pub max_uses: Option<i32>,
    pub use_count: i32,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "channel_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ChannelType {
    Public,
    Private,
    Dm,
    GroupDm,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "channel_role", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ChannelRole {
    Member,
    Admin,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Channel {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: Option<String>,
    pub channel_type: ChannelType,
    pub topic: Option<String>,
    pub description: Option<String>,
    pub created_by: Option<Uuid>,
    pub is_default: bool,
    pub is_archived: bool,
    pub settings: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ChannelMember {
    pub channel_id: Uuid,
    pub user_id: Uuid,
    pub role: ChannelRole,
    pub last_read_at: Option<DateTime<Utc>>,
    pub last_read_msg: Option<Uuid>,
    pub notifications: String,
    pub is_muted: bool,
    pub is_starred: bool,
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateWorkspaceRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateWorkspaceRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub icon_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateInviteRequest {
    pub email: Option<String>,
    pub role: Option<WorkspaceRole>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMemberRoleRequest {
    pub role: WorkspaceRole,
}

#[derive(Debug, Deserialize)]
pub struct CreateChannelRequest {
    pub name: String,
    pub channel_type: Option<ChannelType>,
    pub description: Option<String>,
    pub is_default: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateChannelRequest {
    pub name: Option<String>,
    pub topic: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddChannelMemberRequest {
    pub user_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct SetChannelNotificationsRequest {
    pub muted: bool,
}
