use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub password_hash: Option<String>,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub bio: Option<String>,
    pub title: Option<String>,
    pub pronouns: Option<String>,
    pub timezone: Option<String>,
    pub status: UserStatus,
    pub is_instance_admin: bool,
    pub workspace_creator: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl User {
    pub fn instance_role(&self) -> InstanceRole {
        InstanceRole::of(self.is_instance_admin, self.workspace_creator)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum InstanceRole {
    Member,
    WorkspaceCreator,
    Admin,
}

impl InstanceRole {
    pub fn of(is_instance_admin: bool, workspace_creator: bool) -> Self {
        match (is_instance_admin, workspace_creator) {
            (true, _) => Self::Admin,
            (false, true) => Self::WorkspaceCreator,
            (false, false) => Self::Member,
        }
    }

    pub fn creates_workspaces(self) -> bool {
        self != Self::Member
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq, ToSchema)]
#[sqlx(type_name = "user_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum UserStatus {
    Pending,
    Active,
    Suspended,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserPublic {
    pub id: Uuid,
    pub email: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub bio: Option<String>,
    pub title: Option<String>,
    pub pronouns: Option<String>,
    pub timezone: Option<String>,
    pub status: UserStatus,
    pub instance_role: InstanceRole,
    pub created_at: DateTime<Utc>,
}

impl From<User> for UserPublic {
    fn from(u: User) -> Self {
        let instance_role = u.instance_role();
        Self {
            id: u.id,
            email: u.email,
            display_name: u.display_name,
            avatar_url: u.avatar_url,
            bio: u.bio,
            title: u.title,
            pronouns: u.pronouns,
            timezone: u.timezone,
            status: u.status,
            instance_role,
            created_at: u.created_at,
        }
    }
}

#[derive(Debug)]
pub struct AuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
    pub user: UserPublic,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthSession {
    pub user: UserPublic,
    pub expires_in: i64,
    pub access_token: String,
    pub refresh_token: String,
}

impl From<AuthTokens> for AuthSession {
    fn from(t: AuthTokens) -> Self {
        Self {
            user: t.user,
            expires_in: t.expires_in,
            access_token: t.access_token,
            refresh_token: t.refresh_token,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
    /// A TOTP code or a recovery code; absent on the first attempt, which is how
    /// the client discovers a second factor is required.
    pub totp_code: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RegisterCompleteRequest {
    pub token: String,
    pub display_name: String,
    pub password: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SsoExchangeRequest {
    pub code: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SsoExchanged {
    pub landing: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ForgotPasswordRequest {
    pub email: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ResetPasswordRequest {
    pub token: String,
    pub password: String,
}

#[derive(Debug, Default, Deserialize, garde::Validate, ToSchema)]
#[garde(allow_unvalidated)]
pub struct UpdateProfileRequest {
    #[garde(inner(custom(shared_common::validation::rules::display_name)))]
    pub display_name: Option<String>,
    #[garde(inner(custom(shared_common::validation::rules::avatar_url_or_empty)))]
    pub avatar_url: Option<String>,
    #[garde(inner(custom(shared_common::validation::rules::bio)))]
    pub bio: Option<String>,
    #[garde(inner(custom(shared_common::validation::rules::title)))]
    pub title: Option<String>,
    #[garde(inner(custom(shared_common::validation::rules::pronouns)))]
    pub pronouns: Option<String>,
    #[garde(inner(custom(shared_common::validation::rules::timezone)))]
    pub timezone: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct InvitePreview {
    pub email: String,
    pub workspace_name: String,
    pub workspace_id: Uuid,
    pub already_registered: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct InviteAccepted {
    pub workspace_id: Uuid,
    pub workspace_name: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct InstanceInfo {
    pub name: String,
    pub icon_url: Option<String>,
    pub sso_enabled: bool,
    pub email_code_sign_in: bool,
    pub max_upload_bytes: u64,
    pub setup_required: bool,
    pub workspace_creation_restricted: bool,
}

pub struct RegistrationInvite {
    pub user_id: Uuid,
    pub email: String,
    pub invite_id: Uuid,
}
