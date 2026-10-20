use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use serde::Deserialize;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::repo::ScimToken;
use super::service;
use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::auth::models::{User, UserStatus};
use crate::middleware::AuthUser;
use crate::middleware::PeerAddr;
use crate::state::AppState;

const USER_SCHEMA: &str = "urn:ietf:params:scim:schemas:core:2.0:User";
const LIST_SCHEMA: &str = "urn:ietf:params:scim:api:messages:2.0:ListResponse";
const ERROR_SCHEMA: &str = "urn:ietf:params:scim:api:messages:2.0:Error";

pub fn router(state: Arc<AppState>) -> Router {
    // The caller is a provisioning system holding its own token, so this subtree
    // sits outside `auth_middleware` entirely rather than pretending to have a
    // session.
    let scim = Router::new()
        .route("/scim/v2/Users", get(list_users).post(create_user))
        .route("/scim/v2/Users/{id}", get(get_user))
        .route("/scim/v2/Users/{id}", patch(patch_user))
        .route("/scim/v2/Users/{id}", delete(delete_user))
        .with_state(state.clone());

    let tokens = Router::new()
        .route("/admin/scim/tokens", get(list_tokens).post(create_token))
        .route("/admin/scim/tokens/{id}", delete(revoke_token))
        .route("/admin/scim/tokens/{id}/rotate", post(rotate_token))
        .layer(axum::middleware::from_fn(
            crate::middleware::admin_middleware,
        ));

    crate::protected(state, tokens).merge(scim)
}

/// SCIM has its own error envelope and identity providers read it. An app-shaped
/// error body here shows up in Okta as an opaque failure.
#[derive(Debug)]
struct ScimError {
    status: StatusCode,
    scim_type: Option<&'static str>,
    detail: String,
}

impl ScimError {
    fn new(status: StatusCode, detail: impl Into<String>) -> Self {
        Self {
            status,
            scim_type: None,
            detail: detail.into(),
        }
    }

    fn typed(status: StatusCode, scim_type: &'static str, detail: impl Into<String>) -> Self {
        Self {
            status,
            scim_type: Some(scim_type),
            detail: detail.into(),
        }
    }
}

impl From<AppError> for ScimError {
    fn from(error: AppError) -> Self {
        match error {
            AppError::Conflict(detail) => Self::typed(StatusCode::CONFLICT, "uniqueness", detail),
            AppError::BadRequest(detail) | AppError::Validation(detail) => {
                Self::typed(StatusCode::BAD_REQUEST, "invalidValue", detail)
            }
            AppError::NotFound(detail) => Self::new(StatusCode::NOT_FOUND, detail),
            AppError::Unauthorized(detail) => Self::new(StatusCode::UNAUTHORIZED, detail),
            AppError::Forbidden(detail) => Self::new(StatusCode::FORBIDDEN, detail),
            AppError::TooManyRequests { message, .. } => {
                Self::new(StatusCode::TOO_MANY_REQUESTS, message)
            }
            AppError::ServiceUnavailable(detail) => {
                Self::new(StatusCode::SERVICE_UNAVAILABLE, detail)
            }
            AppError::Internal(detail) | AppError::Database(detail) => {
                tracing::error!(error = %detail, "scim request failed");
                Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
            }
        }
    }
}

impl IntoResponse for ScimError {
    fn into_response(self) -> Response {
        let mut body = serde_json::json!({
            "schemas": [ERROR_SCHEMA],
            "status": self.status.as_u16().to_string(),
            "detail": self.detail,
        });
        if let Some(scim_type) = self.scim_type {
            body["scimType"] = serde_json::json!(scim_type);
        }
        (self.status, Json(body)).into_response()
    }
}

type ScimResult<T> = Result<T, ScimError>;

