use axum::extract::{FromRequestParts, Request};
use axum::http::header::{AUTHORIZATION, COOKIE};
use axum::http::request::Parts;
use axum::middleware::Next;
use axum::response::Response;
use jsonwebtoken::{decode, DecodingKey, Validation};
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
                .map(|s| s.to_string())
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

    let auth_user = AuthUser {
        user_id: token_data.claims.sub,
        is_instance_admin: token_data.claims.is_instance_admin,
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

fn extract_cookie_token(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get(COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| {
            s.split(';').find_map(|part| {
                part.trim()
                    .strip_prefix("access_token=")
                    .map(|v| v.to_string())
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
