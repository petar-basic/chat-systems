use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::*;
use super::service;
use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::authz;
use crate::dto::StatusResponse;
use crate::middleware::AuthUser;
use crate::state::AppState;
use crate::workspace::models::WorkspaceRole;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(list_apps))
        .routes(routes!(configure_app, remove_app))
        .routes(routes!(connect_app))
        .routes(routes!(disconnect_app))
        .routes(routes!(complete_connection))
}

fn provider(raw: &str) -> AppResult<Provider> {
    Provider::parse(raw).ok_or_else(|| AppError::NotFound("No such app".into()))
}

#[utoipa::path(get, path = "/workspaces/{ws_id}/apps", tag = "apps", responses((status = 200, body = AppList)))]
async fn list_apps(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<AppList>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    let data = service::statuses(&state, ws_id, auth.user_id).await?;
    Ok(Json(AppList { data }))
}

#[utoipa::path(put, path = "/workspaces/{ws_id}/apps/{provider}", tag = "apps", request_body = ConfigureAppRequest, responses((status = 200, body = StatusResponse)))]
async fn configure_app(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path((ws_id, raw)): Path<(Uuid, String)>,
    Json(req): Json<ConfigureAppRequest>,
) -> AppResult<Json<StatusResponse>> {
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
    let provider = provider(&raw)?;
    service::configure(&state, ws_id, auth.user_id, provider, &req).await?;
    audit::record(
        &state,
        AuditEntry::new(AuditAction::AppConfigured, auth.user_id)
            .workspace(ws_id)
            .ip(&ip)
            .details(serde_json::json!({ "provider": provider.as_str(), "enabled": req.enabled })),
    )
    .await;
    Ok(Json(StatusResponse { status: "saved" }))
}

#[utoipa::path(delete, path = "/workspaces/{ws_id}/apps/{provider}", tag = "apps", responses((status = 200, body = StatusResponse)))]
async fn remove_app(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path((ws_id, raw)): Path<(Uuid, String)>,
) -> AppResult<Json<StatusResponse>> {
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
    let provider = provider(&raw)?;
    if !state
        .integration_repo
        .delete_app(ws_id, provider.as_str())
        .await?
    {
        return Err(AppError::NotFound("That app is not set up".into()));
    }
    audit::record(
        &state,
        AuditEntry::new(AuditAction::AppRemoved, auth.user_id)
            .workspace(ws_id)
            .ip(&ip)
            .details(serde_json::json!({ "provider": provider.as_str() })),
    )
    .await;
    Ok(Json(StatusResponse { status: "removed" }))
}

#[utoipa::path(post, path = "/workspaces/{ws_id}/apps/{provider}/connect", tag = "apps", request_body = ConnectAppRequest, responses((status = 200, body = ConnectAppResponse)))]
async fn connect_app(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((ws_id, raw)): Path<(Uuid, String)>,
    Json(req): Json<ConnectAppRequest>,
) -> AppResult<Json<ConnectAppResponse>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    let provider = provider(&raw)?;
    let authorize_url =
        service::start_connection(&state, ws_id, auth.user_id, provider, req.return_to).await?;
    Ok(Json(ConnectAppResponse { authorize_url }))
}

#[utoipa::path(delete, path = "/workspaces/{ws_id}/apps/{provider}/connection", tag = "apps", responses((status = 200, body = StatusResponse)))]
async fn disconnect_app(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path((ws_id, raw)): Path<(Uuid, String)>,
) -> AppResult<Json<StatusResponse>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    let provider = provider(&raw)?;
    if !state
        .integration_repo
        .delete_connection(ws_id, auth.user_id, provider.as_str())
        .await?
    {
        return Err(AppError::NotFound("Not connected".into()));
    }
    audit::record(
        &state,
        AuditEntry::new(AuditAction::AppDisconnected, auth.user_id)
            .workspace(ws_id)
            .ip(&ip)
            .details(serde_json::json!({ "provider": provider.as_str() })),
    )
    .await;
    Ok(Json(StatusResponse {
        status: "disconnected",
    }))
}

#[utoipa::path(post, path = "/apps/connections", tag = "apps", request_body = CompleteConnectionRequest, responses((status = 200, body = CompleteConnectionResponse)))]
async fn complete_connection(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Json(req): Json<CompleteConnectionRequest>,
) -> AppResult<Json<CompleteConnectionResponse>> {
    let done = service::complete_connection(&state, auth.user_id, &req).await?;
    audit::record(
        &state,
        AuditEntry::new(AuditAction::AppConnected, auth.user_id)
            .workspace(done.workspace_id)
            .ip(&ip)
            .details(serde_json::json!({ "provider": done.provider.as_str() })),
    )
    .await;
    Ok(Json(done))
}
