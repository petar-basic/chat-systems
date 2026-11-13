use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::AppResult;

use super::models::{
    ApproveInviteRequest, ApprovedInviteRequest, AskForInvites, DeclineInviteRequest,
    WorkspaceInviteRequest,
};
use super::service;
use crate::audit::ClientIp;
use crate::dto::DataList;
use crate::middleware::AuthUser;
use crate::net::AcceptLanguage;
use crate::state::AppState;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(list_invite_requests, ask_for_invites))
        .routes(routes!(approve_invite_request))
        .routes(routes!(decline_invite_request))
}

#[utoipa::path(get, path = "/workspaces/{ws_id}/invite-requests", tag = "workspaces", responses((status = 200, body = DataList<WorkspaceInviteRequest>)))]
async fn list_invite_requests(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<DataList<WorkspaceInviteRequest>>> {
    Ok(Json(
        service::list(&state, ws_id, auth.user_id).await?.into(),
    ))
}

#[utoipa::path(post, path = "/workspaces/{ws_id}/invite-requests", tag = "workspaces", request_body = AskForInvites, responses((status = 200, body = WorkspaceInviteRequest)))]
async fn ask_for_invites(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<AskForInvites>,
) -> AppResult<Json<WorkspaceInviteRequest>> {
    Ok(Json(
        service::ask(&state, ws_id, auth.user_id, &req, &ip).await?,
    ))
}

#[utoipa::path(post, path = "/workspaces/{ws_id}/invite-requests/{request_id}/approve", tag = "workspaces", request_body = ApproveInviteRequest, responses((status = 200, body = ApprovedInviteRequest)))]
async fn approve_invite_request(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    accept_language: AcceptLanguage,
    Path((ws_id, request_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<ApproveInviteRequest>,
) -> AppResult<Json<ApprovedInviteRequest>> {
    let language = state
        .auth_service
        .language_for(auth.user_id, accept_language.header())
        .await?;
    Ok(Json(
        service::approve(&state, ws_id, auth.user_id, request_id, &req, language, &ip).await?,
    ))
}

#[utoipa::path(post, path = "/workspaces/{ws_id}/invite-requests/{request_id}/decline", tag = "workspaces", request_body = DeclineInviteRequest, responses((status = 200, body = WorkspaceInviteRequest)))]
async fn decline_invite_request(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path((ws_id, request_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<DeclineInviteRequest>,
) -> AppResult<Json<WorkspaceInviteRequest>> {
    Ok(Json(
        service::decline(&state, ws_id, auth.user_id, request_id, &req, &ip).await?,
    ))
}
