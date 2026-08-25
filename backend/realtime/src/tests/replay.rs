use super::common::*;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::replay::{replay_workspace, workspace_stream, Replay};

/// Writes an event into a workspace log the way the API's publisher does.
async fn append(
    cm: &std::sync::Arc<crate::connection_manager::ConnectionManager>,
    workspace_id: Uuid,
    event_type: &str,
    payload: serde_json::Value,
) -> String {
    let mut conn = cm.redis();
    let event = json!({
        "id": Uuid::new_v4(),
        "event_type": event_type,
        "payload": payload,
        "timestamp": chrono::Utc::now(),
    });
    redis::cmd("XADD")
        .arg(workspace_stream(workspace_id))
        .arg("*")
        .arg("event")
        .arg(event.to_string())
        .query_async(&mut conn)
        .await
        .expect("append to the workspace log")
}

async fn clear(
    cm: &std::sync::Arc<crate::connection_manager::ConnectionManager>,
    workspace_id: Uuid,
) {
    let mut conn = cm.redis();
    let _: redis::RedisResult<()> = redis::cmd("DEL")
        .arg(workspace_stream(workspace_id))
        .query_async(&mut conn)
        .await;
}

fn types(rx: &mut tokio::sync::mpsc::Receiver<axum::extract::ws::Message>) -> Vec<String> {
    drain_json(rx)
        .into_iter()
        .filter_map(|v| v.get("type").and_then(|t| t.as_str()).map(String::from))
        .collect()
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_gap_is_replayed_in_order_to_the_client_that_missed_it(pool: PgPool) {
    let cm = manager(pool).await;
    let workspace = Uuid::new_v4();
    let channel = Uuid::new_v4();
    let user = Uuid::new_v4();
    clear(&cm, workspace).await;

    // Where the client got to before its socket dropped.
    let position = append(
        &cm,
        workspace,
        "message.created",
        json!({ "id": Uuid::new_v4(), "channel_id": channel, "content": "seen" }),
    )
    .await;

    for i in 0..3 {
        append(
            &cm,
            workspace,
            "message.created",
            json!({ "id": Uuid::new_v4(), "channel_id": channel, "content": format!("missed {i}") }),
        )
        .await;
    }

    let (conn_id, mut rx) = fake_conn(&cm, user);
    cm.subscribe_workspace(&conn_id, workspace);
    cm.join_channel(&conn_id, channel);

    let outcome = replay_workspace(&cm, conn_id, workspace, &position).await;
    assert!(
        matches!(outcome, Replay::Caught { events: 3, .. }),
        "the three missed events are replayed, not the one already seen: {outcome:?}"
    );

    let delivered = drain_json(&mut rx);
    let contents: Vec<String> = delivered
        .iter()
        .filter_map(|v| v.get("message").and_then(|m| m.get("content")))
        .filter_map(|c| c.as_str().map(String::from))
        .collect();
    assert_eq!(contents, vec!["missed 0", "missed 1", "missed 2"]);

    assert!(
        delivered
            .iter()
            .all(|v| v.get("stream_id").and_then(|s| s.as_str()).is_some()),
        "a replayed frame carries its position, or the client cannot resume from it"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_replay_never_carries_a_channel_the_client_is_not_in(pool: PgPool) {
    let cm = manager(pool).await;
    let workspace = Uuid::new_v4();
    let mine = Uuid::new_v4();
    let theirs = Uuid::new_v4();
    let user = Uuid::new_v4();
    clear(&cm, workspace).await;

    let position = append(
        &cm,
        workspace,
        "message.created",
        json!({ "id": Uuid::new_v4(), "channel_id": mine, "content": "start" }),
    )
    .await;

    append(
        &cm,
        workspace,
        "message.created",
        json!({ "id": Uuid::new_v4(), "channel_id": theirs, "content": "private business" }),
    )
    .await;
    append(
        &cm,
        workspace,
        "message.created",
        json!({ "id": Uuid::new_v4(), "channel_id": mine, "content": "for me" }),
    )
    .await;

    let (conn_id, mut rx) = fake_conn(&cm, user);
    cm.subscribe_workspace(&conn_id, workspace);
    cm.join_channel(&conn_id, mine);

    replay_workspace(&cm, conn_id, workspace, &position).await;

    let delivered = drain_json(&mut rx);
    let contents: Vec<String> = delivered
        .iter()
        .filter_map(|v| v.get("message").and_then(|m| m.get("content")))
        .filter_map(|c| c.as_str().map(String::from))
        .collect();
    assert_eq!(
        contents,
        vec!["for me"],
        "a backlog must obey the same visibility rules as the live path"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_position_older_than_the_log_asks_the_client_to_refetch(pool: PgPool) {
    let cm = manager(pool).await;
    let workspace = Uuid::new_v4();
    let user = Uuid::new_v4();
    clear(&cm, workspace).await;

    append(
        &cm,
        workspace,
        "message.created",
        json!({ "id": Uuid::new_v4(), "channel_id": Uuid::new_v4(), "content": "only entry" }),
    )
    .await;

    let (conn_id, _rx) = fake_conn(&cm, user);
    cm.subscribe_workspace(&conn_id, workspace);

    // A position from before anything still in the log: the gap was trimmed and
    // there is no way to know what was in it.
    let outcome = replay_workspace(&cm, conn_id, workspace, "1-0").await;
    assert_eq!(outcome, Replay::RefetchRequired);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn replaying_an_empty_gap_delivers_nothing(pool: PgPool) {
    let cm = manager(pool).await;
    let workspace = Uuid::new_v4();
    let user = Uuid::new_v4();
    clear(&cm, workspace).await;

    let position = append(
        &cm,
        workspace,
        "message.created",
        json!({ "id": Uuid::new_v4(), "channel_id": Uuid::new_v4(), "content": "seen" }),
    )
    .await;

    let (conn_id, mut rx) = fake_conn(&cm, user);
    cm.subscribe_workspace(&conn_id, workspace);

    let outcome = replay_workspace(&cm, conn_id, workspace, &position).await;
    assert!(matches!(outcome, Replay::Caught { events: 0, .. }));
    assert!(types(&mut rx).is_empty(), "nothing to replay, nothing sent");
}