fn user_resource(user: &User, public_url: &str) -> serde_json::Value {
    serde_json::json!({
        "schemas": [USER_SCHEMA],
        "id": user.id,
        "userName": user.email,
        "displayName": user.display_name,
        "name": { "formatted": user.display_name },
        "emails": [{ "value": user.email, "primary": true, "type": "work" }],
        "active": user.status != UserStatus::Suspended,
        "meta": {
            "resourceType": "User",
            "created": user.created_at,
            "lastModified": user.updated_at,
            "location": format!("{public_url}/api/scim/v2/Users/{}", user.id),
        },
    })
}

/// Authenticates the caller and bounds it. The token is the only credential, so
/// a wrong one must cost the same as a right one plus nothing: the per-IP limit
/// runs before the lookup.
async fn authenticate(
    state: &AppState,
    headers: &HeaderMap,
    peer: Option<std::net::SocketAddr>,
) -> ScimResult<ScimToken> {
    let mut conn = state.redis.clone();
    if let Some(ip) = crate::net::client_ip(headers, peer, &state.config.trusted_proxies) {
        crate::rate_limit::enforce(
            &mut conn,
            &format!("rate_limit:scim_ip:{ip}"),
            120,
            60,
            crate::rate_limit::LimiterFailure::Open,
        )
        .await?;
    }

    let presented = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| ScimError::new(StatusCode::UNAUTHORIZED, "A bearer token is required"))?;

    let token = state
        .scim_repo
        .find_active(presented)
        .await
        .map_err(|e| ScimError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| ScimError::new(StatusCode::UNAUTHORIZED, "Invalid SCIM token"))?;

    state.scim_repo.touch(token.id).await;
    Ok(token)
}

#[derive(Debug, Deserialize)]
struct ListQuery {
    filter: Option<String>,
    #[serde(rename = "startIndex")]
    start_index: Option<i64>,
    count: Option<i64>,
}

/// Only `userName eq "…"` is understood, which is the one filter Okta and Entra
/// send. Anything else is refused rather than silently answered with everybody.
fn username_filter(filter: &str) -> Result<String, ScimError> {
    let (attribute, rest) = filter.trim().split_once(" eq ").ok_or_else(|| {
        ScimError::typed(
            StatusCode::BAD_REQUEST,
            "invalidFilter",
            "Only 'userName eq \"value\"' is supported",
        )
    })?;
    if !attribute.trim().eq_ignore_ascii_case("userName") {
        return Err(ScimError::typed(
            StatusCode::BAD_REQUEST,
            "invalidFilter",
            "Only userName may be filtered on",
        ));
    }
    Ok(rest.trim().trim_matches('"').to_lowercase())
}

