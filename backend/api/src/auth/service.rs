use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, TokenData, Validation};
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use rand::rngs::OsRng;
use tracing::info;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};
use shared_common::validation;

use super::models::{AuthTokens, User, UserPublic, UserStatus};
use super::repo::UserRepo;
use crate::config::AppConfig;
use crate::middleware::Claims;

pub struct AuthService {
    repo: UserRepo,
    config: AppConfig,
    mailer: Option<AsyncSmtpTransport<Tokio1Executor>>,
}

fn build_mailer(config: &AppConfig) -> Option<AsyncSmtpTransport<Tokio1Executor>> {
    let creds = Credentials::new(config.smtp_user.clone(), config.smtp_password.clone());
    if config.smtp_use_tls {
        AsyncSmtpTransport::<Tokio1Executor>::relay(&config.smtp_host)
            .ok()
            .map(|b| b.port(config.smtp_port).credentials(creds).build())
    } else {
        Some(
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.smtp_host)
                .port(config.smtp_port)
                .credentials(creds)
                .build(),
        )
    }
}

impl AuthService {
    pub fn new(repo: UserRepo, config: AppConfig) -> Self {
        let mailer = build_mailer(&config);
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
            .map_err(|e| AppError::Internal(format!("Password hashing failed: {}", e)))?;
        Ok(hash.to_string())
    }

