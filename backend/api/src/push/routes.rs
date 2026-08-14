use std::sync::Arc;

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;

use shared_common::errors::{AppError, AppResult};

use crate::middleware::AuthUser;
use crate::state::AppState;

pub fn router(state: Arc<AppState>) -> Router {
    let routes = Router::new()
        .route("/push/key", get(public_key))
        .route("/push/subscriptions", post(subscribe).delete(unsubscribe))
        .route("/push/subscriptions/list", get(list));

    crate::protected(state, routes)
}

/// The public half of the VAPID pair, which the browser needs before it can ask
/// its push service for a subscription. It is public by definition; the reason
/// it sits behind the session is that an instance with push switched off should
/// not have to answer questions about it.
async fn public_key(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
) -> AppResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "public_key": state.config.vapid_public_key,
        "enabled": state.push_sender.is_configured(),
    })))
}

#[derive(Debug, Deserialize)]
pub struct SubscribeRequest {
    pub endpoint: String,
    pub keys: SubscriptionKeys,
    pub user_agent: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SubscriptionKeys {
    pub p256dh: String,
    pub auth: String,
}

async fn subscribe(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(req): Json<SubscribeRequest>,
) -> AppResult<Json<serde_json::Value>> {
    if req.endpoint.trim().is_empty() || req.keys.p256dh.is_empty() || req.keys.auth.is_empty() {
        return Err(AppError::Validation(
            "A subscription needs an endpoint and both keys".into(),
        ));
    }
    if !req.endpoint.starts_with("https://") {
        return Err(AppError::Validation("Push endpoints are https".to_string()));
    }

    let subscription = state
        .push_repo
        .upsert(
            auth.user_id,
            req.endpoint.trim(),
            &req.keys.p256dh,
            &req.keys.auth,
            req.user_agent.as_deref(),
        )
        .await?;

    Ok(Json(serde_json::json!({ "id": subscription.id })))
}

#[derive(Debug, Deserialize)]
pub struct UnsubscribeRequest {
    pub endpoint: String,
}

async fn unsubscribe(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(req): Json<UnsubscribeRequest>,
) -> AppResult<Json<serde_json::Value>> {
    state.push_repo.delete(auth.user_id, &req.endpoint).await?;
    Ok(Json(serde_json::json!({ "status": "unsubscribed" })))
}

async fn list(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> AppResult<Json<serde_json::Value>> {
    let subscriptions = state.push_repo.list_for_user(auth.user_id).await?;
    Ok(Json(serde_json::json!({
        "data": subscriptions
            .iter()
            .map(|s| serde_json::json!({
                "id": s.id,
                "user_agent": s.user_agent,
                "created_at": s.created_at,
                "last_used_at": s.last_used_at,
            }))
            .collect::<Vec<_>>(),
    })))
}
