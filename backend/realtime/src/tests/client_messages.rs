use std::time::Duration;

use futures_util::StreamExt;
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;

async fn await_typing_for_channel(channel_id: Uuid) -> Option<serde_json::Value> {
    let url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into());
    let client = redis::Client::open(url).expect("redis client");
    let mut pubsub = client.get_async_pubsub().await.expect("pubsub");
    pubsub.subscribe("events:typing").await.expect("subscribe");
    let mut stream = pubsub.into_on_message();

    let deadline = tokio::time::Instant::now() + Duration::from_millis(500);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return None;
        }
        let msg = match tokio::time::timeout(remaining, stream.next()).await {
            Ok(Some(m)) => m,
            _ => return None,
        };
        let payload: String = match msg.get_payload() {
            Ok(p) => p,
            Err(_) => continue,
        };
        let env: serde_json::Value = match serde_json::from_str(&payload) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let matches = env
            .get("payload")
            .and_then(|p| p.get("channel_id"))
            .and_then(|v| v.as_str())
            == Some(&channel_id.to_string());
        if matches {
            return Some(env);
        }
    }
}

#[sqlx::test(migrations = "../migrations")]
async fn subscribe_member_workspace_sends_presence_batch(pool: PgPool) {
    let cm = manager(pool).await;
    let user = seed_user(cm.db()).await;
    let ws = seed_workspace(cm.db(), user).await;
    add_ws_member(cm.db(), ws, user).await;

    cm.presence_set_online(user).await;

    let (conn_id, mut rx) = fake_conn(&cm, user);

    let text = serde_json::json!({ "type": "subscribe", "workspace_id": ws }).to_string();
    crate::ws_handler::handle_client_message(&text, &conn_id, user, &cm).await;

    let frames = drain_json(&mut rx);
    let batch = frames
        .iter()
        .find(|f| f.get("type").and_then(|v| v.as_str()) == Some("presence.batch"))
        .expect("conn should receive presence.batch after subscribe");

    let users = batch
        .get("users")
        .and_then(|v| v.as_array())
        .expect("presence.batch has users array");
    let contains_user = users.iter().any(|u| {
        u.get("user_id").and_then(|v| v.as_str()) == Some(&user.to_string())
            && u.get("status").and_then(|v| v.as_str()) == Some("online")
    });
    assert!(
        contains_user,
        "presence.batch should list the online member, got: {users:?}"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn subscribe_non_member_workspace_denied(pool: PgPool) {
    let cm = manager(pool).await;
    let owner = seed_user(cm.db()).await;
    let ws = seed_workspace(cm.db(), owner).await;
    let outsider = seed_user(cm.db()).await;

    let (conn_id, mut rx) = fake_conn(&cm, outsider);

    let text = serde_json::json!({ "type": "subscribe", "workspace_id": ws }).to_string();
    crate::ws_handler::handle_client_message(&text, &conn_id, outsider, &cm).await;

    let frames = drain_json(&mut rx);
    assert!(
        !frames
            .iter()
            .any(|f| f.get("type").and_then(|v| v.as_str()) == Some("presence.batch")),
        "non-member subscribe must NOT receive presence.batch, got: {frames:?}"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn subscribe_member_then_workspace_broadcast_reaches_conn(pool: PgPool) {
    let cm = manager(pool).await;
    let user = seed_user(cm.db()).await;
    let ws = seed_workspace(cm.db(), user).await;
    add_ws_member(cm.db(), ws, user).await;

    let (conn_id, mut rx) = fake_conn(&cm, user);

    let text = serde_json::json!({ "type": "subscribe", "workspace_id": ws }).to_string();
    crate::ws_handler::handle_client_message(&text, &conn_id, user, &cm).await;
    let _ = drain_json(&mut rx);

    cm.broadcast_to_workspace(ws, r#"{"type":"workspace.ping"}"#)
        .await;

    let frames = drain_json(&mut rx);
    assert!(
        frames
            .iter()
            .any(|f| f.get("type").and_then(|v| v.as_str()) == Some("workspace.ping")),
        "subscribed conn should receive workspace broadcast, got: {frames:?}"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn channel_join_member_then_broadcast_reaches_conn(pool: PgPool) {
    let cm = manager(pool).await;
    let (user, _ws, ch) = seed_member_in_channel(cm.db()).await;

    let (conn_id, mut rx) = fake_conn(&cm, user);

    let text = serde_json::json!({ "type": "channel.join", "channel_id": ch }).to_string();
    crate::ws_handler::handle_client_message(&text, &conn_id, user, &cm).await;

    cm.broadcast_to_channel(ch, r#"{"type":"message.new"}"#)
        .await;

    let frames = drain_json(&mut rx);
    assert!(
        frames
            .iter()
            .any(|f| f.get("type").and_then(|v| v.as_str()) == Some("message.new")),
        "joined member should receive channel broadcast, got: {frames:?}"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn channel_join_non_member_denied_no_broadcast(pool: PgPool) {
    let cm = manager(pool).await;
    let creator = seed_user(cm.db()).await;
    let ws = seed_workspace(cm.db(), creator).await;
    add_ws_member(cm.db(), ws, creator).await;
    let ch = seed_channel(cm.db(), ws, creator).await;
    add_ch_member(cm.db(), ch, creator).await;

    let outsider = seed_user(cm.db()).await;
    let (conn_id, mut rx) = fake_conn(&cm, outsider);

    let text = serde_json::json!({ "type": "channel.join", "channel_id": ch }).to_string();
    crate::ws_handler::handle_client_message(&text, &conn_id, outsider, &cm).await;

    cm.broadcast_to_channel(ch, r#"{"type":"message.new"}"#)
        .await;

    let frames = drain_json(&mut rx);
    assert!(
        frames.is_empty(),
        "denied (non-member) join must not subscribe; broadcast must not reach conn, got: {frames:?}"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn channel_leave_stops_broadcasts(pool: PgPool) {
    let cm = manager(pool).await;
    let (user, _ws, ch) = seed_member_in_channel(cm.db()).await;

    let (conn_id, mut rx) = fake_conn(&cm, user);

    let join = serde_json::json!({ "type": "channel.join", "channel_id": ch }).to_string();
    crate::ws_handler::handle_client_message(&join, &conn_id, user, &cm).await;

    let leave = serde_json::json!({ "type": "channel.leave", "channel_id": ch }).to_string();
    crate::ws_handler::handle_client_message(&leave, &conn_id, user, &cm).await;

    cm.broadcast_to_channel(ch, r#"{"type":"message.new"}"#)
        .await;

    let frames = drain_json(&mut rx);
    assert!(
        frames.is_empty(),
        "after leave, channel broadcast must not reach conn, got: {frames:?}"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn typing_start_member_publishes(pool: PgPool) {
    let cm = manager(pool).await;
    let (user, _ws, ch) = seed_member_in_channel(cm.db()).await;
    let (conn_id, _rx) = fake_conn(&cm, user);

    let listener = tokio::spawn(await_typing_for_channel(ch));
    settle().await;

    let text = serde_json::json!({ "type": "typing.start", "channel_id": ch }).to_string();
    crate::ws_handler::handle_client_message(&text, &conn_id, user, &cm).await;

    let env = listener
        .await
        .expect("listener task")
        .expect("typing.start should publish a typing.indicator to events:typing");

    assert_eq!(
        env.get("event_type").and_then(|v| v.as_str()),
        Some("typing.indicator")
    );
    let payload = env.get("payload").expect("envelope payload");
    let user_str = user.to_string();
    assert_eq!(
        payload.get("user_id").and_then(|v| v.as_str()),
        Some(user_str.as_str())
    );
    assert_eq!(
        payload.get("is_typing").and_then(|v| v.as_bool()),
        Some(true)
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn typing_start_non_member_denied_no_publish(pool: PgPool) {
    let cm = manager(pool).await;
    let creator = seed_user(cm.db()).await;
    let ws = seed_workspace(cm.db(), creator).await;
    add_ws_member(cm.db(), ws, creator).await;
    let ch = seed_channel(cm.db(), ws, creator).await;
    add_ch_member(cm.db(), ch, creator).await;

    let outsider = seed_user(cm.db()).await;
    let (conn_id, _rx) = fake_conn(&cm, outsider);

    let listener = tokio::spawn(await_typing_for_channel(ch));
    settle().await;

    let text = serde_json::json!({ "type": "typing.start", "channel_id": ch }).to_string();
    crate::ws_handler::handle_client_message(&text, &conn_id, outsider, &cm).await;

    let env = listener.await.expect("listener task");
    assert!(
        env.is_none(),
        "non-member typing.start must NOT publish, got: {env:?}"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn typing_stop_member_publishes_false(pool: PgPool) {
    let cm = manager(pool).await;
    let (user, _ws, ch) = seed_member_in_channel(cm.db()).await;
    let (conn_id, _rx) = fake_conn(&cm, user);

    let listener = tokio::spawn(await_typing_for_channel(ch));
    settle().await;

    let text = serde_json::json!({ "type": "typing.stop", "channel_id": ch }).to_string();
    crate::ws_handler::handle_client_message(&text, &conn_id, user, &cm).await;

    let env = listener
        .await
        .expect("listener task")
        .expect("typing.stop should publish a typing.indicator");
    assert_eq!(
        env.get("payload")
            .and_then(|p| p.get("is_typing"))
            .and_then(|v| v.as_bool()),
        Some(false)
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn ping_replies_pong(pool: PgPool) {
    let cm = manager(pool).await;
    let user = seed_user(cm.db()).await;
    let (conn_id, mut rx) = fake_conn(&cm, user);

    crate::ws_handler::handle_client_message(r#"{"type":"ping"}"#, &conn_id, user, &cm).await;

    let frame = next_json(&mut rx).expect("ping should enqueue a frame");
    assert_eq!(frame.get("type").and_then(|v| v.as_str()), Some("pong"));
}

#[sqlx::test(migrations = "../migrations")]
async fn invalid_json_is_ignored(pool: PgPool) {
    let cm = manager(pool).await;
    let user = seed_user(cm.db()).await;
    let (conn_id, mut rx) = fake_conn(&cm, user);

    crate::ws_handler::handle_client_message("{not json", &conn_id, user, &cm).await;
    crate::ws_handler::handle_client_message(r#"{"type":"totally.unknown"}"#, &conn_id, user, &cm)
        .await;
    crate::ws_handler::handle_client_message(r#"{"foo":"bar"}"#, &conn_id, user, &cm).await;

    let frames = drain_json(&mut rx);
    assert!(
        frames.is_empty(),
        "malformed/unknown messages must enqueue nothing, got: {frames:?}"
    );
}
