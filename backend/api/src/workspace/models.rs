use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared_common::errors::{AppError, AppResult};
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
    pub status_emoji: Option<String>,
    pub status_text: Option<String>,
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
pub struct BrowsableChannel {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: Option<String>,
    pub channel_type: ChannelType,
    pub topic: Option<String>,
    pub description: Option<String>,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
    pub member_count: i64,
    pub is_member: bool,
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
    pub expires_in_hours: Option<i64>,
    pub max_uses: Option<i32>,
}

/// Every invite is bounded. A link invite has to say how many people it is for;
/// there is no way to tell a legitimate outstanding link from a leaked one, so
/// "unlimited and forever" is not offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InviteLifetime {
    pub max_uses: i32,
    pub expires_in_hours: i64,
}

impl InviteLifetime {
    pub const MAX_HOURS: i64 = 168;
    pub const MAX_LINK_USES: i32 = 100;

    pub fn resolve(req: &CreateInviteRequest) -> AppResult<Self> {
        let expires_in_hours = req.expires_in_hours.unwrap_or(Self::MAX_HOURS);
        if !(1..=Self::MAX_HOURS).contains(&expires_in_hours) {
            return Err(AppError::Validation(format!(
                "An invite lasts between 1 and {} hours",
                Self::MAX_HOURS
            )));
        }

        let max_uses = match req.email {
            Some(_) => req.max_uses.unwrap_or(1),
            None => req.max_uses.ok_or_else(|| {
                AppError::Validation("A link invite must say how many people it is for".into())
            })?,
        };
        if !(1..=Self::MAX_LINK_USES).contains(&max_uses) {
            return Err(AppError::Validation(format!(
                "An invite is good for between 1 and {} uses",
                Self::MAX_LINK_USES
            )));
        }

        Ok(Self {
            max_uses,
            expires_in_hours,
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateMemberRoleRequest {
    pub role: WorkspaceRole,
}

#[derive(Debug, Deserialize)]
pub struct UpdateChannelMemberRoleRequest {
    pub role: ChannelRole,
}

#[derive(Debug, Deserialize)]
pub struct CreateChannelRequest {
    pub name: String,
    pub channel_type: Option<ChannelType>,
    pub description: Option<String>,
    pub is_default: Option<bool>,
    pub post_policy: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateChannelRequest {
    pub name: Option<String>,
    pub topic: Option<String>,
    pub description: Option<String>,
    /// `everyone` or `moderators`. Absent means unchanged, which is how a
    /// client that predates announcement channels keeps working.
    pub post_policy: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddChannelMemberRequest {
    pub user_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct SetChannelNotificationsRequest {
    pub muted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ChannelBookmark {
    pub id: Uuid,
    pub channel_id: Uuid,
    pub created_by: Option<Uuid>,
    pub label: String,
    pub url: String,
    pub emoji: Option<String>,
    pub position: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateChannelBookmarkRequest {
    pub label: String,
    pub url: String,
    pub emoji: Option<String>,
}
