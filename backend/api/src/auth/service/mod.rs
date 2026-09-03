use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use lettre::{AsyncSmtpTransport, Tokio1Executor};
use tracing::info;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};
use shared_common::validation;

use super::models::{AuthTokens, User, UserPublic, UserStatus};
use super::repo::UserRepo;
use crate::config::AppConfig;

mod email;
#[cfg(test)]
mod tests;
mod tokens;

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
static ABSENT_USER_HASH: std::sync::OnceLock<String> = std::sync::OnceLock::new();

fn absent_user_hash() -> &'static str {
    ABSENT_USER_HASH.get_or_init(|| {
        AuthService::hash_password("no-such-account-placeholder")
            .unwrap_or_else(|_| String::from("$argon2id$v=19$m=19456,t=2,p=1$c2FsdA$aGFzaA"))
    })
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

    pub fn hash_password(password: &str) -> AppResult<String> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| AppError::Internal(format!("Password hashing failed: {e}")))?;
        Ok(hash.to_string())
    }

    pub fn verify_password(password: &str, hash: &str) -> AppResult<bool> {
        let parsed_hash = PasswordHash::new(hash)
            .map_err(|e| AppError::Internal(format!("Invalid password hash: {e}")))?;
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok())
    }

    pub async fn login(&self, email: &str, password: &str) -> AppResult<AuthTokens> {
        validation::validate_email(email)?;

        let user = self.repo.find_by_email(email).await?;

        // Always pay for a verification, whatever went wrong. Returning early on
        // an unknown address made "does this person work here" answerable with a
        // stopwatch.
        let hash = user
            .as_ref()
            .and_then(|u| u.password_hash.clone())
            .unwrap_or_else(|| absent_user_hash().to_string());
        let password_matches = Self::verify_password(password, &hash).unwrap_or(false);

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
        let hash = user
            .as_ref()
            .and_then(|u| u.password_hash.clone())
            .unwrap_or_else(|| absent_user_hash().to_string());
        let password_matches = Self::verify_password(password, &hash).unwrap_or(false);

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

        let password_hash = Self::hash_password(password)?;
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

    pub async fn forgot_password(&self, email: &str) -> AppResult<()> {
        validation::validate_email(email)?;

        let user = self.repo.find_by_email(email).await?;

        if let Some(user) = user {
            match self.generate_reset_token(user.id).await {
                Ok(token) => {
                    let reset_url =
                        format!("{}/reset-password?token={}", self.config.public_url, token);
                    if let Err(e) = self.send_reset_email(&user.email, &reset_url).await {
                        tracing::warn!("failed to send password reset email: {}", e);
                    }
                }
                Err(e) => tracing::warn!("failed to generate password reset token: {}", e),
            }
        }

        Ok(())
    }

    pub async fn reset_password(&self, token: &str, new_password: &str) -> AppResult<Uuid> {
        let claims = self.verify_token(token)?;

        if claims.token_type != "reset" {
            return Err(AppError::Unauthorized("Invalid or expired token".into()));
        }

        let jti = claims
            .jti
            .ok_or_else(|| AppError::Unauthorized("Invalid or expired token".into()))?;
        let consumed = self.repo.consume_reset_jti(jti, claims.sub).await?;
        if !consumed {
            return Err(AppError::Unauthorized(
                "reset link already used or expired".into(),
            ));
        }

        validation::validate_password(new_password)?;

        let password_hash = Self::hash_password(new_password)?;
        self.repo
            .update_password(claims.sub, &password_hash)
            .await?;

        Ok(claims.sub)
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

        if !Self::verify_password(current_password, password_hash)? {
            return Err(AppError::Unauthorized(
                "Current password is incorrect".into(),
            ));
        }

        validation::validate_password(new_password)?;

        let new_hash = Self::hash_password(new_password)?;
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

    pub async fn bootstrap_admin(&self) -> AppResult<()> {
        let email = match &self.config.admin_email {
            Some(e) if !e.is_empty() => e.clone(),
            _ => return Ok(()),
        };
        let password = match &self.config.admin_password {
            Some(p) if !p.is_empty() => p.clone(),
            _ => return Ok(()),
        };

        let existing = self.repo.find_by_email(&email).await?;

        if existing.is_some() {
            info!("Instance admin already exists: {}", email);
            return Ok(());
        }

        let hash = Self::hash_password(&password)?;
        self.repo
            .create(&email, Some(&hash), Some("Admin"), true)
            .await?;

        let user = self
            .repo
            .find_by_email(&email)
            .await?
            .ok_or_else(|| AppError::Internal("Admin user not found after creation".into()))?;

        self.repo.activate(user.id, &hash, "Admin").await?;

        info!("Instance admin bootstrapped: {}", email);
        Ok(())
    }
}
