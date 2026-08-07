pub mod admin;
pub mod auth;
pub mod authz;
pub mod config;
pub mod conversations;
pub mod files;
pub mod health;
pub mod hooks;
pub mod huddle;
pub mod messaging;
pub mod metrics;
pub mod middleware;
pub mod notifications;
pub mod presence;
pub mod rate_limit;
pub mod scheduled;
pub mod sessions;
pub mod state;
pub mod workspace;

#[cfg(test)]
mod http_tests;

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::DefaultBodyLimit;
use axum::Extension;
use axum::Router;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use tower_http::compression::CompressionLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use crate::auth::repo::UserRepo;
use crate::auth::service::AuthService;
use crate::config::AppConfig;
use crate::conversations::repo::ConversationRepo;
use crate::files::repo::FileRepo;
use crate::files::storage::create_storage;
use crate::hooks::repo::HookRepo;
use crate::huddle::repo::HuddleRepo;
use crate::messaging::publisher::EventPublisher;
use crate::messaging::repo::MessageRepo;
use crate::middleware::{JwtSecret, RevocationStore};
use crate::notifications::repo::NotificationRepo;
use crate::state::AppState;
use crate::workspace::repo::WorkspaceRepo;
use crate::workspace::service::WorkspaceService;

pub fn init_tracing() {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
}

pub async fn connect_pool(config: &AppConfig) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(config.pg_pool_max)
        .connect(&config.database_url)
        .await?;
    info!("Connected to database");
    Ok(pool)
}

pub async fn build_state(pool: PgPool, config: AppConfig) -> anyhow::Result<Arc<AppState>> {
    let redis_client = redis::Client::open(config.redis_url.as_str())?;
    let redis_conn = redis::aio::ConnectionManager::new(redis_client).await?;

    let file_storage = create_storage(&config).await?;

    let auth_service = AuthService::new(UserRepo::new(pool.clone()), config.clone());
    let workspace_service = WorkspaceService::new(WorkspaceRepo::new(pool.clone()), config.clone());
    let message_repo = MessageRepo::new(pool.clone());
    let publisher = EventPublisher::new(redis_conn.clone());
    let file_repo = FileRepo::new(pool.clone());
    let hook_repo = HookRepo::new(pool.clone());
    let notification_repo = NotificationRepo::new(pool.clone());
    let conversation_repo = ConversationRepo::new(pool.clone());
    let scheduled_repo = scheduled::repo::ScheduledRepo::new(pool.clone());
    let huddle_repo = HuddleRepo::new(pool.clone());

    Ok(Arc::new(AppState {
        config,
        pool,
        redis: redis_conn,
        auth_service,
        workspace_service,
        message_repo,
        publisher,
        file_repo,
        file_storage,
        hook_repo,
        notification_repo,
        conversation_repo,
        scheduled_repo,
        huddle_repo,
    }))
}

pub fn build_app(state: Arc<AppState>) -> Router {
    let jwt_secret = state.config.jwt_secret.clone();
    let cors_origins = state.config.cors_origins.clone();
    let revocation = RevocationStore(state.redis.clone());

    let api = Router::new()
        .merge(auth::routes::router(state.clone()))
        .merge(workspace::routes::router(state.clone()))
        .merge(messaging::routes::router(state.clone()))
        .merge(files::routes::router(state.clone()))
        .merge(hooks::routes::router(state.clone()))
        .merge(notifications::routes::router(state.clone()))
        .merge(admin::routes::router(state.clone()))
        .merge(conversations::routes::router(state.clone()))
        .merge(scheduled::routes::router(state.clone()))
        .merge(huddle::routes::router(state.clone()));

    Router::new()
        .nest("/api", api)
        .merge(health::router(state.clone()))
        .layer(axum::middleware::from_fn(metrics::track_metrics))
        .layer(Extension(revocation))
        .layer(Extension(JwtSecret(jwt_secret)))
        .layer(shared_common::cors::cors_layer(&cors_origins))
        .layer(CompressionLayer::new())
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .layer(TimeoutLayer::new(Duration::from_secs(30)))
        .layer(TraceLayer::new_for_http())
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
}

pub async fn supervise<F, Fut>(name: &str, consumer: F)
where
    F: Fn() -> Fut,
    Fut: Future<Output = ()>,
{
    const MAX_BACKOFF: Duration = Duration::from_secs(30);
    let mut backoff = Duration::from_secs(1);

    loop {
        consumer().await;
        warn!(
            "background task '{}' exited; restarting in {:?}",
            name, backoff
        );
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(MAX_BACKOFF);
    }
}

pub async fn shutdown_signal() {
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
