use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::Deserialize;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use shared_common::errors::{AppError, AppResult};

use crate::dto::{DataList, StatusResponse};
use crate::middleware::AuthUser;
use crate::state::AppState;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(public_key))
        .routes(routes!(subscribe, unsubscribe))
        .routes(routes!(list))
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct PushKey {
    pub public_key: String,
    pub enabled: bool,
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct SubscriptionCreated {
    pub id: uuid::Uuid,
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct PushSubscriptionView {
    pub id: uuid::Uuid,
    pub user_agent: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// The public half of the VAPID pair, which the browser needs before it can ask
/// its push service for a subscription. It is public by definition; the reason
/// it sits behind the session is that an instance with push switched off should
/// not have to answer questions about it.
#[utoipa::path(get, path = "/push/key", tag = "push", responses((status = 200, body = PushKey)))]
async fn public_key(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
) -> AppResult<Json<PushKey>> {
    Ok(Json(PushKey {
        public_key: state.config.vapid_public_key.clone(),
        enabled: state.push_sender.is_configured(),
    }))
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct SubscribeRequest {
    pub endpoint: String,
    pub keys: SubscriptionKeys,
    pub user_agent: Option<String>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct SubscriptionKeys {
    pub p256dh: String,
    pub auth: String,
}

#[utoipa::path(post, path = "/push/subscriptions", tag = "push", request_body = SubscribeRequest, responses((status = 200, body = SubscriptionCreated)))]
async fn subscribe(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(req): Json<SubscribeRequest>,
) -> AppResult<Json<SubscriptionCreated>> {
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

    Ok(Json(SubscriptionCreated {
        id: subscription.id,
    }))
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct UnsubscribeRequest {
    pub endpoint: String,
}

#[utoipa::path(delete, path = "/push/subscriptions", tag = "push", request_body = UnsubscribeRequest, responses((status = 200, body = StatusResponse)))]
async fn unsubscribe(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(req): Json<UnsubscribeRequest>,
) -> AppResult<Json<StatusResponse>> {
    state.push_repo.delete(auth.user_id, &req.endpoint).await?;
    Ok(Json(StatusResponse::new("unsubscribed")))
}

#[utoipa::path(get, path = "/push/subscriptions/list", tag = "push", responses((status = 200, body = DataList<PushSubscriptionView>)))]
async fn list(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> AppResult<Json<DataList<PushSubscriptionView>>> {
    let subscriptions = state.push_repo.list_for_user(auth.user_id).await?;
    let data = subscriptions
        .into_iter()
        .map(|s| PushSubscriptionView {
            id: s.id,
            user_agent: s.user_agent,
            created_at: s.created_at,
            last_used_at: s.last_used_at,
        })
        .collect();
    Ok(Json(DataList { data }))
}
