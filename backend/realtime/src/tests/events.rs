use super::common::*;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

fn next_type(rx: &mut tokio::sync::mpsc::Receiver<axum::extract::ws::Message>) -> Option<String> {
    next_json(rx).and_then(|v| v.get("type").and_then(|t| t.as_str()).map(String::from))
}

fn drain_types(rx: &mut tokio::sync::mpsc::Receiver<axum::extract::ws::Message>) -> Vec<String> {
    drain_json(rx)
        .into_iter()
        .filter_map(|v| v.get("type").and_then(|t| t.as_str()).map(String::from))
        .collect()
}

#[sqlx::test(migrations = "../migrations")]
async fn message_created_delivers_message_new_to_channel_subscriber(pool: PgPool) {
    let cm = manager(pool).await;
    let user = Uuid::new_v4();
    let channel = Uuid::new_v4();
    let message_id = Uuid::new_v4();

    let (conn_id, mut rx) = fake_conn(&cm, user);
    cm.join_channel(&conn_id, channel);

    let payload = json!({
        "message_id": message_id.to_string(),
        "channel_id": channel.to_string(),
        "user_id": user.to_string(),
        "content": "hello world",
    });
    crate::event_consumer::handle_event("message.created", &payload, &cm).await;

    let frame = next_json(&mut rx).expect("subscriber should receive a frame");
    assert_eq!(
        frame.get("type").and_then(|t| t.as_str()),
        Some("message.new")
    );
    assert_eq!(
        frame
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str()),
        Some("hello world")
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn message_created_mentioned_user_in_channel_gets_message_new_not_notification(pool: PgPool) {
    let cm = manager(pool).await;
    let sender = Uuid::new_v4();
    let mentioned = Uuid::new_v4();
    let channel = Uuid::new_v4();

    let (conn_id, mut rx) = fake_conn(&cm, mentioned);
    cm.join_channel(&conn_id, channel);

    let payload = json!({
        "message_id": Uuid::new_v4().to_string(),
        "channel_id": channel.to_string(),
        "user_id": sender.to_string(),
        "content": "ping @you",
        "mentioned_user_ids": [mentioned.to_string()],
    });
    crate::event_consumer::handle_event("message.created", &payload, &cm).await;

    let types = drain_types(&mut rx);
    assert_eq!(types, vec!["message.new".to_string()]);
}

#[sqlx::test(migrations = "../migrations")]
async fn message_created_does_not_notify_sender_even_if_mentioned(pool: PgPool) {
    let cm = manager(pool).await;
    let sender = Uuid::new_v4();
    let channel = Uuid::new_v4();

    let (_sender_conn, mut sender_rx) = fake_conn(&cm, sender);

    let payload = json!({
        "message_id": Uuid::new_v4().to_string(),
        "channel_id": channel.to_string(),
        "user_id": sender.to_string(),
        "content": "self mention @me",
        "mentioned_user_ids": [sender.to_string()],
    });
    crate::event_consumer::handle_event("message.created", &payload, &cm).await;

    assert!(next_json(&mut sender_rx).is_none());
}

#[sqlx::test(migrations = "../migrations")]
async fn message_created_with_unparseable_channel_id_is_a_noop(pool: PgPool) {
    let cm = manager(pool).await;
    let user = Uuid::new_v4();

    let (conn_id, mut rx) = fake_conn(&cm, user);
    cm.join_channel(&conn_id, Uuid::new_v4());

    let payload = json!({
        "message_id": Uuid::new_v4().to_string(),
        "channel_id": "not-a-uuid",
        "user_id": user.to_string(),
        "content": "x",
    });
    crate::event_consumer::handle_event("message.created", &payload, &cm).await;

    assert!(next_json(&mut rx).is_none());
}

#[sqlx::test(migrations = "../migrations")]
async fn message_updated_broadcasts_to_channel_subscriber(pool: PgPool) {
    let cm = manager(pool).await;
    let user = Uuid::new_v4();
    let channel = Uuid::new_v4();

    let (conn_id, mut rx) = fake_conn(&cm, user);
    cm.join_channel(&conn_id, channel);

    let payload = json!({
        "message_id": Uuid::new_v4().to_string(),
        "channel_id": channel.to_string(),
        "content": "edited",
    });
    crate::event_consumer::handle_event("message.updated", &payload, &cm).await;

    assert_eq!(next_type(&mut rx).as_deref(), Some("message.updated"));
}

#[sqlx::test(migrations = "../migrations")]
async fn message_updated_not_delivered_to_non_subscriber(pool: PgPool) {
    let cm = manager(pool).await;
    let user = Uuid::new_v4();
    let channel = Uuid::new_v4();

    let (_conn_id, mut rx) = fake_conn(&cm, user);

    let payload = json!({
        "message_id": Uuid::new_v4().to_string(),
        "channel_id": channel.to_string(),
    });
    crate::event_consumer::handle_event("message.updated", &payload, &cm).await;

    assert!(next_json(&mut rx).is_none());
}

#[sqlx::test(migrations = "../migrations")]
async fn message_deleted_broadcasts_to_channel_subscriber(pool: PgPool) {
    let cm = manager(pool).await;
    let user = Uuid::new_v4();
    let channel = Uuid::new_v4();
    let message_id = Uuid::new_v4();

    let (conn_id, mut rx) = fake_conn(&cm, user);
    cm.join_channel(&conn_id, channel);

    let payload = json!({
        "message_id": message_id.to_string(),
        "channel_id": channel.to_string(),
    });
    crate::event_consumer::handle_event("message.deleted", &payload, &cm).await;

    let frame = next_json(&mut rx).expect("subscriber should receive a frame");
    assert_eq!(
        frame.get("type").and_then(|t| t.as_str()),
        Some("message.deleted")
    );
    assert_eq!(
        frame.get("message_id").and_then(|m| m.as_str()),
        Some(message_id.to_string().as_str())
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn message_pinned_broadcasts_to_channel_subscriber(pool: PgPool) {
    let cm = manager(pool).await;
    let user = Uuid::new_v4();
    let channel = Uuid::new_v4();

    let (conn_id, mut rx) = fake_conn(&cm, user);
    cm.join_channel(&conn_id, channel);

    let payload = json!({
        "message_id": Uuid::new_v4().to_string(),
        "channel_id": channel.to_string(),
        "pinned": true,
    });
    crate::event_consumer::handle_event("message.pinned", &payload, &cm).await;

    let frame = next_json(&mut rx).expect("subscriber should receive a frame");
    assert_eq!(
        frame.get("type").and_then(|t| t.as_str()),
        Some("message.pinned")
    );
    assert_eq!(frame.get("pinned").and_then(|p| p.as_bool()), Some(true));
}

#[sqlx::test(migrations = "../migrations")]
async fn reaction_added_broadcasts_to_channel_subscriber(pool: PgPool) {
    let cm = manager(pool).await;
    let user = Uuid::new_v4();
    let channel = Uuid::new_v4();
    let message_id = Uuid::new_v4();

    let (conn_id, mut rx) = fake_conn(&cm, user);
    cm.join_channel(&conn_id, channel);

    let payload = json!({
        "message_id": message_id.to_string(),
        "channel_id": channel.to_string(),
        "user_id": user.to_string(),
        "emoji": "thumbsup",
    });
    crate::event_consumer::handle_event("reaction.added", &payload, &cm).await;

    let frame = next_json(&mut rx).expect("subscriber should receive a frame");
    assert_eq!(
        frame.get("type").and_then(|t| t.as_str()),
        Some("reaction.added")
    );
    assert_eq!(
        frame
            .get("reaction")
            .and_then(|r| r.get("emoji"))
            .and_then(|e| e.as_str()),
        Some("thumbsup")
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn reaction_removed_broadcasts_to_channel_subscriber(pool: PgPool) {
    let cm = manager(pool).await;
    let user = Uuid::new_v4();
    let channel = Uuid::new_v4();

    let (conn_id, mut rx) = fake_conn(&cm, user);
    cm.join_channel(&conn_id, channel);

    let payload = json!({
        "message_id": Uuid::new_v4().to_string(),
        "channel_id": channel.to_string(),
        "user_id": user.to_string(),
        "emoji": "heart",
    });
    crate::event_consumer::handle_event("reaction.removed", &payload, &cm).await;

    let frame = next_json(&mut rx).expect("subscriber should receive a frame");
    assert_eq!(
        frame.get("type").and_then(|t| t.as_str()),
        Some("reaction.removed")
    );
    assert_eq!(frame.get("emoji").and_then(|e| e.as_str()), Some("heart"));
}

#[sqlx::test(migrations = "../migrations")]
async fn workspace_deleted_broadcasts_to_workspace_subscriber(pool: PgPool) {
    let cm = manager(pool).await;
    let user = Uuid::new_v4();
    let workspace = Uuid::new_v4();

    let (conn_id, mut rx) = fake_conn(&cm, user);
    cm.subscribe_workspace(&conn_id, workspace);

    let payload = json!({
        "workspace_id": workspace.to_string(),
        "delete_type": "soft",
    });
    crate::event_consumer::handle_event("workspace.deleted", &payload, &cm).await;

    let frame = next_json(&mut rx).expect("ws subscriber should receive a frame");
    assert_eq!(
        frame.get("type").and_then(|t| t.as_str()),
        Some("workspace.deleted")
    );
    assert_eq!(
        frame.get("delete_type").and_then(|d| d.as_str()),
        Some("soft")
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn workspace_deleted_not_delivered_to_other_workspace(pool: PgPool) {
    let cm = manager(pool).await;
    let user = Uuid::new_v4();

    let (conn_id, mut rx) = fake_conn(&cm, user);
    cm.subscribe_workspace(&conn_id, Uuid::new_v4());

    let payload = json!({ "workspace_id": Uuid::new_v4().to_string() });
    crate::event_consumer::handle_event("workspace.deleted", &payload, &cm).await;

    assert!(next_json(&mut rx).is_none());
}

#[sqlx::test(migrations = "../migrations")]
async fn workspace_restored_broadcasts_to_all_connections(pool: PgPool) {
    let cm = manager(pool).await;
    let workspace = Uuid::new_v4();

    let (_c1, mut rx1) = fake_conn(&cm, Uuid::new_v4());
    let (_c2, mut rx2) = fake_conn(&cm, Uuid::new_v4());

    let payload = json!({ "workspace_id": workspace.to_string() });
    crate::event_consumer::handle_event("workspace.restored", &payload, &cm).await;

    assert_eq!(next_type(&mut rx1).as_deref(), Some("workspace.restored"));
    assert_eq!(next_type(&mut rx2).as_deref(), Some("workspace.restored"));
}

#[sqlx::test(migrations = "../migrations")]
async fn notification_push_delivers_to_target_user(pool: PgPool) {
    let cm = manager(pool).await;
    let target = Uuid::new_v4();
    let other = Uuid::new_v4();

    let (_target_conn, mut target_rx) = fake_conn(&cm, target);
    let (_other_conn, mut other_rx) = fake_conn(&cm, other);

    let payload = json!({
        "user_id": target.to_string(),
        "channel_id": Uuid::new_v4().to_string(),
        "title": "New message",
        "body": "you've got mail",
        "priority": "normal",
    });
    crate::event_consumer::handle_event("notification.push", &payload, &cm).await;

    let frame = next_json(&mut target_rx).expect("target should receive notification");
    assert_eq!(
        frame.get("type").and_then(|t| t.as_str()),
        Some("notification")
    );
    assert_eq!(
        frame.get("title").and_then(|t| t.as_str()),
        Some("New message")
    );
    assert_eq!(
        frame.get("body").and_then(|b| b.as_str()),
        Some("you've got mail")
    );

    assert!(next_json(&mut other_rx).is_none());
}

#[sqlx::test(migrations = "../migrations")]
async fn presence_changed_fans_out_to_workspace_subscribers_only(pool: PgPool) {
    let cm = manager(pool).await;
    let subject = Uuid::new_v4();
    let observer = Uuid::new_v4();
    let outsider = Uuid::new_v4();
    let ws = Uuid::new_v4();

    let (_subject_conn, mut subject_rx) = fake_conn(&cm, subject);
    let (observer_conn, mut observer_rx) = fake_conn(&cm, observer);
    let (_outsider_conn, mut outsider_rx) = fake_conn(&cm, outsider);

    cm.subscribe_workspace(&observer_conn, ws);

    let payload = json!({
        "user_id": subject.to_string(),
        "status": "online",
        "workspace_ids": [ws.to_string()],
    });
    crate::event_consumer::handle_event("presence.changed", &payload, &cm).await;

    let frame =
        next_json(&mut observer_rx).expect("workspace subscriber should see presence change");
    assert_eq!(
        frame.get("type").and_then(|t| t.as_str()),
        Some("presence.changed")
    );
    assert_eq!(
        frame.get("user_id").and_then(|u| u.as_str()),
        Some(subject.to_string().as_str())
    );
    assert_eq!(frame.get("status").and_then(|s| s.as_str()), Some("online"));

    assert!(next_json(&mut subject_rx).is_none());
    assert!(
        next_json(&mut outsider_rx).is_none(),
        "a connection not subscribed to the subject's workspace must not see the presence change"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn typing_indicator_broadcasts_to_channel_subscriber(pool: PgPool) {
    let cm = manager(pool).await;
    let typer = Uuid::new_v4();
    let watcher = Uuid::new_v4();
    let channel = Uuid::new_v4();

    let (conn_id, mut rx) = fake_conn(&cm, watcher);
    cm.join_channel(&conn_id, channel);

    let payload = json!({
        "channel_id": channel.to_string(),
        "user_id": typer.to_string(),
        "is_typing": true,
    });
    crate::event_consumer::handle_event("typing.indicator", &payload, &cm).await;

    let frame = next_json(&mut rx).expect("channel subscriber should see typing");
    assert_eq!(
        frame.get("type").and_then(|t| t.as_str()),
        Some("typing.indicator")
    );
    assert_eq!(frame.get("is_typing").and_then(|t| t.as_bool()), Some(true));
    assert_eq!(
        frame.get("channel_id").and_then(|c| c.as_str()),
        Some(channel.to_string().as_str())
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn dm_created_delivers_dm_new_to_both_parties(pool: PgPool) {
    let cm = manager(pool).await;
    let from = Uuid::new_v4();
    let to = Uuid::new_v4();
    let bystander = Uuid::new_v4();

    let (_from_conn, mut from_rx) = fake_conn(&cm, from);
    let (_to_conn, mut to_rx) = fake_conn(&cm, to);
    let (_by_conn, mut by_rx) = fake_conn(&cm, bystander);

    let payload = json!({
        "from_user_id": from.to_string(),
        "to_user_id": to.to_string(),
        "content": "hi there",
    });
    crate::event_consumer::handle_event("dm.created", &payload, &cm).await;

    let from_frame = next_json(&mut from_rx).expect("sender should get dm.new");
    assert_eq!(
        from_frame.get("type").and_then(|t| t.as_str()),
        Some("dm.new")
    );
    assert_eq!(
        from_frame
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str()),
        Some("hi there")
    );

    assert_eq!(next_type(&mut to_rx).as_deref(), Some("dm.new"));

    assert!(next_json(&mut by_rx).is_none());
}

#[sqlx::test(migrations = "../migrations")]
async fn dm_created_with_missing_to_user_is_a_noop(pool: PgPool) {
    let cm = manager(pool).await;
    let from = Uuid::new_v4();

    let (_from_conn, mut from_rx) = fake_conn(&cm, from);

    let payload = json!({ "from_user_id": from.to_string(), "content": "x" });
    crate::event_consumer::handle_event("dm.created", &payload, &cm).await;

    assert!(next_json(&mut from_rx).is_none());
}

#[sqlx::test(migrations = "../migrations")]
async fn unknown_event_type_is_ignored(pool: PgPool) {
    let cm = manager(pool).await;
    let user = Uuid::new_v4();

    let (conn_id, mut rx) = fake_conn(&cm, user);
    cm.join_channel(&conn_id, Uuid::new_v4());
    cm.subscribe_workspace(&conn_id, Uuid::new_v4());

    let payload = json!({ "channel_id": Uuid::new_v4().to_string() });
    crate::event_consumer::handle_event("totally.unknown", &payload, &cm).await;

    assert!(next_json(&mut rx).is_none());
}
