use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use tracing::info;

use chat_api::config::AppConfig;
use chat_api::state::AppState;
use chat_api::{
    build_state, connect_pool, export, health, hooks, huddle, init_tracing, messaging, metrics,
    notifications, retention, scheduled, shutdown_signal, slack_import, supervise,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    init_tracing();

    let metrics_handle = metrics::install_recorder()?;
    ::metrics::counter!("chat_worker_starts_total").increment(1);

    let config = AppConfig::from_env();
    let port = worker_port();
    let redis_url = config.redis_url.clone();

    let pool = connect_pool(&config).await?;
    let state = build_state(pool, config).await?;

    info!("chat-worker starting background consumers (single replica by contract)");

    spawn_consumers(&state, &redis_url);

    {
        let digest_state = state.clone();
        tokio::spawn(async move {
            supervise("mention_email_digest", || {
                let digest_state = digest_state.clone();
                async move {
                    notifications::email::start_digest_job(digest_state).await;
                }
            })
            .await;
        });
    }

    {
        let backfill_state = state.clone();
        tokio::spawn(async move {
            chat_api::search::backfill(&backfill_state).await;
        });
    }

    let app = Router::new()
        .merge(health::router(state.clone()))
        .merge(metrics::router(metrics_handle));

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("chat-worker listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

/// Defaults to 3005 rather than the shared `PORT` default of 3000 so the
/// documented three-terminal dev workflow does not collide with chat-api.
fn worker_port() -> u16 {
    std::env::var("WORKER_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .or_else(|| std::env::var("PORT").ok().and_then(|v| v.parse().ok()))
        .unwrap_or(3005)
}

fn spawn_consumers(state: &Arc<AppState>, redis_url: &str) {
    let hook_repo = Arc::new(state.hook_repo.clone());
    let notif_repo = Arc::new(state.notification_repo.clone());
    let huddle_repo = Arc::new(state.huddle_repo.clone());

    {
        let redis_url = redis_url.to_string();
        let hook_repo = hook_repo.clone();
        tokio::spawn(async move {
            supervise("hook_consumer", || {
                let redis_url = redis_url.clone();
                let hook_repo = hook_repo.clone();
                async move {
                    hooks::executor::start_hook_consumer(&redis_url, hook_repo).await;
                }
            })
            .await;
        });
    }

    {
        let redis_url = redis_url.to_string();
        let reminder_state = state.clone();
        tokio::spawn(async move {
            supervise("reminder_checker", || {
                let redis_url = redis_url.clone();
                let reminder_state = reminder_state.clone();
                async move {
                    hooks::executor::start_reminder_checker(&redis_url, reminder_state).await;
                }
            })
            .await;
        });
    }

    {
        let reconciler_state = state.clone();
        tokio::spawn(async move {
            supervise("unread_reconciler", || {
                let reconciler_state = reconciler_state.clone();
                async move {
                    messaging::reconcile::start_unread_reconciler(reconciler_state).await;
                }
            })
            .await;
        });
    }

    {
        let trimmer_state = state.clone();
        tokio::spawn(async move {
            supervise("stream_trimmer", || {
                let trimmer_state = trimmer_state.clone();
                async move {
                    messaging::stream_trim::start_stream_trimmer(trimmer_state).await;
                }
            })
            .await;
        });
    }

    {
        let export_state = state.clone();
        tokio::spawn(async move {
            supervise("export_worker", || {
                let export_state = export_state.clone();
                async move {
                    export::job::start_export_worker(export_state).await;
                }
            })
            .await;
        });
    }

    {
        let import_state = state.clone();
        tokio::spawn(async move {
            supervise("slack_import_worker", || {
                let import_state = import_state.clone();
                async move {
                    slack_import::job::start_import_worker(import_state).await;
                }
            })
            .await;
        });
    }

    {
        let retention_state = state.clone();
        tokio::spawn(async move {
            supervise("retention_job", || {
                let retention_state = retention_state.clone();
                async move {
                    retention::job::start_retention_job(retention_state).await;
                }
            })
            .await;
        });
    }

    {
        let dispatcher_state = state.clone();
        tokio::spawn(async move {
            supervise("scheduled_dispatcher", || {
                let dispatcher_state = dispatcher_state.clone();
                async move {
                    scheduled::executor::start_dispatcher(dispatcher_state).await;
                }
            })
            .await;
        });
    }

    {
        let redis_url = redis_url.to_string();
        let notif_repo = notif_repo.clone();
        let push = Arc::new(state.push_sender.clone());
        let consumer_state = state.clone();
        tokio::spawn(async move {
            supervise("notification_consumer", || {
                let redis_url = redis_url.clone();
                let notif_repo = notif_repo.clone();
                let push_sender = push.clone();
                let state = consumer_state.clone();
                async move {
                    notifications::consumer::start_consumer(
                        &redis_url,
                        state,
                        notif_repo,
                        push_sender,
                    )
                    .await;
                }
            })
            .await;
        });
    }

    {
        let redis_url = redis_url.to_string();
        let notif_repo = notif_repo.clone();
        let ring_repo = huddle_repo.clone();
        tokio::spawn(async move {
            supervise("call_notification_consumer", || {
                let redis_url = redis_url.clone();
                let notif_repo = notif_repo.clone();
                let huddle_repo = ring_repo.clone();
                async move {
                    notifications::consumer::start_call_consumer(
                        &redis_url,
                        notif_repo,
                        huddle_repo,
                    )
                    .await;
                }
            })
            .await;
        });
    }

    {
        let redis_url = redis_url.to_string();
        let huddle_repo = huddle_repo.clone();
        tokio::spawn(async move {
            supervise("huddle_consumer", || {
                let redis_url = redis_url.clone();
                let huddle_repo = huddle_repo.clone();
                async move {
                    huddle::consumer::start_consumer(&redis_url, huddle_repo).await;
                }
            })
            .await;
        });
    }
}
