use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::AppResult;

use super::models::{
    CustomResponse, CustomResponses, NewCustomResponse, RenameResponsesBot, UpdateCustomResponse,
};
use super::service;
use crate::audit::ClientIp;
use crate::dto::StatusResponse;
use crate::middleware::AuthUser;
use crate::state::AppState;
use crate::workspace_bots::models::WorkspaceBot;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(list_custom_responses, create_custom_response))
        .routes(routes!(update_custom_response, delete_custom_response))
        .routes(routes!(rename_responses_bot))
}

#[utoipa::path(get, path = "/workspaces/{ws_id}/custom-responses", tag = "workspaces", responses((status = 200, body = CustomResponses)))]
async fn list_custom_responses(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<CustomResponses>> {
    Ok(Json(service::list(&state, ws_id, auth.user_id).await?))
}

#[utoipa::path(post, path = "/workspaces/{ws_id}/custom-responses", tag = "workspaces", request_body = NewCustomResponse, responses((status = 200, body = CustomResponse)))]
async fn create_custom_response(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<NewCustomResponse>,
) -> AppResult<Json<CustomResponse>> {
    Ok(Json(
        service::create(&state, ws_id, auth.user_id, &req, &ip).await?,
    ))
}

#[utoipa::path(patch, path = "/workspaces/{ws_id}/custom-responses/{response_id}", tag = "workspaces", request_body = UpdateCustomResponse, responses((status = 200, body = CustomResponse)))]
async fn update_custom_response(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path((ws_id, response_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<UpdateCustomResponse>,
) -> AppResult<Json<CustomResponse>> {
    Ok(Json(
        service::update(&state, ws_id, auth.user_id, response_id, &req, &ip).await?,
    ))
}

#[utoipa::path(delete, path = "/workspaces/{ws_id}/custom-responses/{response_id}", tag = "workspaces", responses((status = 200, body = StatusResponse)))]
async fn delete_custom_response(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path((ws_id, response_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<StatusResponse>> {
    service::delete(&state, ws_id, auth.user_id, response_id, &ip).await?;
    Ok(Json(StatusResponse::new("deleted")))
}

#[utoipa::path(put, path = "/workspaces/{ws_id}/custom-responses/bot", tag = "workspaces", request_body = RenameResponsesBot, responses((status = 200, body = WorkspaceBot)))]
async fn rename_responses_bot(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<RenameResponsesBot>,
) -> AppResult<Json<WorkspaceBot>> {
    Ok(Json(
        service::rename_bot(&state, ws_id, auth.user_id, &req.name, &ip).await?,
    ))
}
