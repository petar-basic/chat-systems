use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::AppResult;

use super::models::{
    NewProfileField, OrderProfileFields, ProfileField, ProfileValues, SetProfileValues,
    UpdateProfileField,
};
use super::service;
use crate::audit::ClientIp;
use crate::authz;
use crate::bots::{models::Scope, Caller};
use crate::dto::{DataList, StatusResponse};
use crate::middleware::AuthUser;
use crate::state::AppState;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(list_profile_fields, create_profile_field))
        .routes(routes!(update_profile_field, delete_profile_field))
        .routes(routes!(order_profile_fields))
        .routes(routes!(set_profile_values))
}

#[utoipa::path(
    get, path = "/workspaces/{ws_id}/profile-fields", tags = ["workspaces", "bot"],
    security(("bearer" = []), ("bot" = ["users:read"])),
    responses((status = 200, body = DataList<ProfileField>)))]
async fn list_profile_fields(
    State(state): State<Arc<AppState>>,
    caller: Caller,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<DataList<ProfileField>>> {
    let auth = caller.scoped(Scope::UsersRead)?;
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    Ok(Json(service::list(&state, ws_id).await?.into()))
}

#[utoipa::path(post, path = "/workspaces/{ws_id}/profile-fields", tag = "workspaces", request_body = NewProfileField, responses((status = 200, body = ProfileField)))]
async fn create_profile_field(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<NewProfileField>,
) -> AppResult<Json<ProfileField>> {
    Ok(Json(
        service::create(&state, ws_id, auth.user_id, &req, &ip).await?,
    ))
}

#[utoipa::path(patch, path = "/workspaces/{ws_id}/profile-fields/{field_id}", tag = "workspaces", request_body = UpdateProfileField, responses((status = 200, body = ProfileField)))]
async fn update_profile_field(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path((ws_id, field_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<UpdateProfileField>,
) -> AppResult<Json<ProfileField>> {
    Ok(Json(
        service::update(&state, ws_id, auth.user_id, field_id, &req, &ip).await?,
    ))
}

#[utoipa::path(delete, path = "/workspaces/{ws_id}/profile-fields/{field_id}", tag = "workspaces", responses((status = 200, body = StatusResponse)))]
async fn delete_profile_field(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path((ws_id, field_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<StatusResponse>> {
    service::delete(&state, ws_id, auth.user_id, field_id, &ip).await?;
    Ok(Json(StatusResponse::new("deleted")))
}

#[utoipa::path(put, path = "/workspaces/{ws_id}/profile-fields/order", tag = "workspaces", request_body = OrderProfileFields, responses((status = 200, body = DataList<ProfileField>)))]
async fn order_profile_fields(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<OrderProfileFields>,
) -> AppResult<Json<DataList<ProfileField>>> {
    Ok(Json(
        service::reorder(&state, ws_id, auth.user_id, &req.field_ids)
            .await?
            .into(),
    ))
}

#[utoipa::path(put, path = "/workspaces/{ws_id}/members/{user_id}/profile-fields", tag = "workspaces", request_body = SetProfileValues, responses((status = 200, body = ProfileValues)))]
async fn set_profile_values(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((ws_id, user_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<SetProfileValues>,
) -> AppResult<Json<ProfileValues>> {
    Ok(Json(
        service::set_values(&state, ws_id, auth.user_id, user_id, &req).await?,
    ))
}
