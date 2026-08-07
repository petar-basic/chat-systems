use std::net::SocketAddr;

use tracing::{info, warn};

use chat_api::config::AppConfig;
use chat_api::{build_app, build_state, connect_pool, init_tracing, metrics, shutdown_signal};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    init_tracing();

    let metrics_handle = metrics::install_recorder()?;
    ::metrics::counter!("chat_api_starts_total").increment(1);

    let config = AppConfig::from_env();
    let port = config.port;

    let pool = connect_pool(&config).await?;

    sqlx::migrate!("../migrations").run(&pool).await?;
    info!("Migrations applied");

    let state = build_state(pool, config).await?;

    if let Err(e) = state.auth_service.bootstrap_admin().await {
        warn!("Admin bootstrap failed: {}", e);
    }

    let app = build_app(state).merge(metrics::router(metrics_handle));

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("chat-api listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}
