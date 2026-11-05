use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::Json;
use chrono::{Duration, NaiveDate, Utc};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::{AnalyticsQuery, MemberActivity, WorkspaceAnalytics, WINDOWS};
use crate::authz;
use crate::dto::DataList;
use crate::middleware::AuthUser;
use crate::state::AppState;
use crate::workspace::models::WorkspaceRole;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(workspace_analytics))
        .routes(routes!(member_analytics))
}

fn window(days: i32) -> AppResult<(NaiveDate, NaiveDate)> {
    if !WINDOWS.contains(&days) {
        return Err(AppError::BadRequest(
            "Analytics cover the last 7, 30 or 90 days".into(),
        ));
    }
    let to = Utc::now().date_naive();
    Ok((to - Duration::days(i64::from(days) - 1), to))
}

#[utoipa::path(get, path = "/workspaces/{ws_id}/analytics", tag = "analytics", params(AnalyticsQuery), responses((status = 200, body = WorkspaceAnalytics)))]
async fn workspace_analytics(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Query(query): Query<AnalyticsQuery>,
) -> AppResult<Json<WorkspaceAnalytics>> {
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
    let (from, to) = window(query.days)?;
    let repo = &state.analytics_repo;
    let daily = repo.daily(ws_id, from, to, query.guests).await?;
    let weekly = repo.weekly(ws_id, from, to, query.guests).await?;
    let active_members = repo.active_between(ws_id, from, to, query.guests).await?;
    let members = daily.last().map(|d| d.members).unwrap_or(0);
    Ok(Json(WorkspaceAnalytics {
        from,
        to,
        members,
        active_members,
        daily,
        weekly,
    }))
}

#[utoipa::path(get, path = "/workspaces/{ws_id}/analytics/members", tag = "analytics", params(AnalyticsQuery), responses((status = 200, body = DataList<MemberActivity>)))]
async fn member_analytics(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Query(query): Query<AnalyticsQuery>,
) -> AppResult<Json<DataList<MemberActivity>>> {
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
    let (from, to) = window(query.days)?;
    let members = state
        .analytics_repo
        .members(ws_id, from, to, query.guests)
        .await?;
    Ok(Json(members.into()))
}
