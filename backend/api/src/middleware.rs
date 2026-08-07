use axum::extract::{FromRequestParts, Request};
use axum::http::header::{AUTHORIZATION, COOKIE};
use axum::http::request::Parts;
use axum::middleware::Next;
use axum::response::Response;
use jsonwebtoken::{decode, DecodingKey, Validation};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use shared_common::errors::AppError;

fn default_token_type() -> String {
    "access".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    pub email: String,
    pub is_instance_admin: bool,
    pub iat: i64,
    pub exp: i64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub jti: Option<Uuid>,
    #[serde(default = "default_token_type")]
    pub token_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invite_role: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: Uuid,
    pub is_instance_admin: bool,
    pub jti: Option<Uuid>,
}

pub async fn auth_middleware(request: Request, next: Next) -> Result<Response, AppError> {
    let (mut parts, body) = request.into_parts();

    let jwt_secret = parts
        .extensions
        .get::<JwtSecret>()
        .ok_or_else(|| AppError::Internal("JWT secret not configured".into()))?
        .0
        .clone();

    let token = extract_cookie_token(&parts.headers)
        .or_else(|| {
            parts
                .headers
                .get(AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer "))
                .map(std::string::ToString::to_string)
        })
        .ok_or_else(|| AppError::Unauthorized("Missing authentication".into()))?;

    let token_data = decode::<Claims>(
        &token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|_| AppError::Unauthorized("Invalid or expired token".into()))?;

    if token_data.claims.token_type != "access" {
        return Err(AppError::Unauthorized("Invalid or expired token".into()));
    }

    if let Some(revocation) = parts.extensions.get::<RevocationStore>() {
        if revocation.is_revoked(&token_data.claims).await {
            return Err(AppError::Unauthorized("Session revoked".into()));
        }
    }

    let auth_user = AuthUser {
        user_id: token_data.claims.sub,
        is_instance_admin: token_data.claims.is_instance_admin,
        jti: token_data.claims.jti,
    };

    parts.extensions.insert(auth_user);

    let request = Request::from_parts(parts, body);
    Ok(next.run(request).await)
}

pub async fn admin_middleware(request: Request, next: Next) -> Result<Response, AppError> {
    let auth = request
        .extensions()
        .get::<AuthUser>()
        .cloned()
        .ok_or_else(|| AppError::Unauthorized("Not authenticated".into()))?;

    if !auth.is_instance_admin {
        return Err(AppError::Forbidden("Requires instance admin".into()));
    }

    Ok(next.run(request).await)
}

#[derive(Debug, Clone)]
pub struct JwtSecret(pub String);

/// A revocation cuts off every token issued up to the moment it happened.
/// Storing that moment rather than a boolean is what lets a user sign in again
/// right after their own sessions are revoked — a flat "this user is blocked"
/// flag would also reject the token they just obtained with their new password.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevocationRecord {
    pub at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub except_jti: Option<Uuid>,
}

impl RevocationRecord {
    pub fn covers(&self, claims: &Claims) -> bool {
        // `iat` has second granularity, so a token minted in the same second as
        // the revocation is indistinguishable from one minted just before it.
        // Cover it: a survivor that should have died is worse than a re-login
        // that has to be retried a second later.
        if claims.iat > self.at {
            return false;
        }
        !matches!((self.except_jti, claims.jti), (Some(except), Some(jti)) if except == jti)
    }
}

#[derive(Clone)]
pub struct RevocationStore(pub redis::aio::ConnectionManager);

impl RevocationStore {
    pub fn key(user_id: Uuid) -> String {
        format!("revoked:{user_id}")
    }

    pub async fn revoke(&self, user_id: Uuid, record: &RevocationRecord, ttl_secs: u64) {
        let payload = match serde_json::to_string(record) {
            Ok(payload) => payload,
            Err(e) => {
                tracing::warn!("failed to encode revocation for user {}: {}", user_id, e);
                return;
            }
        };
        let mut conn = self.0.clone();
        let res: redis::RedisResult<()> = conn.set_ex(Self::key(user_id), payload, ttl_secs).await;
        if let Err(e) = res {
            tracing::warn!("failed to revoke sessions for user {}: {}", user_id, e);
        }
    }

    pub async fn restore(&self, user_id: Uuid) {
        let mut conn = self.0.clone();
        let res: redis::RedisResult<()> = conn.del(Self::key(user_id)).await;
        if let Err(e) = res {
            tracing::warn!("failed to clear revocation for user {}: {}", user_id, e);
        }
    }

    pub async fn is_revoked(&self, claims: &Claims) -> bool {
        let mut conn = self.0.clone();
        let raw: redis::RedisResult<Option<String>> = conn.get(Self::key(claims.sub)).await;
        let payload = match raw {
            Ok(Some(payload)) => payload,
            Ok(None) => return false,
            Err(e) => {
                tracing::warn!("revocation lookup failed for user {}: {}", claims.sub, e);
                return false;
            }
        };
        match serde_json::from_str::<RevocationRecord>(&payload) {
            Ok(record) => record.covers(claims),
            // A record written by an older build is the bare value `1`. Treat
            // anything unparseable as still revoked: the key only exists because
            // somebody revoked this user, and it expires on its own.
            Err(_) => true,
        }
    }
}

fn extract_cookie_token(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get(COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| {
            s.split(';').find_map(|part| {
                part.trim()
                    .strip_prefix("access_token=")
                    .map(std::string::ToString::to_string)
            })
        })
}

#[axum::async_trait]
impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthUser>()
            .cloned()
            .ok_or_else(|| AppError::Unauthorized("Not authenticated".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claims(iat: i64, jti: Option<Uuid>) -> Claims {
        Claims {
            sub: Uuid::new_v4(),
            email: "user@test.local".into(),
            is_instance_admin: false,
            iat,
            exp: iat + 3600,
            jti,
            token_type: "access".into(),
            workspace_id: None,
            invite_role: None,
        }
    }

    #[test]
    fn revokes_tokens_issued_before_the_revocation() {
        let record = RevocationRecord {
            at: 1_000,
            except_jti: None,
        };
        assert!(record.covers(&claims(999, None)));
    }

    #[test]
    fn leaves_tokens_issued_after_the_revocation_alone() {
        let record = RevocationRecord {
            at: 1_000,
            except_jti: None,
        };
        assert!(!record.covers(&claims(1_001, None)));
    }

    #[test]
    fn covers_the_revocation_second_itself() {
        let record = RevocationRecord {
            at: 1_000,
            except_jti: None,
        };
        assert!(record.covers(&claims(1_000, None)));
    }

    #[test]
    fn spares_the_excepted_session() {
        let survivor = Uuid::new_v4();
        let record = RevocationRecord {
            at: 1_000,
            except_jti: Some(survivor),
        };
        assert!(!record.covers(&claims(999, Some(survivor))));
        assert!(record.covers(&claims(999, Some(Uuid::new_v4()))));
        assert!(record.covers(&claims(999, None)));
    }
}
