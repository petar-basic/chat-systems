use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use lettre::{AsyncSmtpTransport, Tokio1Executor};
use tracing::info;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};
use shared_common::validation;

use super::models::{AuthTokens, User, UserPublic, UserStatus};
use super::repo::UserRepo;
use crate::config::AppConfig;
use crate::profile::models::Language;
use crate::scim::repo::hash_token;

mod email;
pub use email::{InviteEmail, InviteRequestEmail};
#[cfg(test)]
mod tests;
mod tokens;

#[derive(Debug)]
pub enum PasswordRecovery {
    Handled,
    AwaitingInvite(Box<User>, Language),
}

pub struct AuthService {
    repo: UserRepo,
    config: AppConfig,
    mailer: Option<AsyncSmtpTransport<Tokio1Executor>>,
}

/// Verified against when no account exists, so "unknown address" costs the same
/// as "wrong password". Without it the fast path is a timing oracle that hands
/// over the instance's address book.
///
/// Computed once per process: Argon2 is deliberately expensive, and hashing it
/// per `AuthService` starved the runtime everywhere the service is constructed
/// more than once.
static ABSENT_USER_HASH: tokio::sync::OnceCell<String> = tokio::sync::OnceCell::const_new();

async fn absent_user_hash() -> &'static str {
    ABSENT_USER_HASH
        .get_or_init(|| async {
            AuthService::hash_password("no-such-account-placeholder")
                .await
                .unwrap_or_else(|_| String::from("$argon2id$v=19$m=19456,t=2,p=1$c2FsdA$aGFzaA"))
        })
        .await
}

impl AuthService {
    pub fn new(repo: UserRepo, config: AppConfig) -> Self {
        let mailer = email::build_mailer(&config);
        Self {
            repo,
            config,
            mailer,
        }
    }

    pub fn repo(&self) -> &UserRepo {
        &self.repo
    }

    pub async fn hash_password(password: &str) -> AppResult<String> {
        let password = password.to_owned();
        tokio::task::spawn_blocking(move || {
            Argon2::default()
                .hash_password(password.as_bytes())
                .map(|hash| hash.to_string())
                .map_err(|e| AppError::Internal(format!("Password hashing failed: {e}")))
        })
        .await
        .map_err(|e| AppError::Internal(format!("Password hashing failed: {e}")))?
    }

    pub async fn verify_password(password: &str, hash: &str) -> AppResult<bool> {
        let password = password.to_owned();
        let hash = hash.to_owned();
        tokio::task::spawn_blocking(move || {
            let parsed_hash = PasswordHash::new(&hash)
                .map_err(|e| AppError::Internal(format!("Invalid password hash: {e}")))?;
            Ok(Argon2::default()
                .verify_password(password.as_bytes(), &parsed_hash)
                .is_ok())
        })
        .await
        .map_err(|e| AppError::Internal(format!("Password check failed: {e}")))?
    }

    pub async fn login(&self, email: &str, password: &str) -> AppResult<AuthTokens> {
        validation::validate_email(email)?;

        let user = self.repo.find_by_email(email).await?;

        // Always pay for a verification, whatever went wrong. Returning early on
        // an unknown address made "does this person work here" answerable with a
        // stopwatch.
        let hash = match user.as_ref().and_then(|u| u.password_hash.clone()) {
            Some(hash) => hash,
            None => absent_user_hash().await.to_string(),
        };
        let password_matches = Self::verify_password(password, &hash)
            .await
            .unwrap_or(false);

        let reason = match &user {
            None => Some("no such account"),
            Some(u) if u.status != UserStatus::Active => Some("account is not active"),
            Some(u) if u.password_hash.is_none() => Some("account has no password set"),
            _ if !password_matches => Some("wrong password"),
            _ => None,
        };

        if let Some(reason) = reason {
            // The caller gets one answer for every failure; the operator still
            // gets the real one.
            info!(email = %email, reason, "login rejected");
            return Err(AppError::Unauthorized("Invalid email or password".into()));
        }

        let user =
            user.ok_or_else(|| AppError::Unauthorized("Invalid email or password".into()))?;
        self.generate_tokens(&user).await
    }

    /// The password is verified here and the tokens are *not* minted: whether a
    /// second factor stands between the two is the caller's business, because
    /// only the route knows the TOTP state.
    pub async fn verify_password_only(&self, email: &str, password: &str) -> AppResult<User> {
        validation::validate_email(email)?;

        let user = self.repo.find_by_email(email).await?;
        let hash = match user.as_ref().and_then(|u| u.password_hash.clone()) {
            Some(hash) => hash,
            None => absent_user_hash().await.to_string(),
        };
        let password_matches = Self::verify_password(password, &hash)
            .await
            .unwrap_or(false);

        let reason = match &user {
            None => Some("no such account"),
            Some(u) if u.status != UserStatus::Active => Some("account is not active"),
            Some(u) if u.password_hash.is_none() => Some("account has no password set"),
            _ if !password_matches => Some("wrong password"),
            _ => None,
        };

        if let Some(reason) = reason {
            info!(email = %email, reason, "login rejected");
            return Err(AppError::Unauthorized("Invalid email or password".into()));
        }

        user.ok_or_else(|| AppError::Unauthorized("Invalid email or password".into()))
    }

