use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::AuthService;
use crate::auth::models::{AuthTokens, RegistrationInvite, User, UserStatus};
use crate::auth::repo::UserRepo;
use crate::middleware::Claims;

impl AuthService {
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
            .await?
            .ok_or_else(|| {
                AppError::Unauthorized("Refresh token has been revoked or expired".into())
            })?;

        if uid != claims.sub {
            return Err(AppError::Unauthorized("Invalid refresh token".into()));
        }

        let user = self
            .repo
            .find_by_id(claims.sub)
            .await?
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
            jti: Some(new_jti),
            token_type: "access".to_string(),
            invite_id: None,
        };
        let access_token = self
            .config
            .jwt_keys
            .encode(&access_claims)
            .map_err(|e| AppError::Internal(format!("Token generation failed: {e}")))?;

        let refresh_claims = Claims {
            sub: user.id,
            email: user.email.clone(),
            is_instance_admin: user.is_instance_admin,
            iat: now.timestamp(),
            exp: refresh_exp.timestamp(),
            jti: Some(new_jti),
            token_type: "refresh".to_string(),
            invite_id: None,
        };
        let new_refresh_token = self
            .config
            .jwt_keys
            .encode(&refresh_claims)
            .map_err(|e| AppError::Internal(format!("Token generation failed: {e}")))?;

        let mut tx = self.repo.begin().await?;
        UserRepo::delete_refresh_token_tx(&mut tx, &jti.to_string()).await?;
        UserRepo::store_refresh_token_tx(&mut tx, user.id, &new_jti.to_string(), refresh_exp)
            .await?;
        tx.commit().await?;

        Ok(AuthTokens {
            access_token,
            refresh_token: new_refresh_token,
            expires_in: self.config.access_token_expiry,
            user: user.into(),
        })
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
            jti: Some(jti),
            token_type: "access".to_string(),
            invite_id: None,
        };

        let access_token = self
            .config
            .jwt_keys
            .encode(&access_claims)
            .map_err(|e| AppError::Internal(format!("Token generation failed: {e}")))?;

        let refresh_claims = Claims {
            sub: user.id,
            email: user.email.clone(),
            is_instance_admin: user.is_instance_admin,
            iat: now.timestamp(),
            exp: refresh_exp.timestamp(),
            jti: Some(jti),
            token_type: "refresh".to_string(),
            invite_id: None,
        };

        let refresh_token = self
            .config
            .jwt_keys
            .encode(&refresh_claims)
            .map_err(|e| AppError::Internal(format!("Token generation failed: {e}")))?;

        self.repo
            .store_refresh_token(user.id, &jti.to_string(), refresh_exp)
            .await?;

        Ok(AuthTokens {
            access_token,
            refresh_token,
            expires_in: self.config.access_token_expiry,
            user: user.clone().into(),
        })
    }

    pub fn verify_token(&self, token: &str) -> AppResult<Claims> {
        self.config
            .jwt_keys
            .verifying()
            .decode::<Claims>(token)
            .map_err(|_| AppError::Unauthorized("Invalid or expired token".into()))
    }

    #[cfg(test)]
    pub async fn issue_reset_token_for_test(&self, user_id: Uuid) -> AppResult<String> {
        self.generate_reset_token(user_id).await
    }

    pub(super) async fn generate_reset_token(&self, user_id: Uuid) -> AppResult<String> {
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
            invite_id: None,
        };
        let token = self
            .config
            .jwt_keys
            .encode(&claims)
            .map_err(|e| AppError::Internal(format!("Reset token generation failed: {e}")))?;

        self.repo.store_reset_jti(jti, user_id, exp).await?;

        Ok(token)
    }

    pub fn generate_registration_token(
        &self,
        user_id: Uuid,
        email: &str,
        invite_id: Uuid,
        expires_at: DateTime<Utc>,
    ) -> AppResult<String> {
        let claims = Claims {
            sub: user_id,
            email: email.to_string(),
            is_instance_admin: false,
            iat: Utc::now().timestamp(),
            exp: expires_at.timestamp(),
            jti: None,
            token_type: "registration".to_string(),
            invite_id: Some(invite_id),
        };
        self.config
            .jwt_keys
            .encode(&claims)
            .map_err(|e| AppError::Internal(format!("Registration token generation failed: {e}")))
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

    pub fn registration_invite(&self, token: &str) -> AppResult<RegistrationInvite> {
        let claims = self.verify_registration_token(token)?;
        let invite_id = claims
            .invite_id
            .ok_or_else(|| AppError::Unauthorized("Invalid or expired invite".into()))?;
        Ok(RegistrationInvite {
            user_id: claims.sub,
            email: claims.email,
            invite_id,
        })
    }
}
