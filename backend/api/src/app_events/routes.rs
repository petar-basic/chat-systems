use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::{generate_app_token, generate_secret, sender, APP_TOKEN_PREFIX};
use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::authz;
use crate::bots::hash_token;
use crate::bots::models::{AppToken, BotApp, SigningSecret, SocketUrl};
use crate::dto::StatusResponse;
use crate::middleware::AuthUser;
use crate::state::AppState;
use crate::workspace::models::WorkspaceRole;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(make_app_token, revoke_app_token))
        .routes(routes!(reveal_signing_secret))
}

pub fn public_router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new().routes(routes!(open_connection))
}

async fn find_app_as_admin(state: &AppState, app_id: Uuid, user_id: Uuid) -> AppResult<BotApp> {
    let app = state
        .bot_repo
        .find(app_id)
        .await?
        .ok_or_else(|| AppError::NotFound("App not found".into()))?;
    authz::require_workspace_role(state, app.workspace_id, user_id, &WorkspaceRole::Admin).await?;
    Ok(app)
}

#[utoipa::path(post, path = "/bot-apps/{app_id}/app-token", tag = "bot-apps",
    responses((status = 200, body = AppToken)))]
async fn make_app_token(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(app_id): Path<Uuid>,
) -> AppResult<Json<AppToken>> {
    let app = find_app_as_admin(&state, app_id, auth.user_id).await?;
    let app_token = generate_app_token();
    if !state
        .bot_repo
        .rotate_app_token(app_id, Some(&hash_token(&app_token)))
        .await?
    {
        return Err(AppError::NotFound("App not found".into()));
    }
    audit::record(
        &state,
        AuditEntry::new(AuditAction::BotAppTokenRotated, auth.user_id)
            .workspace(app.workspace_id)
            .resource(app.id)
            .ip(&ip)
            .details(serde_json::json!({ "token": "app" })),
    )
    .await;
    Ok(Json(AppToken { app_id, app_token }))
}

#[utoipa::path(delete, path = "/bot-apps/{app_id}/app-token", tag = "bot-apps",
    responses((status = 200, body = StatusResponse)))]
async fn revoke_app_token(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(app_id): Path<Uuid>,
) -> AppResult<Json<StatusResponse>> {
    let app = find_app_as_admin(&state, app_id, auth.user_id).await?;
    state.bot_repo.rotate_app_token(app_id, None).await?;
    audit::record(
        &state,
        AuditEntry::new(AuditAction::BotAppTokenRotated, auth.user_id)
            .workspace(app.workspace_id)
            .resource(app.id)
            .ip(&ip)
            .details(serde_json::json!({ "token": "app", "revoked": true })),
    )
    .await;
    Ok(Json(StatusResponse::new("revoked")))
}

#[utoipa::path(get, path = "/bot-apps/{app_id}/signing-secret", tag = "bot-apps",
    responses((status = 200, body = SigningSecret)))]
async fn reveal_signing_secret(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(app_id): Path<Uuid>,
) -> AppResult<Json<SigningSecret>> {
    find_app_as_admin(&state, app_id, auth.user_id).await?;
    Ok(Json(SigningSecret {
        app_id,
        signing_secret: sender::signing_secret(&state, app_id).await?,
    }))
}

fn socket_base(public_url: &str) -> String {
    let base = public_url.trim_end_matches('/');
    if let Some(rest) = base.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = base.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        base.to_string()
    }
}

#[utoipa::path(post, path = "/apps/connections.open", tags = ["bot-apps"],
    security(("app" = [])),
    responses((status = 200, body = SocketUrl)))]
async fn open_connection(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> AppResult<Json<SocketUrl>> {
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|t| t.starts_with(APP_TOKEN_PREFIX))
        .ok_or_else(|| AppError::Unauthorized("An app-level token is required".into()))?;
    let (app_id, socket_mode) = state
        .bot_repo
        .by_app_token(&hash_token(token))
        .await?
        .ok_or_else(|| AppError::Unauthorized("Invalid app-level token".into()))?;
    if !socket_mode {
        return Err(AppError::Forbidden(
            "Socket mode is off for this app".into(),
        ));
    }
    let ticket = generate_secret();
    state
        .bot_repo
        .open_socket(app_id, &hash_token(&ticket))
        .await?;
    Ok(Json(SocketUrl {
        url: format!(
            "{}/ws/apps?ticket={ticket}",
            socket_base(&state.config.public_url)
        ),
    }))
}

#[cfg(test)]
mod tests {
    use super::socket_base;

    #[test]
    fn the_socket_url_follows_the_public_scheme() {
        assert_eq!(
            socket_base("https://chat.example.com/"),
            "wss://chat.example.com"
        );
        assert_eq!(socket_base("http://localhost:8080"), "ws://localhost:8080");
    }
}
