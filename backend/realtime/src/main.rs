mod config;
mod connection_manager;
mod event_consumer;
mod ws_handler;

#[cfg(test)]
mod tests;

use std::net::SocketAddr;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::WebSocketUpgrade;
use axum::extract::State;
use axum::http::header::{COOKIE, SEC_WEBSOCKET_PROTOCOL};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use jsonwebtoken::{decode, DecodingKey, Validation};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use shared_common::errors::AppError;

use crate::config::RealtimeConfig;
use crate::connection_manager::ConnectionManager;

#[derive(Clone)]
struct AppState {
    cm: Arc<ConnectionManager>,
    jwt_secret: String,
    consumer_heartbeat: Arc<AtomicI64>,
    cors_origins: String,
}

const CONSUMER_STALE_SECS: i64 = 60;

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
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
        .max_connections(config.pg_pool_max)
        .connect(&config.database_url)
        .await?;
    info!("Connected to Postgres");

    let redis_client = redis::Client::open(config.redis_url.clone())?;
    let redis_conn = redis::aio::ConnectionManager::new(redis_client).await?;
    info!("Connected to Redis");

    let cm = Arc::new(ConnectionManager::new(db, redis_conn));

    let prometheus = PrometheusBuilder::new()
        .install_recorder()
        .map_err(|e| anyhow::anyhow!("failed to install Prometheus recorder: {e}"))?;

    let consumer_heartbeat = Arc::new(AtomicI64::new(now_unix()));

    {
        let cm = cm.clone();
        let hb = consumer_heartbeat.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(15));
            loop {
                tick.tick().await;
                metrics::gauge!("realtime_ws_connections").set(cm.connection_count() as f64);
                let age = now_unix() - hb.load(Ordering::Relaxed);
                metrics::gauge!("realtime_consumer_heartbeat_age_seconds").set(age as f64);
            }
        });
    }

    let consumer_cm = cm.clone();
    let redis_url = config.redis_url.clone();
    let heartbeat = consumer_heartbeat.clone();
    tokio::spawn(async move {
        let mut backoff = 1u64;
        loop {
            event_consumer::start_event_consumer(
                &redis_url,
                consumer_cm.clone(),
                heartbeat.clone(),
            )
            .await;
            warn!("event consumer exited, restarting in {}s", backoff);
            tokio::time::sleep(Duration::from_secs(backoff)).await;
            backoff = (backoff * 2).min(30);
        }
    });

    let state = AppState {
        cm,
        jwt_secret: config.jwt_secret.clone(),
        consumer_heartbeat,
        cors_origins: config.cors_origins.clone(),
    };

    let app = build_app(state)
        .merge(metrics_router(prometheus))
        .layer(shared_common::cors::cors_layer(&config.cors_origins));

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

fn metrics_router(handle: PrometheusHandle) -> Router {
    Router::new()
        .route("/metrics", get(render_metrics))
        .with_state(handle)
}

async fn render_metrics(State(handle): State<PrometheusHandle>) -> impl IntoResponse {
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4",
        )],
        handle.render(),
    )
}

fn protocol_token(headers: &axum::http::HeaderMap) -> Option<String> {
    let raw = headers.get(SEC_WEBSOCKET_PROTOCOL)?.to_str().ok()?;
    let parts: Vec<&str> = raw.split(',').map(str::trim).collect();
    let idx = parts.iter().position(|&p| p == "bearer")?;
    parts.get(idx + 1).map(std::string::ToString::to_string)
}

fn cookie_token(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get(COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| {
            s.split(';').find_map(|part| {
                part.trim()
                    .strip_prefix("access_token=")
                    .map(std::string::ToString::to_string)
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

fn origin_allowed(headers: &axum::http::HeaderMap, allowed: &str) -> bool {
    let Some(origin) = headers
        .get(axum::http::header::ORIGIN)
        .and_then(|v| v.to_str().ok())
    else {
        return false;
    };
    allowed
        .split(',')
        .map(str::trim)
        .any(|a| a == "*" || a == origin)
}

async fn ws_upgrade(
    State(state): State<AppState>,
    ws: Option<WebSocketUpgrade>,
    headers: axum::http::HeaderMap,
) -> Result<Response, AppError> {
    if !origin_allowed(&headers, &state.cors_origins) {
        return Err(AppError::Unauthorized("Origin not allowed".into()));
    }
    let (user_id, exp) = authenticate_ws(&headers, &state.jwt_secret)?;
    if state.cm.is_revoked(user_id).await {
        return Err(AppError::Unauthorized("Session revoked".into()));
    }
    let ws = ws.ok_or_else(|| AppError::BadRequest("Expected a WebSocket upgrade".into()))?;
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
    if let Err(e) = ping {
        warn!("readyz Redis check failed: {}", e);
        return (StatusCode::SERVICE_UNAVAILABLE, "redis unavailable").into_response();
    }

    let age = now_unix() - state.consumer_heartbeat.load(Ordering::Relaxed);
    if age > CONSUMER_STALE_SECS {
        warn!("readyz consumer stalled: last heartbeat {}s ago", age);
        return (StatusCode::SERVICE_UNAVAILABLE, "consumer stalled").into_response();
    }

    (StatusCode::OK, "ready").into_response()
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
