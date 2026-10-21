use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::{is_unique_violation, AppError, AppResult};

use super::models::*;
use crate::authz;
use crate::dto::StatusResponse;
use crate::middleware::AuthUser;
use crate::state::AppState;

const MAX_SECTIONS: i64 = 50;
const NAME_MAX: usize = 80;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(get_layout))
        .routes(routes!(create_section))
        .routes(routes!(update_section, delete_section))
        .routes(routes!(reorder_sections))
        .routes(routes!(collapse_builtin))
        .routes(routes!(place_channel))
}

fn section_name(raw: &str) -> AppResult<String> {
    let name = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() || name.chars().count() > NAME_MAX {
        return Err(AppError::Validation(format!(
            "A section name is 1 to {NAME_MAX} characters"
        )));
    }
    Ok(name)
}

fn duplicate_name(err: sqlx::Error) -> AppError {
    if is_unique_violation(&err) {
        AppError::Conflict("You already have a section with that name".into())
    } else {
        err.into()
    }
}

#[utoipa::path(get, path = "/workspaces/{ws_id}/sidebar", tag = "sidebar", responses((status = 200, body = SidebarLayout)))]
async fn get_layout(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<SidebarLayout>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    let repo = &state.sidebar_repo;
    let sections = repo.sections(auth.user_id, ws_id).await?;
    let placements = repo.placements(auth.user_id, ws_id).await?;
    let collapsed_builtins = repo
        .collapsed_builtins(auth.user_id, ws_id)
        .await?
        .iter()
        .filter_map(|section| BuiltinSection::parse(section))
        .collect();
    Ok(Json(SidebarLayout {
        sections,
        placements,
        collapsed_builtins,
    }))
}

#[utoipa::path(post, path = "/workspaces/{ws_id}/sidebar/sections", tag = "sidebar", request_body = CreateSectionRequest, responses((status = 200, body = SidebarSection)))]
async fn create_section(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<CreateSectionRequest>,
) -> AppResult<Json<SidebarSection>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    let name = section_name(&req.name)?;
    if state.sidebar_repo.count(auth.user_id, ws_id).await? >= MAX_SECTIONS {
        return Err(AppError::Validation(format!(
            "A sidebar holds at most {MAX_SECTIONS} sections"
        )));
    }
    let section = state
        .sidebar_repo
        .create(auth.user_id, ws_id, &name)
        .await
        .map_err(duplicate_name)?;
    Ok(Json(section))
}

#[utoipa::path(patch, path = "/workspaces/{ws_id}/sidebar/sections/{section_id}", tag = "sidebar", request_body = UpdateSectionRequest, responses((status = 200, body = SidebarSection)))]
async fn update_section(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((ws_id, section_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<UpdateSectionRequest>,
) -> AppResult<Json<SidebarSection>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    let name = req.name.as_deref().map(section_name).transpose()?;
    let section = state
        .sidebar_repo
        .update(
            section_id,
            auth.user_id,
            ws_id,
            name.as_deref(),
            req.collapsed,
        )
        .await
        .map_err(duplicate_name)?
        .ok_or_else(|| AppError::NotFound("Section not found".into()))?;
    Ok(Json(section))
}

#[utoipa::path(delete, path = "/workspaces/{ws_id}/sidebar/sections/{section_id}", tag = "sidebar", responses((status = 200, body = StatusResponse)))]
async fn delete_section(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((ws_id, section_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<StatusResponse>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    if !state
        .sidebar_repo
        .delete(section_id, auth.user_id, ws_id)
        .await?
    {
        return Err(AppError::NotFound("Section not found".into()));
    }
    Ok(Json(StatusResponse { status: "deleted" }))
}

#[utoipa::path(put, path = "/workspaces/{ws_id}/sidebar/order", tag = "sidebar", request_body = ReorderSectionsRequest, responses((status = 200, body = StatusResponse)))]
async fn reorder_sections(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<ReorderSectionsRequest>,
) -> AppResult<Json<StatusResponse>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    if req.section_ids.len() as i64 > MAX_SECTIONS {
        return Err(AppError::Validation("Too many sections".into()));
    }
    state
        .sidebar_repo
        .reorder(auth.user_id, ws_id, &req.section_ids)
        .await?;
    Ok(Json(StatusResponse {
        status: "reordered",
    }))
}

#[utoipa::path(put, path = "/workspaces/{ws_id}/sidebar/builtins", tag = "sidebar", request_body = CollapseBuiltinRequest, responses((status = 200, body = StatusResponse)))]
async fn collapse_builtin(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<CollapseBuiltinRequest>,
) -> AppResult<Json<StatusResponse>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    state
        .sidebar_repo
        .set_builtin_collapsed(auth.user_id, ws_id, req.section.as_str(), req.collapsed)
        .await?;
    Ok(Json(StatusResponse { status: "saved" }))
}

#[utoipa::path(put, path = "/channels/{ch_id}/section", tag = "sidebar", request_body = PlaceChannelRequest, responses((status = 200, body = StatusResponse)))]
async fn place_channel(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ch_id): Path<Uuid>,
    Json(req): Json<PlaceChannelRequest>,
) -> AppResult<Json<StatusResponse>> {
    authz::require_channel_access(&state, ch_id, auth.user_id).await?;
    if !state
        .sidebar_repo
        .place_channel(ch_id, auth.user_id, req.section_id)
        .await?
    {
        return Err(AppError::NotFound(
            "Not a member of that channel, or no such section".into(),
        ));
    }
    Ok(Json(StatusResponse { status: "moved" }))
}
