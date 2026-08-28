use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;
use crate::messaging::stream_group::StreamGroup;

async fn stream_len(state: &crate::state::AppState, workspace_id: Uuid) -> i64 {
    let mut conn = state.redis.clone();
    redis::cmd("XLEN")
        .arg(crate::messaging::publisher::workspace_stream(workspace_id))
        .query_async(&mut conn)
        .await
        .unwrap_or(0)
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_durable_event_lands_in_its_workspace_log(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "stream-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Stream WS").await;
    let ch = seed_channel(&state, ws, owner_id, "main", false).await;

    let before = stream_len(&state, ws).await;
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch}/messages"),
        Some(&token),
        Some(serde_json::json!({ "content": "durable" })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);

    assert_eq!(
        stream_len(&state, ws).await,
        before + 1,
        "a message is replayable, so it is written to the workspace log"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_ephemeral_event_is_not_written_to_the_log(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "stream-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Stream WS").await;

    let before = stream_len(&state, ws).await;
    let _ = state
        .publisher
        .publish_scoped(
            "typing.indicator",
            ws,
            serde_json::json!({ "channel_id": Uuid::new_v4() }),
        )
        .await;

    assert_eq!(
        stream_len(&state, ws).await,
        before,
        "replaying a typing indicator is not recovery, so it never enters the log"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn two_worker_replicas_each_see_an_event_once(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "stream-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Stream WS").await;
    let ch = seed_channel(&state, ws, owner_id, "main", false).await;

    let redis_url = state.config.redis_url.clone();

    // One message first so the stream and its index entry exist, then both
    // replicas join and drain it. From here the group is caught up and anything
    // new is delivered to exactly one of them.
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch}/messages"),
        Some(&token),
        Some(serde_json::json!({ "content": "warm up" })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);

    let group_name = format!("test-workers-{}", Uuid::new_v4());
    let mut first = StreamGroup::connect(&redis_url, &group_name)
        .await
        .expect("first replica");
    let mut second = StreamGroup::connect(&redis_url, &group_name)
        .await
        .expect("second replica");

    for replica in [&mut first, &mut second] {
        for delivery in replica.next_batch().await {
            replica.ack(&delivery.key, &delivery.id).await;
        }
    }

    for i in 0..4 {
        let (status, _) = send(
            &app,
            "POST",
            &format!("/api/channels/{ch}/messages"),
            Some(&token),
            Some(serde_json::json!({ "content": format!("work {i}") })),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK);
    }

    // The group reads every workspace stream, which is what a real worker wants
    // and what makes this test share a Redis with the others — so only this
    // channel's events are counted.
    let mine = |delivery: &crate::messaging::stream_group::Delivery| {
        delivery
            .event
            .payload
            .get("channel_id")
            .and_then(|v| v.as_str())
            == Some(&ch.to_string())
            && delivery
                .event
                .payload
                .get("content")
                .and_then(|v| v.as_str())
                .is_some_and(|c| c.starts_with("work "))
    };

    // A deadline rather than a fixed number of rounds: the group reads every
    // workspace stream, so on a busy Redis a round can come back full of another
    // test's events. What is being asserted is exactly-once delivery, not how
    // few polls it takes.
    let mut seen: Vec<Uuid> = Vec::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while std::time::Instant::now() < deadline {
        for delivery in first.next_batch().await {
            if mine(&delivery) {
                seen.push(delivery.event.id);
            }
            first.ack(&delivery.key, &delivery.id).await;
        }
        for delivery in second.next_batch().await {
            if mine(&delivery) {
                seen.push(delivery.event.id);
            }
            second.ack(&delivery.key, &delivery.id).await;
        }
        if seen.len() >= 4 {
            break;
        }
    }

    assert_eq!(seen.len(), 4, "every event reaches the group exactly once");
    let unique: std::collections::HashSet<Uuid> = seen.iter().copied().collect();
    assert_eq!(
        unique.len(),
        seen.len(),
        "two replicas must not both process the same event"
    );
}
