use std::sync::Arc;
use std::time::{Duration, Instant};

use tracing::{info, warn};
use uuid::Uuid;

use shared_events::Event;

use super::repo::AnalyticsRepo;
use crate::messaging::stream_group::StreamGroup;
use crate::state::AppState;

const FORGET_EVERY: Duration = Duration::from_secs(3600);
const REMEMBER_DAYS: i32 = 7;

pub async fn start_consumer(redis_url: &str, repo: Arc<AnalyticsRepo>) {
    let Some(mut group) = StreamGroup::connect(redis_url, "analytics").await else {
        return;
    };

    info!("Analytics consumer started");
    let mut forgot_at: Option<Instant> = None;

    loop {
        for delivery in group.next_batch().await {
            match count(&repo, &delivery.event).await {
                Ok(()) => group.ack(&delivery.key, &delivery.id).await,
                Err(e) => warn!(
                    "Analytics consumer: {} (id={}) not counted yet: {}",
                    delivery.event.event_type, delivery.event.id, e
                ),
            }
        }

        if forgot_at.is_none_or(|at| at.elapsed() >= FORGET_EVERY) {
            forgot_at = Some(Instant::now());
            if let Err(e) = repo.forget_counted_before(REMEMBER_DAYS).await {
                warn!("Analytics consumer: could not forget counted events: {}", e);
            }
        }
    }
}

fn id_at(event: &Event, key: &str) -> Option<Uuid> {
    event
        .payload
        .get(key)
        .and_then(|v| v.as_str())
        .and_then(|v| v.parse().ok())
}

pub(crate) async fn count(repo: &AnalyticsRepo, event: &Event) -> sqlx::Result<()> {
    match event.event_type.as_str() {
        "message.created" => match id_at(event, "id") {
            Some(message_id) => repo.count_message(event.id, message_id).await,
            None => Ok(()),
        },
        "reaction.added" => match id_at(event, "id") {
            Some(reaction_id) => repo.count_reaction(event.id, reaction_id).await,
            None => Ok(()),
        },
        "channel.read_state" => match (id_at(event, "workspace_id"), id_at(event, "user_id")) {
            (Some(workspace_id), Some(user_id)) => {
                repo.mark_active(workspace_id, user_id, event.timestamp.date_naive())
                    .await
            }
            _ => Ok(()),
        },
        _ => Ok(()),
    }
}

pub async fn backfill(state: &AppState) {
    match state.analytics_repo.backfill().await {
        Ok(true) => info!("analytics backfill: counted what came before the rollup"),
        Ok(false) => {}
        Err(e) => warn!("analytics backfill failed: {}", e),
    }
}
