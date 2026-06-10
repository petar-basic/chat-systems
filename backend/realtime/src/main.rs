mod config;
mod connection_manager;
mod event_consumer;
mod ws_handler;

#[cfg(test)]
mod tests;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::ws::WebSocketUpgrade;
use axum::extract::State;
use axum::http::header::{COOKIE, SEC_WEBSOCKET_PROTOCOL};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use jsonwebtoken::{decode, DecodingKey, Validation};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use shared_common::errors::AppError;

use crate::config::RealtimeConfig;
use crate::connection_manager::ConnectionManager;

#[derive(Clone)]
struct AppState {
    cm: Arc<ConnectionManager>,
    jwt_secret: String,
}

fn default_token_type() -> String {
    "access".to_string()
}

#[derive(Debug, serde::Deserialize)]
struct Claims {
    sub: uuid::Uuid,
    exp: i64,
    #[serde(default = "default_token_type")]
    token_type: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = RealtimeConfig::from_env();
    let port = config.port;

    let db = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database_url)
        .await?;
    info!("Connected to Postgres");

    let redis_client = redis::Client::open(config.redis_url.clone())?;
    let redis_conn = redis::aio::ConnectionManager::new(redis_client).await?;
    info!("Connected to Redis");

    let cm = Arc::new(ConnectionManager::new(db, redis_conn));

    let consumer_cm = cm.clone();
    let redis_url = config.redis_url.clone();
    tokio::spawn(async move {
        event_consumer::start_event_consumer(&redis_url, consumer_cm).await;
    });

    let state = AppState {
        cm,
        jwt_secret: config.jwt_secret.clone(),
    };

    let app = build_app(state).layer(shared_common::cors::cors_layer(&config.cors_origins));

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("chat-realtime listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

pub(crate) fn build_app(state: AppState) -> Router {
    Router::new()
        .route("/ws", get(ws_upgrade))
        .route("/livez", get(livez))
        .route("/readyz", get(readyz))
        .with_state(state)
}

fn protocol_token(headers: &axum::http::HeaderMap) -> Option<String> {
    let raw = headers.get(SEC_WEBSOCKET_PROTOCOL)?.to_str().ok()?;
    let parts: Vec<&str> = raw.split(',').map(|p| p.trim()).collect();
    let idx = parts.iter().position(|&p| p == "bearer")?;
    parts.get(idx + 1).map(|s| s.to_string())
}

fn cookie_token(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get(COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| {
            s.split(';').find_map(|part| {
                part.trim()
                    .strip_prefix("access_token=")
                    .map(|v| v.to_string())
            })
        })
}

pub(crate) fn authenticate_ws(
    headers: &axum::http::HeaderMap,
    jwt_secret: &str,
) -> Result<(uuid::Uuid, i64), AppError> {
    let token = protocol_token(headers)
        .or_else(|| cookie_token(headers))
        .ok_or_else(|| AppError::Unauthorized("Missing access token".into()))?;

    let token_data = decode::<Claims>(
        &token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|_| AppError::Unauthorized("Invalid or expired token".into()))?;

    if token_data.claims.token_type != "access" {
        warn!(
            "Rejected WS upgrade: non-access token_type={}",
            token_data.claims.token_type
        );
        return Err(AppError::Unauthorized("Invalid token type".into()));
    }

    Ok((token_data.claims.sub, token_data.claims.exp))
}

async fn ws_upgrade(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
    headers: axum::http::HeaderMap,
) -> Result<Response, AppError> {
    let (user_id, exp) = authenticate_ws(&headers, &state.jwt_secret)?;
    let cm = state.cm.clone();
    Ok(ws
        .protocols(["bearer"])
        .on_upgrade(move |socket| ws_handler::handle_ws(socket, user_id, exp, cm)))
}

async fn livez() -> impl IntoResponse {
    StatusCode::OK
}

async fn readyz(State(state): State<AppState>) -> Response {
    if let Err(e) = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(state.cm.db())
        .await
    {
        warn!("readyz DB check failed: {}", e);
        return (StatusCode::SERVICE_UNAVAILABLE, "db unavailable").into_response();
    }

    let mut redis_conn = state.cm.redis();
    let ping: redis::RedisResult<String> = redis::cmd("PING").query_async(&mut redis_conn).await;
    match ping {
        Ok(_) => (StatusCode::OK, "ready").into_response(),
        Err(e) => {
            warn!("readyz Redis check failed: {}", e);
            (StatusCode::SERVICE_UNAVAILABLE, "redis unavailable").into_response()
        }
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl-C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("shutdown signal received, draining connections");
}
