use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{middleware, Json, Router};
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::*;
use crate::middleware::{auth_middleware, AuthUser};
use crate::state::AppState;

pub fn router(state: Arc<AppState>) -> Router {
    let routes = Router::new()
        .route("/workspaces/:ws_id/notifications", get(list_notifications))
        .route("/notifications/read", post(mark_read))
        .route(
            "/workspaces/:ws_id/notifications/read-all",
            post(mark_all_read),
        )
        .route(
            "/workspaces/:ws_id/channels/:ch_id/notifications/read",
            post(mark_channel_read),
        )
        .route(
            "/workspaces/:ws_id/notifications/unread-count",
            get(unread_count),
        )
        .route("/notifications/dnd", get(get_dnd).patch(set_dnd))
        .layer(middleware::from_fn(auth_middleware));

    Router::new().merge(routes).with_state(state)
}

async fn list_notifications(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> AppResult<Json<serde_json::Value>> {
    let limit = params
        .get("limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(50i64);
    let offset = params
        .get("offset")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0i64);
    let notifications = state
        .notification_repo
        .list_for_user(auth.user_id, ws_id, limit, offset)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(Json(serde_json::json!({ "data": notifications })))
}

async fn mark_read(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(req): Json<MarkReadRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let count = state
        .notification_repo
        .mark_read(&req.notification_ids, auth.user_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(Json(serde_json::json!({ "updated": count })))
}

async fn mark_all_read(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let count = state
        .notification_repo
        .mark_all_read(auth.user_id, ws_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(Json(serde_json::json!({ "updated": count })))
}

async fn mark_channel_read(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((ws_id, ch_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
    let count = state
        .notification_repo
        .mark_channel_read(auth.user_id, ws_id, ch_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(Json(serde_json::json!({ "updated": count })))
}

async fn unread_count(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let count = state
        .notification_repo
        .unread_count(auth.user_id, ws_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(Json(serde_json::json!({ "unread_count": count })))
}

async fn get_dnd(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> AppResult<Json<serde_json::Value>> {
    let dnd_until = state
        .notification_repo
        .get_dnd(auth.user_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(Json(serde_json::json!({ "dnd_until": dnd_until })))
}

async fn set_dnd(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(req): Json<SetDndRequest>,
) -> AppResult<Json<serde_json::Value>> {
    state
        .notification_repo
        .set_dnd(auth.user_id, req.dnd_until)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(Json(serde_json::json!({ "dnd_until": req.dnd_until })))
}
