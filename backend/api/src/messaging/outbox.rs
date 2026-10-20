use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use tracing::{info, warn};
use uuid::Uuid;

use shared_events::Event;

use crate::state::AppState;

const POLL_INTERVAL: Duration = Duration::from_secs(2);
const BATCH: i64 = 100;
pub const FAST_PATH_GRACE_SECS: i64 = 5;
const CLAIM_LEASE_SECS: i64 = 60;
const KEEP_PUBLISHED_HOURS: i64 = 24;

#[derive(Debug)]
struct OutboxRow {
    id: i64,
    event_id: Uuid,
    event_type: String,
    workspace_id: Uuid,
    payload: serde_json::Value,
    created_at: DateTime<Utc>,
}

pub async fn relay_once(state: &AppState) -> usize {
    let due = sqlx::query_as!(
        OutboxRow,
        "UPDATE event_outbox SET claimed_at = NOW()
          WHERE id IN (
                SELECT id FROM event_outbox
                 WHERE published_at IS NULL
                   AND created_at < NOW() - make_interval(secs => $2)
                   AND (claimed_at IS NULL OR claimed_at < NOW() - make_interval(secs => $3))
                 ORDER BY id
                 LIMIT $1
                 FOR UPDATE SKIP LOCKED)
      RETURNING id, event_id, event_type, workspace_id, payload, created_at",
        BATCH,
        FAST_PATH_GRACE_SECS as f64,
        CLAIM_LEASE_SECS as f64
    )
    .fetch_all(&state.pool)
    .await;

    let due = match due {
        Ok(due) => due,
        Err(e) => {
            warn!("could not read the event outbox: {}", e);
            return 0;
        }
    };

    let mut relayed = 0;
    for row in due {
        let mut event = Event {
            id: row.event_id,
            event_type: row.event_type,
            payload: row.payload,
            timestamp: row.created_at,
            stream_id: None,
        };
        match state
            .publisher
            .emit(&mut event, Some(row.workspace_id))
            .await
        {
            Ok(()) => {
                state.publisher.mark_published(row.id).await;
                relayed += 1;
                metrics::counter!("event_outbox_relayed_total").increment(1);
            }
            Err(e) => warn!(
                "outbox relay could not publish {} (id={}): {}",
                event.event_type, event.id, e
            ),
        }
    }

    let pruned = sqlx::query!(
        "DELETE FROM event_outbox
          WHERE id IN (
                SELECT id FROM event_outbox
                 WHERE published_at < NOW() - make_interval(hours => $1)
                 LIMIT 1000)",
        KEEP_PUBLISHED_HOURS as i32
    )
    .execute(&state.pool)
    .await;
    if let Err(e) = pruned {
        warn!("could not prune the event outbox: {}", e);
    }

    relayed
}

pub async fn start_relay(state: Arc<AppState>) {
    info!("Event outbox relay started");
    loop {
        tokio::time::sleep(POLL_INTERVAL).await;
        let relayed = relay_once(&state).await;
        if relayed > 0 {
            info!(
                "outbox relay published {} events the fast path missed",
                relayed
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use super::*;
    use crate::http_tests::common::app_and_state;

    async fn stream_len(state: &AppState, workspace_id: Uuid) -> i64 {
        let mut conn = state.redis.clone();
        redis::cmd("XLEN")
            .arg(crate::messaging::publisher::workspace_stream(workspace_id))
            .query_async(&mut conn)
            .await
            .unwrap_or(0)
    }

    async fn outbox_state(state: &AppState, event_id: Uuid) -> Option<Option<DateTime<Utc>>> {
        sqlx::query_scalar("SELECT published_at FROM event_outbox WHERE event_id = $1")
            .bind(event_id)
            .fetch_optional(&state.pool)
            .await
            .unwrap()
    }

    #[test_macros::db_test(migrations = "../migrations")]
    async fn a_staged_event_is_gone_with_its_transaction(pool: PgPool) {
        let (_, state) = app_and_state(pool).await;
        let ws = Uuid::new_v4();

        let mut tx = state.pool.begin().await.unwrap();
        let staged = state
            .publisher
            .stage(
                &mut tx,
                "message.created",
                ws,
                serde_json::json!({ "n": 1 }),
            )
            .await
            .unwrap();
        let event_id = staged.event_id();
        tx.rollback().await.unwrap();

        assert!(outbox_state(&state, event_id).await.is_none());
    }

    #[test_macros::db_test(migrations = "../migrations")]
    async fn a_committed_event_is_published_and_marked(pool: PgPool) {
        let (_, state) = app_and_state(pool).await;
        let ws = Uuid::new_v4();
        let before = stream_len(&state, ws).await;

        let mut tx = state.pool.begin().await.unwrap();
        let staged = state
            .publisher
            .stage(
                &mut tx,
                "message.created",
                ws,
                serde_json::json!({ "n": 2 }),
            )
            .await
            .unwrap();
        let event_id = staged.event_id();
        tx.commit().await.unwrap();
        state.publisher.dispatch(staged).await;

        assert_eq!(stream_len(&state, ws).await, before + 1);
        assert!(outbox_state(&state, event_id).await.flatten().is_some());
    }

    #[test_macros::db_test(migrations = "../migrations")]
    async fn the_relay_publishes_what_the_fast_path_missed(pool: PgPool) {
        let (_, state) = app_and_state(pool).await;
        let ws = Uuid::new_v4();
        let event_id = Uuid::new_v4();
        let before = stream_len(&state, ws).await;

        sqlx::query(
            "INSERT INTO event_outbox (event_id, event_type, workspace_id, payload, created_at)
             VALUES ($1, 'message.created', $2, '{\"n\": 3}', NOW() - make_interval(secs => $3))",
        )
        .bind(event_id)
        .bind(ws)
        .bind((FAST_PATH_GRACE_SECS + 1) as f64)
        .execute(&state.pool)
        .await
        .unwrap();

        assert!(relay_once(&state).await >= 1);
        assert_eq!(stream_len(&state, ws).await, before + 1);
        assert!(outbox_state(&state, event_id).await.flatten().is_some());

        assert_eq!(
            relay_once(&state).await,
            0,
            "a published row is not relayed again"
        );
    }
}
