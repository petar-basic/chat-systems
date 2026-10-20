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

/// The peer address, when the server was started with `into_make_service_with_connect_info`.
/// Behind a proxy it is the proxy's address, which is why the rate limiters that
/// use it also key on something the caller controls.
#[derive(Debug, Clone, Copy)]
pub struct PeerAddr(pub Option<std::net::SocketAddr>);

impl<S> FromRequestParts<S> for PeerAddr
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self(
            parts
                .extensions
                .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
                .map(|info| info.0),
        ))
    }
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
        match revocation.revocation_state(&token_data.claims).await {
            RevocationState::Valid => {}
            RevocationState::Revoked => {
                return Err(AppError::Unauthorized("Session revoked".into()));
            }
            // Deliberately not a 401: the client should retry in a moment, not
            // throw the session away and send somebody back to a login form
            // because a cache was briefly unreachable.
            RevocationState::Unknown => {
                return Err(AppError::ServiceUnavailable(
                    "Cannot verify the session right now. Please retry.".into(),
                ));
            }
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

/// Short enough that a healthy Redis is never noticed, long enough that a busy
/// one is not mistaken for a dead one.
const REVOCATION_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(250);
const REVOCATION_RETRIES: u8 = 1;
const REVOCATION_RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(50);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevocationState {
    Valid,
    Revoked,
    /// The store could not be reached. Not the same as valid, and the whole
    /// reason this enum exists rather than a `bool`.
    Unknown,
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

    /// Answers "is this session still valid", or refuses to answer.
    ///
    /// It used to treat a Redis error as "not revoked", which meant that during
    /// a Redis incident, suspending somebody or deprovisioning them through
    /// SCIM quietly did nothing for the life of their access token. The whole
    /// promise of CS-033 runs through this call.
    ///
    /// A blanket fail-closed is not the answer either: it turns a blip in a
    /// cache into a total outage. So a slow Redis gets one more chance, and
    /// only a Redis that will not answer at all stops the request.
    pub async fn revocation_state(&self, claims: &Claims) -> RevocationState {
        let mut last_error = None;

        for attempt in 0..=REVOCATION_RETRIES {
            let mut conn = self.0.clone();
            let read = conn.get::<_, Option<String>>(Self::key(claims.sub));
            let lookup = tokio::time::timeout(REVOCATION_TIMEOUT, read);

            match lookup.await {
                Ok(Ok(Some(payload))) => return Self::decide(claims, &payload),
                Ok(Ok(None)) => return RevocationState::Valid,
                Ok(Err(e)) => last_error = Some(e.to_string()),
                Err(_) => last_error = Some("timed out".to_string()),
            }

            if attempt < REVOCATION_RETRIES {
                tokio::time::sleep(REVOCATION_RETRY_DELAY).await;
            }
        }

        metrics::counter!("auth_revocation_lookup_failures_total").increment(1);
        tracing::warn!(
            user_id = %claims.sub,
            "revocation lookup failed, refusing the request: {}",
            last_error.unwrap_or_default()
        );
        RevocationState::Unknown
    }

    pub async fn is_revoked(&self, claims: &Claims) -> bool {
        matches!(
            self.revocation_state(claims).await,
            RevocationState::Revoked
        )
    }

    fn decide(claims: &Claims, payload: &str) -> RevocationState {
        match serde_json::from_str::<RevocationRecord>(payload) {
            Ok(record) if record.covers(claims) => RevocationState::Revoked,
            Ok(_) => RevocationState::Valid,
            // A record written by an older build is the bare value `1`. Treat
            // anything unparseable as still revoked: the key only exists because
            // somebody revoked this user, and it expires on its own.
            Err(_) => RevocationState::Revoked,
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
