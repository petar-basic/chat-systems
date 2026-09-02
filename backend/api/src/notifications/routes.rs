use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use uuid::Uuid;

use shared_common::errors::AppResult;

use super::models::*;
use crate::authz;
use crate::middleware::AuthUser;
use crate::pagination::PageQuery;
use crate::state::AppState;

pub fn router(state: Arc<AppState>) -> Router {
    let routes = Router::new()
        .route("/workspaces/{ws_id}/notifications", get(list_notifications))
        .route("/notifications/read", post(mark_read))
        .route(
            "/workspaces/{ws_id}/notifications/read-all",
            post(mark_all_read),
        )
        .route(
            "/workspaces/{ws_id}/channels/{ch_id}/notifications/read",
            post(mark_channel_read),
        )
        .route(
            "/workspaces/{ws_id}/notifications/unread-count",
            get(unread_count),
        )
        .route("/notifications/dnd", get(get_dnd).patch(set_dnd))
        .route(
            "/notifications/email",
            get(get_email_preference).patch(set_email_preference),
        );

    crate::protected(state, routes)
}

async fn list_notifications(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Query(params): Query<PageQuery>,
) -> AppResult<Json<serde_json::Value>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    let notifications = state
        .notification_repo
        .list_for_user(auth.user_id, ws_id, params.limit(), params.offset())
        .await?;
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
        .await?;
    Ok(Json(serde_json::json!({ "updated": count })))
}

async fn mark_all_read(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    let count = state
        .notification_repo
        .mark_all_read(auth.user_id, ws_id)
        .await?;
    Ok(Json(serde_json::json!({ "updated": count })))
}

async fn mark_channel_read(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((ws_id, ch_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    let count = state
        .notification_repo
        .mark_channel_read(auth.user_id, ws_id, ch_id)
        .await?;
    Ok(Json(serde_json::json!({ "updated": count })))
}

async fn unread_count(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    let count = state
        .notification_repo
        .unread_count(auth.user_id, ws_id)
        .await?;
    Ok(Json(serde_json::json!({ "unread_count": count })))
}

async fn get_dnd(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> AppResult<Json<serde_json::Value>> {
    let dnd_until = state.notification_repo.get_dnd(auth.user_id).await?;
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
        .await?;
    Ok(Json(serde_json::json!({ "dnd_until": req.dnd_until })))
}

/// Whether to email a mention that reached nobody. On by default: somebody who
/// never grants push permission is exactly who it is for, and they are not going
/// to go looking for a switch to turn it on.
async fn get_email_preference(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> AppResult<Json<serde_json::Value>> {
    let enabled: Option<bool> =
        sqlx::query_scalar("SELECT mention_emails FROM users WHERE id = $1")
            .bind(auth.user_id)
            .fetch_optional(&state.pool)
            .await?;

    Ok(Json(serde_json::json!({
        "mention_emails": enabled.unwrap_or(true),
        "available": state.auth_service.can_send_email(),
    })))
}

#[derive(serde::Deserialize)]
pub struct EmailPreferenceRequest {
    pub mention_emails: bool,
}

async fn set_email_preference(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(req): Json<EmailPreferenceRequest>,
) -> AppResult<Json<serde_json::Value>> {
    sqlx::query("UPDATE users SET mention_emails = $2 WHERE id = $1")
        .bind(auth.user_id)
        .bind(req.mention_emails)
        .execute(&state.pool)
        .await?;

    Ok(Json(
        serde_json::json!({ "mention_emails": req.mention_emails }),
    ))
}
