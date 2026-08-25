use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub password_hash: Option<String>,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub bio: Option<String>,
    pub timezone: String,
    pub status: UserStatus,
    pub status_emoji: Option<String>,
    pub status_text: Option<String>,
    pub status_expires_at: Option<DateTime<Utc>>,
    pub is_instance_admin: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "user_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum UserStatus {
    Pending,
    Active,
    Suspended,
}

#[derive(Debug, Serialize)]
pub struct UserPublic {
    pub id: Uuid,
    pub email: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub bio: Option<String>,
    pub timezone: String,
    pub status: UserStatus,
    pub status_emoji: Option<String>,
    pub status_text: Option<String>,
    pub status_expires_at: Option<DateTime<Utc>>,
    pub is_instance_admin: bool,
    pub created_at: DateTime<Utc>,
}

/// An expired status is nobody's business. The row is cleaned up lazily by the
/// worker, so the read side has to be the one that stops showing it.
fn live_status(u: &User) -> bool {
    u.status_expires_at.is_none_or(|at| at > Utc::now())
}

impl From<User> for UserPublic {
    fn from(u: User) -> Self {
        let live = live_status(&u);
        Self {
            id: u.id,
            email: u.email,
            display_name: u.display_name,
            avatar_url: u.avatar_url,
            bio: u.bio,
            timezone: u.timezone,
            status: u.status,
            status_emoji: live.then_some(u.status_emoji).flatten(),
            status_text: live.then_some(u.status_text).flatten(),
            status_expires_at: live.then_some(u.status_expires_at).flatten(),
            is_instance_admin: u.is_instance_admin,
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

#[derive(Debug, Serialize)]
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

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
    /// A TOTP code or a recovery code; absent on the first attempt, which is how
    /// the client discovers a second factor is required.
    pub totp_code: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RegisterCompleteRequest {
    pub token: String,
    pub display_name: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct ForgotPasswordRequest {
    pub email: String,
}

#[derive(Debug, Deserialize)]
pub struct ResetPasswordRequest {
    pub token: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct SetStatusRequest {
    pub emoji: Option<String>,
    pub text: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProfileRequest {
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub bio: Option<String>,
    pub timezone: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}
