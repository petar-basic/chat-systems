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

    info!(
        "Sessions are signed with key {:?}; the gateway's JWT_PUBLIC_KEYS={}",
        config.jwt_keys.current_id(),
        config.jwt_keys.verifying().listing()
    );

    if let Err(e) = chat_api::sealed::reseal::run(
        &pool,
        &config.secrets_keys,
        config.legacy_jwt_secret.as_deref(),
    )
    .await
    {
        warn!("Stored secrets could not be re-sealed: {}", e);
    }

    let state = build_state(pool, config).await?;

    if let Err(e) = state.auth_service.bootstrap_admin().await {
        warn!("Admin bootstrap failed: {}", e);
    }
    if let Err(e) = chat_api::setup::announce(&state).await {
        warn!("Setup token could not be prepared: {}", e);
    }

    let app = build_app(state).merge(metrics::router(metrics_handle));

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("chat-api listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    Ok(())
}
