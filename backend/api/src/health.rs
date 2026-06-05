use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;

use crate::state::AppState;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/livez", get(livez))
        .route("/readyz", get(readyz))
        .with_state(state)
}

async fn livez() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

async fn readyz(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    if let Err(e) = sqlx::query("SELECT 1").execute(&state.pool).await {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            format!("database unavailable: {}", e),
        );
    }

    let mut redis = state.redis.clone();
    let ping: redis::RedisResult<()> = redis::cmd("PING").query_async(&mut redis).await;
    if let Err(e) = ping {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            format!("redis unavailable: {}", e),
        );
    }

    (StatusCode::OK, "ready".to_string())
}