async fn list_users(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    PeerAddr(peer): PeerAddr,
    Query(query): Query<ListQuery>,
) -> ScimResult<Json<serde_json::Value>> {
    authenticate(&state, &headers, peer).await?;

    let start_index = query.start_index.unwrap_or(1).max(1);
    let count = query.count.unwrap_or(100).clamp(1, 200);

    let users: Vec<User> = match query.filter.as_deref() {
        Some(filter) => {
            let email = username_filter(filter)?;
            state
                .auth_service
                .repo()
                .find_by_email(&email)
                .await
                .map_err(|e| ScimError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
                .into_iter()
                .collect()
        }
        None => state
            .auth_service
            .repo()
            .list_page(start_index - 1, count)
            .await
            .map_err(|e| ScimError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?,
    };

    let total: i64 = match query.filter {
        Some(_) => users.len() as i64,
        None => state
            .auth_service
            .repo()
            .count()
            .await
            .map_err(|e| ScimError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?,
    };

    Ok(Json(serde_json::json!({
        "schemas": [LIST_SCHEMA],
        "totalResults": total,
        "startIndex": start_index,
        "itemsPerPage": users.len(),
        "Resources": users
            .iter()
            .map(|u| user_resource(u, &state.config.public_url))
            .collect::<Vec<_>>(),
    })))
}

async fn get_user(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    PeerAddr(peer): PeerAddr,
    Path(id): Path<Uuid>,
) -> ScimResult<Json<serde_json::Value>> {
    authenticate(&state, &headers, peer).await?;
    let user = load(&state, id).await?;
    Ok(Json(user_resource(&user, &state.config.public_url)))
}

async fn load(state: &AppState, id: Uuid) -> ScimResult<User> {
    state
        .auth_service
        .repo()
        .find_by_id(id)
        .await
        .map_err(|e| ScimError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| ScimError::new(StatusCode::NOT_FOUND, "No such user"))
}

#[derive(Debug, Deserialize)]
struct CreateUser {
    #[serde(rename = "userName")]
    user_name: String,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    active: Option<bool>,
}

async fn create_user(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    PeerAddr(peer): PeerAddr,
    ip: ClientIp,
    Json(body): Json<CreateUser>,
) -> ScimResult<Response> {
    let token = authenticate(&state, &headers, peer).await?;

    let user = service::provision(
        &state,
        &body.user_name,
        body.display_name.as_deref(),
        token.id,
        &ip,
    )
    .await?;

    if body.active == Some(false) {
        service::deactivate(&state, user.id, token.id, &ip).await?;
    }

    let user = load(&state, user.id).await?;
    Ok((
        StatusCode::CREATED,
        Json(user_resource(&user, &state.config.public_url)),
    )
        .into_response())
}

#[derive(Debug, Deserialize)]
struct PatchOp {
    #[serde(rename = "Operations", default)]
    operations: Vec<Operation>,
}

#[derive(Debug, Deserialize)]
struct Operation {
    op: String,
    path: Option<String>,
    value: Option<serde_json::Value>,
}

/// Two shapes reach this handler: `{path: "active", value: false}` and Entra's
/// pathless `{value: {"active": false}}`. Reading only one of them means
/// deprovisioning silently does nothing for half the market.
fn changes(body: &PatchOp) -> Vec<(String, serde_json::Value)> {
    let mut out = Vec::new();
    for op in &body.operations {
        if !op.op.eq_ignore_ascii_case("replace") && !op.op.eq_ignore_ascii_case("add") {
            continue;
        }
        match (&op.path, &op.value) {
            (Some(path), Some(value)) => out.push((path.to_lowercase(), value.clone())),
            (None, Some(serde_json::Value::Object(map))) => {
                for (key, value) in map {
                    out.push((key.to_lowercase(), value.clone()));
                }
            }
            _ => {}
        }
    }
    out
}

fn as_bool(value: &serde_json::Value) -> Option<bool> {
    value
        .as_bool()
        .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
}

async fn patch_user(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    PeerAddr(peer): PeerAddr,
    ip: ClientIp,
    Path(id): Path<Uuid>,
    Json(body): Json<PatchOp>,
) -> ScimResult<Json<serde_json::Value>> {
    let token = authenticate(&state, &headers, peer).await?;
    let user = load(&state, id).await?;

    for (path, value) in changes(&body) {
        match path.as_str() {
            "active" => match as_bool(&value) {
                Some(false) => service::deactivate(&state, user.id, token.id, &ip).await?,
                Some(true) => service::reactivate(&state, user.id, token.id, &ip).await?,
                None => {
                    return Err(ScimError::typed(
                        StatusCode::BAD_REQUEST,
                        "invalidValue",
                        "active must be a boolean",
                    ))
                }
            },
            "displayname" => {
                if let Some(name) = value.as_str() {
                    service::rename(&state, user.id, name).await?;
                }
            }
            _ => {}
        }
    }

    let user = load(&state, id).await?;
    Ok(Json(user_resource(&user, &state.config.public_url)))
}

/// Deactivation, never destruction. Erasure is a deliberate act by an
/// administrator of this instance; an identity provider retrying a delete must
/// not be able to take a workspace's history with it.
async fn delete_user(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    PeerAddr(peer): PeerAddr,
    ip: ClientIp,
    Path(id): Path<Uuid>,
) -> ScimResult<StatusCode> {
    let token = authenticate(&state, &headers, peer).await?;
    let user = load(&state, id).await?;
    service::deactivate(&state, user.id, token.id, &ip).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn generate_token() -> String {
    use base64::Engine;
    use rand::RngExt;
    let mut bytes = [0u8; 24];
    rand::rng().fill(&mut bytes[..]);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

#[derive(Debug, Deserialize)]
struct CreateToken {
    description: Option<String>,
}

async fn list_tokens(State(state): State<Arc<AppState>>) -> AppResult<Json<serde_json::Value>> {
    let tokens = state.scim_repo.list().await?;
    Ok(Json(serde_json::json!({ "data": tokens })))
}

/// Returned once. There is nowhere to read it from afterwards, which is the
/// point: a token an admin can re-read is a token a stolen admin session can
/// read too.
async fn create_token(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Json(body): Json<CreateToken>,
) -> AppResult<Json<serde_json::Value>> {
    let plaintext = generate_token();
    let token = state
        .scim_repo
        .create(&plaintext, body.description.as_deref(), auth.user_id)
        .await?;

    audit::record(
        &state,
        AuditEntry::new(AuditAction::ScimTokenCreated, auth.user_id)
            .resource(token.id)
            .ip(&ip),
    )
    .await;

    Ok(Json(serde_json::json!({
        "id": token.id,
        "description": token.description,
        "token": plaintext,
    })))
}

async fn revoke_token(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    if !state.scim_repo.revoke(id).await? {
        return Err(AppError::NotFound("No such token".into()));
    }
    audit::record(
        &state,
        AuditEntry::new(AuditAction::ScimTokenRevoked, auth.user_id)
            .resource(id)
            .ip(&ip),
    )
    .await;
    Ok(Json(serde_json::json!({ "status": "revoked" })))
}

async fn rotate_token(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let existing = state
        .scim_repo
        .list()
        .await?
        .into_iter()
        .find(|t| t.id == id)
        .ok_or_else(|| AppError::NotFound("No such token".into()))?;

    let plaintext = generate_token();
    let replacement = state
        .scim_repo
        .create(&plaintext, existing.description.as_deref(), auth.user_id)
        .await?;
    state.scim_repo.revoke(id).await?;

    audit::record(
        &state,
        AuditEntry::new(AuditAction::ScimTokenRotated, auth.user_id)
            .resource(replacement.id)
            .ip(&ip)
            .details(serde_json::json!({ "replaced": id })),
    )
    .await;

    Ok(Json(serde_json::json!({
        "id": replacement.id,
        "description": replacement.description,
        "token": plaintext,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_only_filter_providers_send_is_understood() {
        assert_eq!(
            username_filter("userName eq \"Someone@Example.com\"").expect("parses"),
            "someone@example.com"
        );
        assert_eq!(
            username_filter("username eq \"x@y.z\"").expect("case is not a schema"),
            "x@y.z"
        );
    }

    #[test]
    fn anything_else_is_refused_rather_than_answered_with_everybody() {
        assert!(username_filter("displayName eq \"x\"").is_err());
        assert!(username_filter("userName co \"admin\"").is_err());
        assert!(username_filter("").is_err());
    }

    #[test]
    fn both_patch_shapes_reach_the_same_change() {
        let with_path: PatchOp = serde_json::from_str(
            r#"{"Operations":[{"op":"replace","path":"active","value":false}]}"#,
        )
        .expect("parse");
        let pathless: PatchOp =
            serde_json::from_str(r#"{"Operations":[{"op":"Replace","value":{"active":false}}]}"#)
                .expect("parse");

        for body in [with_path, pathless] {
            let changes = changes(&body);
            assert_eq!(changes.len(), 1);
            assert_eq!(changes[0].0, "active");
            assert_eq!(as_bool(&changes[0].1), Some(false));
        }
    }

    #[test]
    fn a_string_boolean_is_still_a_boolean() {
        assert_eq!(as_bool(&serde_json::json!("False")), None);
        assert_eq!(as_bool(&serde_json::json!("false")), Some(false));
        assert_eq!(as_bool(&serde_json::json!(true)), Some(true));
    }
}
