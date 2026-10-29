use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use garde::Validate;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::AppResult;

use super::models::{AppDirectory, AppRequest};
use super::service;
use crate::dto::StatusResponse;
use crate::middleware::AuthUser;
use crate::state::AppState;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(app_directory))
        .routes(routes!(request_app))
}

#[utoipa::path(get, path = "/workspaces/{ws_id}/app-directory", tag = "app-directory",
    responses((status = 200, body = AppDirectory)))]
async fn app_directory(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<AppDirectory>> {
    Ok(Json(service::directory(&state, ws_id, auth.user_id).await?))
}

#[utoipa::path(post, path = "/workspaces/{ws_id}/app-requests", tag = "app-directory",
    request_body = AppRequest,
    responses((status = 200, body = StatusResponse)))]
async fn request_app(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<AppRequest>,
) -> AppResult<Json<StatusResponse>> {
    req.validate()?;
    service::request(&state, ws_id, auth.user_id, &req).await?;
    Ok(Json(StatusResponse::new("requested")))
}