    pub async fn tokens_for(&self, user: &User) -> AppResult<AuthTokens> {
        self.generate_tokens(user).await
    }

    pub async fn complete_registration(
        &self,
        user_id: Uuid,
        password: &str,
        display_name: &str,
    ) -> AppResult<AuthTokens> {
        validation::validate_display_name(display_name)?;

        let user = self
            .repo
            .find_by_id(user_id)
            .await?
            .ok_or_else(|| AppError::NotFound("User not found".into()))?;

        // State of the account first: telling somebody their password is weak
        // when the real answer is "this invite was already used" sends them
        // round in circles.
        if user.status != UserStatus::Pending {
            return Err(AppError::BadRequest("Account is already activated".into()));
        }

        validation::validate_password(password)?;

        let password_hash = Self::hash_password(password).await?;
        let user = self
            .repo
            .activate(user_id, &password_hash, display_name)
            .await?;

        self.generate_tokens(&user).await
    }

    pub async fn logout(&self, refresh_token: &str) -> AppResult<()> {
        if let Ok(claims) = self.verify_token(refresh_token) {
            if let Some(jti) = claims.jti {
                let _ = self.repo.delete_refresh_token(&jti.to_string()).await;
            }
        }
        Ok(())
    }

    pub async fn forgot_password(
        &self,
        email: &str,
        accept_language: Option<&str>,
    ) -> AppResult<PasswordRecovery> {
        validation::validate_email(email)?;

        let Some(user) = self.repo.find_by_email(email).await? else {
            return Ok(PasswordRecovery::Handled);
        };
        let language = self.language_for(user.id, accept_language).await?;

        if user.status == UserStatus::Pending {
            return Ok(PasswordRecovery::AwaitingInvite(Box::new(user), language));
        }

        if user.password_hash.is_none() {
            if let Err(e) = self.send_sso_reminder(&user.email, language).await {
                tracing::warn!("failed to send SSO reminder email: {}", e);
            }
            return Ok(PasswordRecovery::Handled);
        }

        match self.generate_reset_token(user.id).await {
            Ok(token) => {
                let reset_url =
                    format!("{}/reset-password?token={}", self.config.public_url, token);
                if let Err(e) = self
                    .send_reset_email(&user.email, &reset_url, language)
                    .await
                {
                    tracing::warn!("failed to send password reset email: {}", e);
                }
            }
            Err(e) => tracing::warn!("failed to generate password reset token: {}", e),
        }

        Ok(PasswordRecovery::Handled)
    }

    pub async fn reset_password(&self, token: &str, new_password: &str) -> AppResult<Uuid> {
        validation::validate_password(new_password)?;

        let user_id = self
            .repo
            .consume_reset_token(&hash_token(token))
            .await?
            .ok_or_else(|| AppError::Unauthorized("reset link already used or expired".into()))?;

        let password_hash = Self::hash_password(new_password).await?;
        self.repo.update_password(user_id, &password_hash).await?;

        Ok(user_id)
    }

    pub async fn change_password(
        &self,
        user_id: Uuid,
        current_password: &str,
        new_password: &str,
    ) -> AppResult<()> {
        let user = self
            .repo
            .find_by_id(user_id)
            .await?
            .ok_or_else(|| AppError::NotFound("User not found".into()))?;

        let password_hash = user
            .password_hash
            .as_ref()
            .ok_or_else(|| AppError::Unauthorized("Current password is incorrect".into()))?;

        if !Self::verify_password(current_password, password_hash).await? {
            return Err(AppError::Unauthorized(
                "Current password is incorrect".into(),
            ));
        }

        validation::validate_password(new_password)?;

        let new_hash = Self::hash_password(new_password).await?;
        self.repo.update_password(user_id, &new_hash).await?;

        Ok(())
    }

    pub async fn provision_user(&self, email: &str) -> AppResult<UserPublic> {
        let user = self.repo.find_by_email(email).await?;

        if let Some(user) = user {
            return Ok(user.into());
        }

        let user = self.repo.create(email, None, None, false).await?;

        Ok(user.into())
    }

    pub async fn create_instance_admin(
        &self,
        email: &str,
        password: &str,
        display_name: &str,
    ) -> AppResult<User> {
        let hash = Self::hash_password(password).await?;
        let user = self
            .repo
            .create(email, Some(&hash), Some(display_name), true)
            .await?;
        Ok(self.repo.activate(user.id, &hash, display_name).await?)
    }

    pub async fn bootstrap_admin(&self) -> AppResult<()> {
        let email = match &self.config.admin_email {
            Some(e) if !e.is_empty() => e.clone(),
            _ => return Ok(()),
        };
        let password = match &self.config.admin_password {
            Some(p) if !p.is_empty() => p.clone(),
            _ => return Ok(()),
        };

        if self.repo.find_by_email(&email).await?.is_some() {
            info!("Instance admin already exists: {}", email);
            return Ok(());
        }

        self.create_instance_admin(&email, &password, "Admin")
            .await?;

        info!("Instance admin bootstrapped: {}", email);
        Ok(())
    }
}