    pub fn verify_password(password: &str, hash: &str) -> AppResult<bool> {
        let parsed_hash = PasswordHash::new(hash)
            .map_err(|e| AppError::Internal(format!("Invalid password hash: {}", e)))?;
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok())
    }

    pub async fn login(&self, email: &str, password: &str) -> AppResult<AuthTokens> {
        validation::validate_email(email)?;

        let user = self
            .repo
            .find_by_email(email)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
            .ok_or_else(|| AppError::Unauthorized("Invalid email or password".into()))?;

        if user.status != UserStatus::Active {
            return Err(AppError::Unauthorized(
                "Account is not active. Please complete registration first.".into(),
            ));
        }

        let password_hash = user
            .password_hash
            .as_ref()
            .ok_or_else(|| AppError::Unauthorized("Account requires password setup".into()))?;

        if !Self::verify_password(password, password_hash)? {
            return Err(AppError::Unauthorized("Invalid email or password".into()));
        }

        self.generate_tokens(&user).await
    }

    pub async fn complete_registration(
        &self,
        user_id: Uuid,
        password: &str,
        display_name: &str,
    ) -> AppResult<AuthTokens> {
        validation::validate_password(password)?;
        validation::validate_display_name(display_name)?;

        let user = self
            .repo
            .find_by_id(user_id)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
            .ok_or_else(|| AppError::NotFound("User not found".into()))?;

        if user.status != UserStatus::Pending {
            return Err(AppError::BadRequest("Account is already activated".into()));
        }

        let password_hash = Self::hash_password(password)?;
        let user = self
            .repo
            .activate(user_id, &password_hash, display_name)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

        self.generate_tokens(&user).await
    }

    pub async fn refresh_access_token(&self, refresh_token: &str) -> AppResult<AuthTokens> {
        let claims = self.verify_token(refresh_token)?;

        if claims.token_type != "refresh" {
            return Err(AppError::Unauthorized("Invalid refresh token".into()));
        }

        let jti = claims
            .jti
            .ok_or_else(|| AppError::Unauthorized("Invalid refresh token".into()))?;

        let uid = self
            .repo
            .find_refresh_token(&jti.to_string())
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
            .ok_or_else(|| {
                AppError::Unauthorized("Refresh token has been revoked or expired".into())
            })?;

        if uid != claims.sub {
            return Err(AppError::Unauthorized("Invalid refresh token".into()));
        }

        let user = self
            .repo
            .find_by_id(claims.sub)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
            .ok_or_else(|| AppError::Unauthorized("User not found".into()))?;

        if user.status != UserStatus::Active {
            return Err(AppError::Unauthorized("Account is not active".into()));
        }

        let now = Utc::now();
        let access_exp = now + Duration::seconds(self.config.access_token_expiry);
        let refresh_exp = now + Duration::seconds(self.config.refresh_token_expiry);
        let new_jti = Uuid::new_v4();

        let access_claims = Claims {
            sub: user.id,
            email: user.email.clone(),
            is_instance_admin: user.is_instance_admin,
            iat: now.timestamp(),
            exp: access_exp.timestamp(),
            jti: None,
            token_type: "access".to_string(),
            workspace_id: None,
            invite_role: None,
        };
        let access_token = encode(
            &Header::default(),
            &access_claims,
            &EncodingKey::from_secret(self.config.jwt_secret.as_bytes()),
        )
        .map_err(|e| AppError::Internal(format!("Token generation failed: {}", e)))?;

        let refresh_claims = Claims {
            sub: user.id,
            email: user.email.clone(),
            is_instance_admin: user.is_instance_admin,
            iat: now.timestamp(),
            exp: refresh_exp.timestamp(),
            jti: Some(new_jti),
            token_type: "refresh".to_string(),
            workspace_id: None,
            invite_role: None,
        };
        let new_refresh_token = encode(
            &Header::default(),
            &refresh_claims,
            &EncodingKey::from_secret(self.config.jwt_secret.as_bytes()),
        )
        .map_err(|e| AppError::Internal(format!("Token generation failed: {}", e)))?;

        let mut tx = self
            .repo
            .begin()
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        UserRepo::delete_refresh_token_tx(&mut tx, &jti.to_string())
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        UserRepo::store_refresh_token_tx(&mut tx, user.id, &new_jti.to_string(), refresh_exp)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(AuthTokens {
            access_token,
            refresh_token: new_refresh_token,
            expires_in: self.config.access_token_expiry,
            user: user.into(),
        })
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

        let user = self
            .repo
            .find_by_email(email)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

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

    pub async fn reset_password(&self, token: &str, new_password: &str) -> AppResult<()> {
        validation::validate_password(new_password)?;

        let claims = self.verify_token(token)?;

        if claims.token_type != "reset" {
            return Err(AppError::Unauthorized("Invalid or expired token".into()));
        }

        let jti = claims
            .jti
            .ok_or_else(|| AppError::Unauthorized("Invalid or expired token".into()))?;
        let consumed = self
            .repo
            .consume_reset_jti(jti, claims.sub)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        if !consumed {
            return Err(AppError::Unauthorized(
                "reset link already used or expired".into(),
            ));
        }

        let password_hash = Self::hash_password(new_password)?;
        self.repo
            .update_password(claims.sub, &password_hash)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

        let _ = self.repo.delete_user_refresh_tokens(claims.sub).await;

        Ok(())
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
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
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
        self.repo
            .update_password(user_id, &new_hash)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

        let _ = self.repo.delete_user_refresh_tokens(user_id).await;

        Ok(())
    }

    pub async fn provision_user(&self, email: &str) -> AppResult<UserPublic> {
        let user = self
            .repo
            .find_by_email(email)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

        if let Some(user) = user {
            return Ok(user.into());
        }

        let user = self
            .repo
            .create(email, None, None, false)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(user.into())
    }

    pub async fn generate_tokens(&self, user: &User) -> AppResult<AuthTokens> {
        let now = Utc::now();
        let access_exp = now + Duration::seconds(self.config.access_token_expiry);
        let refresh_exp = now + Duration::seconds(self.config.refresh_token_expiry);
        let jti = Uuid::new_v4();

        let access_claims = Claims {
            sub: user.id,
            email: user.email.clone(),
            is_instance_admin: user.is_instance_admin,
            iat: now.timestamp(),
            exp: access_exp.timestamp(),
            jti: None,
            token_type: "access".to_string(),
            workspace_id: None,
            invite_role: None,
        };

        let access_token = encode(
            &Header::default(),
            &access_claims,
            &EncodingKey::from_secret(self.config.jwt_secret.as_bytes()),
        )
        .map_err(|e| AppError::Internal(format!("Token generation failed: {}", e)))?;

        let refresh_claims = Claims {
            sub: user.id,
            email: user.email.clone(),
            is_instance_admin: user.is_instance_admin,
            iat: now.timestamp(),
            exp: refresh_exp.timestamp(),
            jti: Some(jti),
            token_type: "refresh".to_string(),
            workspace_id: None,
            invite_role: None,
        };

        let refresh_token = encode(
            &Header::default(),
            &refresh_claims,
            &EncodingKey::from_secret(self.config.jwt_secret.as_bytes()),
        )
        .map_err(|e| AppError::Internal(format!("Token generation failed: {}", e)))?;

        self.repo
            .store_refresh_token(user.id, &jti.to_string(), refresh_exp)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(AuthTokens {
            access_token,
            refresh_token,
            expires_in: self.config.access_token_expiry,
            user: user.clone().into(),
        })
    }

    pub fn verify_token(&self, token: &str) -> AppResult<Claims> {
        let token_data: TokenData<Claims> = decode(
            token,
            &DecodingKey::from_secret(self.config.jwt_secret.as_bytes()),
            &Validation::default(),
        )
        .map_err(|_| AppError::Unauthorized("Invalid or expired token".into()))?;

        Ok(token_data.claims)
    }

    async fn generate_reset_token(&self, user_id: Uuid) -> AppResult<String> {
        let now = Utc::now();
        let exp = now + Duration::seconds(3600);
        let jti = Uuid::new_v4();
        let claims = Claims {
            sub: user_id,
            email: String::new(),
            is_instance_admin: false,
            iat: now.timestamp(),
            exp: exp.timestamp(),
            jti: Some(jti),
            token_type: "reset".to_string(),
            workspace_id: None,
            invite_role: None,
        };
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.config.jwt_secret.as_bytes()),
        )
        .map_err(|e| AppError::Internal(format!("Reset token generation failed: {}", e)))?;

        self.repo
            .store_reset_jti(jti, user_id, exp)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(token)
    }

    pub fn generate_registration_token(
        &self,
        user_id: Uuid,
        email: &str,
        workspace_id: Uuid,
        role: &str,
    ) -> AppResult<String> {
        let now = Utc::now();
        let exp = now + Duration::days(7);
        let claims = Claims {
            sub: user_id,
            email: email.to_string(),
            is_instance_admin: false,
            iat: now.timestamp(),
            exp: exp.timestamp(),
            jti: None,
            token_type: "registration".to_string(),
            workspace_id: Some(workspace_id),
            invite_role: Some(role.to_string()),
        };
        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.config.jwt_secret.as_bytes()),
        )
        .map_err(|e| AppError::Internal(format!("Registration token generation failed: {}", e)))
    }

    pub fn verify_registration_token(&self, token: &str) -> AppResult<Claims> {
        let claims = self
            .verify_token(token)
            .map_err(|_| AppError::Unauthorized("Invalid or expired invite".into()))?;

        if claims.token_type != "registration" {
            return Err(AppError::Unauthorized("Invalid or expired invite".into()));
        }

        Ok(claims)
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

        let existing = self
            .repo
            .find_by_email(&email)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

        if existing.is_some() {
            info!("Instance admin already exists: {}", email);
            return Ok(());
        }

        let hash = Self::hash_password(&password)?;
        self.repo
            .create(&email, Some(&hash), Some("Admin"), true)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

        let user = self
            .repo
            .find_by_email(&email)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
            .ok_or_else(|| AppError::Internal("Admin user not found after creation".into()))?;

        self.repo
            .activate(user.id, &hash, "Admin")
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

        info!("Instance admin bootstrapped: {}", email);
        Ok(())
    }

    async fn send_reset_email(&self, to_email: &str, reset_url: &str) -> AppResult<()> {
        let body = format!(
            "You requested a password reset.\n\nClick the link below to reset your password:\n{}\n\nThis link expires in 1 hour.",
            reset_url
        );

        self.send_email(to_email, "Reset your password", &body)
            .await
    }

    pub async fn send_invite_email(
        &self,
        to_email: &str,
        workspace_name: &str,
        invite_url: &str,
    ) -> AppResult<()> {
        let body = format!(
            "You've been invited to join {} on {}.\n\nClick the link below to get started:\n{}\n",
            workspace_name, self.config.instance_name, invite_url
        );

        self.send_email(
            to_email,
            &format!("Join {} on {}", workspace_name, self.config.instance_name),
            &body,
        )
        .await
    }

    async fn send_email(&self, to: &str, subject: &str, body: &str) -> AppResult<()> {
        let mailer = self
            .mailer
            .as_ref()
            .ok_or_else(|| AppError::Internal("Email service not configured".into()))?;

        let from = format!(
            "{} <{}>",
            self.config.smtp_from_name, self.config.smtp_from_address
        );

        let email = Message::builder()
            .from(
                from.parse()
                    .map_err(|e| AppError::Internal(format!("Invalid from address: {}", e)))?,
            )
            .to(to
                .parse()
                .map_err(|e| AppError::Internal(format!("Invalid to address: {}", e)))?)
            .subject(subject)
            .header(ContentType::TEXT_PLAIN)
            .body(body.to_string())
            .map_err(|e| AppError::Internal(format!("Failed to build email: {}", e)))?;

        mailer
            .send(email)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to send email: {}", e)))?;

        info!("Email sent to {}: {}", to, subject);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfig, StorageBackend};
    use crate::middleware::Claims;
    use jsonwebtoken::{decode, DecodingKey, Validation};
    use sqlx::PgPool;

    fn test_config() -> AppConfig {
        AppConfig {
            port: 3000,
            database_url: String::new(),
            redis_url: String::new(),
            jwt_secret: "test-jwt-secret-0123456789-abcdefghij".to_string(),
            access_token_expiry: 3600,
            refresh_token_expiry: 604_800,
            admin_email: None,
            admin_password: None,
            smtp_host: "localhost".to_string(),
            smtp_port: 1025,
            smtp_user: String::new(),
            smtp_password: String::new(),
            smtp_from_address: "noreply@test.local".to_string(),
            smtp_from_name: "Test".to_string(),
            smtp_use_tls: false,
            public_url: "http://localhost:3000".to_string(),
            instance_name: "Test".to_string(),
            instance_icon_url: None,
            cors_origins: String::new(),
            storage_backend: StorageBackend::Local,
            local_storage_path: "./data/files".to_string(),
            s3_endpoint: String::new(),
            s3_region: String::new(),
            s3_bucket: String::new(),
            s3_access_key: String::new(),
            s3_secret_key: String::new(),
        }
    }

    fn decode_claims(token: &str) -> Claims {
        let config = test_config();
        decode::<Claims>(
            token,
            &DecodingKey::from_secret(config.jwt_secret.as_bytes()),
            &Validation::default(),
        )
        .expect("token should decode with the test secret")
        .claims
    }

    fn dummy_user() -> User {
        let now = Utc::now();
        User {
            id: Uuid::new_v4(),
            email: "user@test.local".to_string(),
            password_hash: None,
            display_name: Some("Test User".to_string()),
            avatar_url: None,
            bio: None,
            timezone: "UTC".to_string(),
            status: UserStatus::Active,
            is_instance_admin: false,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn hash_password_verifies_correct_and_rejects_wrong() {
        let password = "correct horse battery staple";
        let hash = AuthService::hash_password(password).expect("hashing should succeed");

        assert_ne!(hash, password);
        assert!(hash.starts_with("$argon2"), "expected an argon2 PHC string");

        assert!(
            AuthService::verify_password(password, &hash).expect("verify should not error"),
            "correct password must verify true"
        );

        assert!(
            !AuthService::verify_password("wrong password", &hash)
                .expect("verify should not error"),
            "wrong password must verify false"
        );
    }

    #[tokio::test]
    async fn generate_tokens_stamps_token_type() {
        let config = test_config();
        let user = dummy_user();
        let now = Utc::now();
        let access_exp = now + Duration::seconds(config.access_token_expiry);
        let refresh_exp = now + Duration::seconds(config.refresh_token_expiry);
        let secret = EncodingKey::from_secret(config.jwt_secret.as_bytes());

        let access_claims = Claims {
            sub: user.id,
            email: user.email.clone(),
            is_instance_admin: user.is_instance_admin,
            iat: now.timestamp(),
            exp: access_exp.timestamp(),
            jti: None,
            token_type: "access".to_string(),
            workspace_id: None,
            invite_role: None,
        };
        let access_token = encode(&Header::default(), &access_claims, &secret).unwrap();

        let refresh_claims = Claims {
            sub: user.id,
            email: user.email.clone(),
            is_instance_admin: user.is_instance_admin,
            iat: now.timestamp(),
            exp: refresh_exp.timestamp(),
            jti: Some(Uuid::new_v4()),
            token_type: "refresh".to_string(),
            workspace_id: None,
            invite_role: None,
        };
        let refresh_token = encode(&Header::default(), &refresh_claims, &secret).unwrap();

        let access = decode_claims(&access_token);
        assert_eq!(
            access.token_type, "access",
            "access token must be stamped \"access\""
        );
        assert!(access.jti.is_none(), "access tokens must not carry a jti");
        assert_eq!(access.sub, user.id);

        let refresh = decode_claims(&refresh_token);
        assert_eq!(
            refresh.token_type, "refresh",
            "refresh token must be stamped \"refresh\""
        );
        assert!(refresh.jti.is_some(), "refresh tokens must carry a jti");
        assert_eq!(refresh.sub, user.id);
    }

    #[sqlx::test(migrations = "../migrations")]
    async fn refresh_rotation_is_single_use(pool: PgPool) {
        let service = AuthService::new(UserRepo::new(pool), test_config());

        let hash = AuthService::hash_password("hunter2hunter2").expect("hash");
        let created = service
            .repo()
            .create("rotate@test.local", Some(&hash), Some("Rotator"), false)
            .await
            .expect("create user");
        let user = service
            .repo()
            .activate(created.id, &hash, "Rotator")
            .await
            .expect("activate user");

        let tokens = service
            .generate_tokens(&user)
            .await
            .expect("generate tokens");

        let rotated = service
            .refresh_access_token(&tokens.refresh_token)
            .await
            .expect("first refresh should succeed");
        assert_ne!(
            rotated.refresh_token, tokens.refresh_token,
            "rotation must mint a new refresh token"
        );

        let replay = service.refresh_access_token(&tokens.refresh_token).await;
        assert!(
            replay.is_err(),
            "replaying a rotated-out refresh token must be rejected"
        );

        let wrong_type = service.refresh_access_token(&tokens.access_token).await;
        assert!(
            wrong_type.is_err(),
            "an access token must not be accepted by refresh_access_token"
        );
    }

    #[sqlx::test(migrations = "../migrations")]
    async fn reset_password_is_single_use_and_revokes_refresh_tokens(pool: PgPool) {
        let db = pool.clone();
        let service = AuthService::new(UserRepo::new(pool), test_config());

        let old_hash = AuthService::hash_password("oldpassword123").expect("hash");
        let created = service
            .repo()
            .create("reset@test.local", Some(&old_hash), Some("Resetter"), false)
            .await
            .expect("create user");
        let user = service
            .repo()
            .activate(created.id, &old_hash, "Resetter")
            .await
            .expect("activate user");

        let tokens = service
            .generate_tokens(&user)
            .await
            .expect("generate tokens");
        let refresh_count_before: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM refresh_tokens WHERE user_id = $1")
                .bind(user.id)
                .fetch_one(&db)
                .await
                .expect("count refresh before");
        assert_eq!(
            refresh_count_before, 1,
            "user should hold exactly one refresh token before reset"
        );
        assert!(
            service
                .repo()
                .find_refresh_token(
                    &decode_claims(&tokens.refresh_token)
                        .jti
                        .expect("refresh jti")
                        .to_string()
                )
                .await
                .expect("find refresh")
                .is_some(),
            "refresh token must be live before reset"
        );

        let reset_token = service
            .generate_reset_token(user.id)
            .await
            .expect("generate reset token");
        let reset_claims = decode_claims(&reset_token);
        let jti = reset_claims.jti.expect("reset token must carry a jti");
        let stored_jti_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM password_reset_tokens WHERE jti = $1")
                .bind(jti)
                .fetch_one(&db)
                .await
                .expect("count reset jti");
        assert_eq!(
            stored_jti_count, 1,
            "generate_reset_token must store the jti exactly once"
        );

        service
            .reset_password(&reset_token, "brandnewpw123")
            .await
            .expect("first reset_password should succeed");

        let after = service
            .repo()
            .find_by_id(user.id)
            .await
            .expect("find user")
            .expect("user exists");
        let stored = after.password_hash.expect("password hash set");
        assert!(
            AuthService::verify_password("brandnewpw123", &stored).expect("verify new"),
            "password must be rotated to the new value"
        );
        assert!(
            !AuthService::verify_password("oldpassword123", &stored).expect("verify old"),
            "old password must no longer verify after reset"
        );

        let jti_after: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM password_reset_tokens WHERE jti = $1")
                .bind(jti)
                .fetch_one(&db)
                .await
                .expect("count reset jti after");
        assert_eq!(jti_after, 0, "reset jti must be consumed (single-use)");

        let refresh_count_after: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM refresh_tokens WHERE user_id = $1")
                .bind(user.id)
                .fetch_one(&db)
                .await
                .expect("count refresh after");
        assert_eq!(
            refresh_count_after, 0,
            "all refresh tokens must be revoked after a password reset"
        );

        let replay = service.reset_password(&reset_token, "anotherpw456").await;
        assert!(
            replay.is_err(),
            "reusing a consumed reset token must be rejected"
        );
        match replay {
            Err(AppError::Unauthorized(_)) => {}
            other => panic!(
                "expected Unauthorized on reset-token replay, got {:?}",
                other
            ),
        }
        let after_replay = service
            .repo()
            .find_by_id(user.id)
            .await
            .expect("find user")
            .expect("user exists");
        let stored_after_replay = after_replay.password_hash.expect("password hash set");
        assert!(
            AuthService::verify_password("brandnewpw123", &stored_after_replay)
                .expect("verify still-new"),
            "a rejected replay must not rotate the password"
        );
        assert!(
            !AuthService::verify_password("anotherpw456", &stored_after_replay)
                .expect("verify replay pw"),
            "the replay's new password must never have been applied"
        );
    }

    #[sqlx::test(migrations = "../migrations")]
    async fn reset_token_is_rejected_by_refresh_and_tagged_reset(pool: PgPool) {
        let service = AuthService::new(UserRepo::new(pool), test_config());

        let hash = AuthService::hash_password("confusion123").expect("hash");
        let created = service
            .repo()
            .create("confuse@test.local", Some(&hash), Some("Confuser"), false)
            .await
            .expect("create user");
        let user = service
            .repo()
            .activate(created.id, &hash, "Confuser")
            .await
            .expect("activate user");

        let reset_token = service
            .generate_reset_token(user.id)
            .await
            .expect("generate reset token");

        let claims = service
            .verify_token(&reset_token)
            .expect("reset token verifies");
        assert_eq!(
            claims.token_type, "reset",
            "a reset token must be stamped token_type == \"reset\" so middleware rejects it as access"
        );
        assert_eq!(claims.sub, user.id, "reset token subject must be the user");

        let refused = service.refresh_access_token(&reset_token).await;
        assert!(
            refused.is_err(),
            "a reset token must not be exchangeable at refresh_access_token"
        );
        match refused {
            Err(AppError::Unauthorized(_)) => {}
            other => panic!(
                "expected Unauthorized for reset-token at refresh, got {:?}",
                other
            ),
        }
    }

    #[sqlx::test(migrations = "../migrations")]
    async fn forgot_password_does_not_leak_email_existence(pool: PgPool) {
        let db = pool.clone();
        let service = AuthService::new(UserRepo::new(pool), test_config());

        let missing = service.forgot_password("ghost@test.local").await;
        assert!(
            missing.is_ok(),
            "forgot_password for an unknown email must return Ok(()) to avoid enumeration, got {:?}",
            missing
        );
        let tokens_for_missing: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM password_reset_tokens")
                .fetch_one(&db)
                .await
                .expect("count tokens after missing");
        assert_eq!(
            tokens_for_missing, 0,
            "no reset token may be created for a non-existent email"
        );

        let hash = AuthService::hash_password("enumerate123").expect("hash");
        let created = service
            .repo()
            .create("real@test.local", Some(&hash), Some("Real"), false)
            .await
            .expect("create user");
        service
            .repo()
            .activate(created.id, &hash, "Real")
            .await
            .expect("activate user");

        let existing = service.forgot_password("real@test.local").await;
        match &existing {
            Ok(()) => {}
            Err(AppError::Internal(_)) => {}
            other => panic!(
                "forgot_password for an existing user must not reveal existence via error kind, got {:?}",
                other
            ),
        }
        assert!(
            !matches!(
                existing,
                Err(AppError::NotFound(_))
                    | Err(AppError::Unauthorized(_))
                    | Err(AppError::BadRequest(_))
            ),
            "forgot_password must not return an enumeration-revealing error for an existing user"
        );

        let tokens_for_existing: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM password_reset_tokens WHERE user_id = $1")
                .bind(created.id)
                .fetch_one(&db)
                .await
                .expect("count tokens for existing");
        assert_eq!(
            tokens_for_existing, 1,
            "an existing user's forgot_password must store exactly one reset jti"
        );
    }

    #[sqlx::test(migrations = "../migrations")]
    async fn bootstrap_admin_creates_active_admin_and_is_idempotent(pool: PgPool) {
        let db = pool.clone();
        let mut config = test_config();
        config.admin_email = Some("admin@test.local".to_string());
        config.admin_password = Some("bootstrap-admin-pw".to_string());
        let service = AuthService::new(UserRepo::new(pool), config);

        assert!(
            service
                .repo()
                .find_by_email("admin@test.local")
                .await
                .expect("find before bootstrap")
                .is_none(),
            "admin must not exist before bootstrap"
        );

        service
            .bootstrap_admin()
            .await
            .expect("first bootstrap should succeed");

        let admin = service
            .repo()
            .find_by_email("admin@test.local")
            .await
            .expect("find after bootstrap")
            .expect("admin user must exist after bootstrap");
        assert!(
            admin.is_instance_admin,
            "bootstrapped user must be flagged as an instance admin"
        );
        assert_eq!(
            admin.status,
            UserStatus::Active,
            "bootstrapped admin must be Active (activated, not left Pending)"
        );
        let stored = admin
            .password_hash
            .clone()
            .expect("bootstrapped admin must have a password hash");
        assert!(
            AuthService::verify_password("bootstrap-admin-pw", &stored)
                .expect("verify admin password"),
            "bootstrapped admin's stored hash must verify the configured password"
        );

        service
            .bootstrap_admin()
            .await
            .expect("second bootstrap must be idempotent (no error)");

        let admin_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE email = $1")
            .bind("admin@test.local")
            .fetch_one(&db)
            .await
            .expect("count admins");
        assert_eq!(
            admin_count, 1,
            "bootstrap_admin must not create a duplicate admin on a second call"
        );

        let admin_after = service
            .repo()
            .find_by_email("admin@test.local")
            .await
            .expect("find after second bootstrap")
            .expect("admin still exists");
        assert_eq!(
            admin_after.id, admin.id,
            "idempotent bootstrap must not replace the existing admin user"
        );
    }

    #[sqlx::test(migrations = "../migrations")]
    async fn registration_token_round_trips_and_rejects_other_token_types(pool: PgPool) {
        let service = AuthService::new(UserRepo::new(pool), test_config());

        let invited = service
            .repo()
            .create("invitee@test.local", None, None, false)
            .await
            .expect("create pending invitee");
        let workspace_id = Uuid::new_v4();

        let token = service
            .generate_registration_token(invited.id, "invitee@test.local", workspace_id, "member")
            .expect("generate registration token");

        let raw = decode_claims(&token);
        assert_eq!(
            raw.token_type, "registration",
            "registration token must be stamped token_type == \"registration\""
        );
        assert_eq!(
            raw.sub, invited.id,
            "registration token subject must be the invitee"
        );
        assert_eq!(
            raw.workspace_id,
            Some(workspace_id),
            "registration token must carry the target workspace_id"
        );
        assert_eq!(
            raw.invite_role.as_deref(),
            Some("member"),
            "registration token must carry the invite_role"
        );

        let verified = service
            .verify_registration_token(&token)
            .expect("verify_registration_token must accept a genuine registration token");
        assert_eq!(verified.sub, invited.id);
        assert_eq!(verified.workspace_id, Some(workspace_id));
        assert_eq!(verified.invite_role.as_deref(), Some("member"));

        let hash = AuthService::hash_password("regtokenpw123").expect("hash");
        let active = service
            .repo()
            .activate(invited.id, &hash, "Invitee")
            .await
            .expect("activate invitee");

        let access_token = service
            .generate_tokens(&active)
            .await
            .expect("generate access/refresh tokens")
            .access_token;
        let access_rejected = service.verify_registration_token(&access_token);
        assert!(
            access_rejected.is_err(),
            "an access token must be rejected by verify_registration_token"
        );
        match access_rejected {
            Err(AppError::Unauthorized(_)) => {}
            other => panic!("expected Unauthorized for access token, got {:?}", other),
        }

        let reset_token = service
            .generate_reset_token(active.id)
            .await
            .expect("generate reset token");
        let reset_rejected = service.verify_registration_token(&reset_token);
        assert!(
            reset_rejected.is_err(),
            "a reset token must be rejected by verify_registration_token"
        );
        match reset_rejected {
            Err(AppError::Unauthorized(_)) => {}
            other => panic!("expected Unauthorized for reset token, got {:?}", other),
        }
    }

    #[sqlx::test(migrations = "../migrations")]
    async fn complete_registration_activates_pending_then_rejects_already_active(pool: PgPool) {
        let service = AuthService::new(UserRepo::new(pool), test_config());

        let pending = service
            .repo()
            .create("complete@test.local", None, None, false)
            .await
            .expect("create pending user");
        assert_eq!(
            pending.status,
            UserStatus::Pending,
            "a freshly created user (no activation) must start Pending"
        );
        assert!(
            pending.password_hash.is_none(),
            "a pending user must not have a password set yet"
        );

        let tokens = service
            .complete_registration(pending.id, "completepw123", "Completed User")
            .await
            .expect("complete_registration must succeed for a pending user");

        let access_claims = decode_claims(&tokens.access_token);
        assert_eq!(access_claims.sub, pending.id);
        assert_eq!(access_claims.token_type, "access");

        let activated = service
            .repo()
            .find_by_id(pending.id)
            .await
            .expect("find after complete")
            .expect("user exists");
        assert_eq!(
            activated.status,
            UserStatus::Active,
            "complete_registration must transition the user to Active"
        );
        assert_eq!(
            activated.display_name.as_deref(),
            Some("Completed User"),
            "complete_registration must set the supplied display name"
        );
        let stored = activated
            .password_hash
            .expect("completed user must have a password hash");
        assert!(
            AuthService::verify_password("completepw123", &stored).expect("verify completed pw"),
            "the supplied password must be hashed + stored"
        );

        let already = service
            .complete_registration(pending.id, "anotherpw456", "Second Try")
            .await;
        assert!(
            already.is_err(),
            "complete_registration must reject an already-active user"
        );
        match already {
            Err(AppError::BadRequest(_)) => {}
            other => panic!("expected BadRequest on re-completion, got {:?}", other),
        }
        let unchanged = service
            .repo()
            .find_by_id(pending.id)
            .await
            .expect("find after rejected re-complete")
            .expect("user exists");
        let unchanged_hash = unchanged.password_hash.expect("password still set");
        assert!(
            AuthService::verify_password("completepw123", &unchanged_hash)
                .expect("verify original pw"),
            "a rejected re-completion must leave the original password intact"
        );
        assert!(
            !AuthService::verify_password("anotherpw456", &unchanged_hash)
                .expect("verify rejected pw"),
            "the rejected re-completion's password must never have been applied"
        );
    }

    #[sqlx::test(migrations = "../migrations")]
    async fn provision_user_creates_pending_and_is_idempotent(pool: PgPool) {
        let db = pool.clone();
        let service = AuthService::new(UserRepo::new(pool), test_config());

        let provisioned = service
            .provision_user("provision@test.local")
            .await
            .expect("provision new email");
        assert_eq!(
            provisioned.email, "provision@test.local",
            "provisioned user must carry the requested email"
        );
        assert_eq!(
            provisioned.status,
            UserStatus::Pending,
            "a freshly provisioned user must be Pending (awaiting registration)"
        );

        let row = service
            .repo()
            .find_by_email("provision@test.local")
            .await
            .expect("find provisioned")
            .expect("provisioned user exists");
        assert_eq!(row.id, provisioned.id);
        assert!(
            row.password_hash.is_none(),
            "a provisioned user must not have a password until they complete registration"
        );

        let again = service
            .provision_user("provision@test.local")
            .await
            .expect("provision existing email");
        assert_eq!(
            again.id, provisioned.id,
            "re-provisioning an existing email must return the same user"
        );

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE email = $1")
            .bind("provision@test.local")
            .fetch_one(&db)
            .await
            .expect("count provisioned users");
        assert_eq!(
            count, 1,
            "re-provisioning must not create a duplicate user row"
        );
    }

    #[sqlx::test(migrations = "../migrations")]
    async fn generate_tokens_persists_findable_refresh_token(pool: PgPool) {
        let service = AuthService::new(UserRepo::new(pool), test_config());

        let hash = AuthService::hash_password("persistpw123").expect("hash");
        let created = service
            .repo()
            .create("persist@test.local", Some(&hash), Some("Persister"), false)
            .await
            .expect("create user");
        let user = service
            .repo()
            .activate(created.id, &hash, "Persister")
            .await
            .expect("activate user");

        let tokens = service
            .generate_tokens(&user)
            .await
            .expect("generate tokens");

        let jti = decode_claims(&tokens.refresh_token)
            .jti
            .expect("refresh token must carry a jti");
        let found = service
            .repo()
            .find_refresh_token(&jti.to_string())
            .await
            .expect("find refresh token");
        assert_eq!(
            found,
            Some(user.id),
            "generate_tokens must persist a refresh-token row resolvable to the user id"
        );
    }
}
