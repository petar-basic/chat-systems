use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use garde::Validate;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::{RecentSearch, RecordSearchRequest};
use crate::authz;
use crate::dto::{DataList, StatusResponse};
use crate::middleware::AuthUser;
use crate::state::AppState;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(
            list_recent_searches,
            record_search,
            clear_recent_searches
        ))
        .routes(routes!(forget_search))
}

#[utoipa::path(get, path = "/workspaces/{ws_id}/searches/recent", tag = "messages", responses((status = 200, body = DataList<RecentSearch>)))]
async fn list_recent_searches(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<DataList<RecentSearch>>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    let searches = state.search_repo.recent(auth.user_id, ws_id).await?;
    Ok(Json(DataList { data: searches }))
}

#[utoipa::path(post, path = "/workspaces/{ws_id}/searches/recent", tag = "messages", request_body = RecordSearchRequest, responses((status = 200, body = RecentSearch)))]
async fn record_search(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<RecordSearchRequest>,
) -> AppResult<Json<RecentSearch>> {
    req.validate()?;
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    let query = req.query.split_whitespace().collect::<Vec<_>>().join(" ");
    let filters = req.filters.for_tab(req.tab);
    if query.is_empty() && filters.is_empty() {
        return Err(AppError::Validation(
            "A search needs words or at least one filter".into(),
        ));
    }
    if let Some(place) = filters.place {
        require_place(&state, ws_id, auth.user_id, place).await?;
    }
    let search = state
        .search_repo
        .record(auth.user_id, ws_id, &query, req.tab, &filters)
        .await?;
    Ok(Json(search))
}

#[utoipa::path(delete, path = "/workspaces/{ws_id}/searches/recent", tag = "messages", responses((status = 200, body = StatusResponse)))]
async fn clear_recent_searches(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<StatusResponse>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    state.search_repo.clear(auth.user_id, ws_id).await?;
    Ok(Json(StatusResponse::new("cleared")))
}

#[utoipa::path(delete, path = "/workspaces/{ws_id}/searches/recent/{id}", tag = "messages", responses((status = 200, body = StatusResponse)))]
async fn forget_search(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((ws_id, id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<StatusResponse>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    if !state.search_repo.forget(id, auth.user_id, ws_id).await? {
        return Err(AppError::NotFound("Not found".into()));
    }
    Ok(Json(StatusResponse::new("deleted")))
}

async fn require_place(
    state: &AppState,
    ws_id: Uuid,
    user_id: Uuid,
    channel_id: Uuid,
) -> AppResult<()> {
    let unknown = || AppError::NotFound("Not found".into());
    let access = authz::require_channel_access(state, channel_id, user_id)
        .await
        .map_err(|e| match e {
            AppError::Forbidden(_) | AppError::NotFound(_) => unknown(),
            other => other,
        })?;
    if access.channel.workspace_id != ws_id || access.channel_member.is_none() {
        return Err(unknown());
    }
    Ok(())
}
