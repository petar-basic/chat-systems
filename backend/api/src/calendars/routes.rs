use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::AppResult;

use super::models::{CalendarList, CalendarState, UpdateCalendarSettings};
use super::service;
use crate::authz;
use crate::middleware::AuthUser;
use crate::state::AppState;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(list_calendars))
        .routes(routes!(update_calendar))
}

/// The person's Google and Outlook calendars in this workspace: whether an admin set each up,
/// the account they connected and what it does for them.
#[utoipa::path(get, path = "/workspaces/{ws_id}/calendars", tag = "apps", responses((status = 200, body = CalendarList)))]
async fn list_calendars(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<CalendarList>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    let data = service::list(&state, ws_id, auth.user_id).await?;
    Ok(Json(CalendarList { data }))
}

/// Turns the status, the reminders or the agenda of a connected calendar on or off, or moves
/// the agenda to another time of day. Turning the status off clears one the calendar set.
#[utoipa::path(patch, path = "/workspaces/{ws_id}/calendars/{provider}", tag = "apps", request_body = UpdateCalendarSettings, responses((status = 200, body = CalendarState)))]
async fn update_calendar(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((ws_id, provider)): Path<(Uuid, String)>,
    Json(req): Json<UpdateCalendarSettings>,
) -> AppResult<Json<CalendarState>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    Ok(Json(
        service::update(&state, ws_id, auth.user_id, &provider, &req).await?,
    ))
}
