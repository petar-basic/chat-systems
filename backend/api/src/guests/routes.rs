use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::AppResult;

use super::service;
use crate::audit::ClientIp;
use crate::middleware::AuthUser;
use crate::state::AppState;
use crate::workspace::models::{GuestSeat, UpdateGuestRequest};

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new().routes(routes!(update_guest))
}

#[utoipa::path(patch, path = "/workspaces/{ws_id}/members/{user_id}/guest", tag = "workspaces", request_body = UpdateGuestRequest, responses((status = 200, body = GuestSeat)))]
async fn update_guest(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path((ws_id, user_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<UpdateGuestRequest>,
) -> AppResult<Json<GuestSeat>> {
    Ok(Json(
        service::set_seat(&state, ws_id, auth.user_id, user_id, &req, &ip).await?,
    ))
}
