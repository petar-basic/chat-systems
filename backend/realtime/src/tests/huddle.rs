use std::time::Duration;

use futures_util::StreamExt;
use serde_json::json;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;

fn inbound() -> crate::ws_handler::InboundState {
    crate::ws_handler::InboundState::new()
}

async fn collect_on(channel: &'static str, huddle_id: Uuid, window: Duration) -> Vec<Value> {
    let url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into());
    let client = redis::Client::open(url).expect("redis client");
    let mut pubsub = client.get_async_pubsub().await.expect("pubsub");
    pubsub.subscribe(channel).await.expect("subscribe");
    let mut stream = pubsub.into_on_message();

    let deadline = tokio::time::Instant::now() + window;
    let mut out = Vec::new();
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, stream.next()).await {
            Ok(Some(msg)) => {
                let Ok(payload) = msg.get_payload::<String>() else {
                    continue;
                };
                let Ok(env) = serde_json::from_str::<Value>(&payload) else {
                    continue;
                };
                let matches = env
                    .get("payload")
                    .and_then(|p| p.get("huddle_id"))
                    .and_then(|v| v.as_str())
                    == Some(&huddle_id.to_string());
                if matches {
                    out.push(env);
                }
            }
            _ => break,
        }
    }
    out
}

fn has_event(events: &[Value], event_type: &str) -> bool {
    events
        .iter()
        .any(|e| e.get("event_type").and_then(|t| t.as_str()) == Some(event_type))
}

#[test_macros::db_test(migrations = "../migrations")]
async fn huddle_offer_routes_to_signal_channel_not_lifecycle(pool: PgPool) {
    let cm = manager(pool).await;
    let a = seed_user(cm.db()).await;
    let b = seed_user(cm.db()).await;
    let huddle = Uuid::new_v4();

    cm.huddle_redis_join(huddle, a).await;
    cm.huddle_redis_join(huddle, b).await;

    let (conn_a, _rx) = fake_conn(&cm, a);

    let signal = tokio::spawn(collect_on(
        "events:huddle-signal",
        huddle,
        Duration::from_millis(300),
    ));
    let lifecycle = tokio::spawn(collect_on(
        "events:huddle",
        huddle,
        Duration::from_millis(300),
    ));
    settle().await;

    let offer = json!({
        "type": "huddle.offer",
        "huddle_id": huddle,
        "to_user_id": b,
        "sdp": { "type": "offer", "sdp": "v=0" },
    })
    .to_string();
    crate::ws_handler::handle_client_message(&offer, &conn_a, a, &cm, &mut inbound()).await;

    let on_signal = signal.await.expect("signal task");
    let on_lifecycle = lifecycle.await.expect("lifecycle task");

    assert!(
        has_event(&on_signal, "huddle.offer"),
        "offer must publish to events:huddle-signal, got: {on_signal:?}"
    );
    assert!(
        !has_event(&on_lifecycle, "huddle.offer"),
        "offer must NOT publish to events:huddle (API consumers skip signaling), got: {on_lifecycle:?}"
    );

    cm.huddle_redis_leave(huddle, a).await;
    cm.huddle_redis_leave(huddle, b).await;
}

#[test_macros::db_test(migrations = "../migrations")]
async fn huddle_member_joined_routes_to_lifecycle_channel(pool: PgPool) {
    let cm = manager(pool).await;
    let a = seed_user(cm.db()).await;
    let ws = seed_workspace(cm.db(), a).await;
    add_ws_member(cm.db(), ws, a).await;
    let b = seed_user(cm.db()).await;
    add_ws_member(cm.db(), ws, b).await;
    let huddle = seed_huddle(cm.db(), ws, None, a, Some(b)).await;

    let (conn_a, _rx) = fake_conn(&cm, a);

    let lifecycle = tokio::spawn(collect_on(
        "events:huddle",
        huddle,
        Duration::from_millis(300),
    ));
    let signal = tokio::spawn(collect_on(
        "events:huddle-signal",
        huddle,
        Duration::from_millis(300),
    ));
    settle().await;

    let join = json!({ "type": "huddle.join", "huddle_id": huddle }).to_string();
    crate::ws_handler::handle_client_message(&join, &conn_a, a, &cm, &mut inbound()).await;

    let on_lifecycle = lifecycle.await.expect("lifecycle task");
    let on_signal = signal.await.expect("signal task");

    assert!(
        has_event(&on_lifecycle, "huddle.member_joined"),
        "member_joined must publish to events:huddle, got: {on_lifecycle:?}"
    );
    assert!(
        !has_event(&on_signal, "huddle.member_joined"),
        "member_joined must NOT publish to events:huddle-signal, got: {on_signal:?}"
    );

    cm.huddle_redis_leave(huddle, a).await;
}

#[test_macros::db_test(migrations = "../migrations")]
async fn huddle_member_joined_broadcasts_to_huddle_subscribers(pool: PgPool) {
    let cm = manager(pool).await;
    let member = Uuid::new_v4();
    let huddle = Uuid::new_v4();

    let (conn_id, mut rx) = fake_conn(&cm, member);
    cm.join_huddle(&conn_id, huddle);

    let payload = json!({
        "huddle_id": huddle.to_string(),
        "user_id": Uuid::new_v4().to_string(),
    });
    crate::event_consumer::handle_event("huddle.member_joined", &payload, &cm).await;

    let frame = next_json(&mut rx).expect("huddle subscriber should receive a frame");
    assert_eq!(
        frame.get("type").and_then(|t| t.as_str()),
        Some("huddle.member_joined")
    );
    assert_eq!(
        frame.get("huddle_id").and_then(|v| v.as_str()),
        Some(huddle.to_string().as_str())
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn huddle_camera_broadcasts_to_huddle_subscribers(pool: PgPool) {
    let cm = manager(pool).await;
    let member = Uuid::new_v4();
    let huddle = Uuid::new_v4();

    let (conn_id, mut rx) = fake_conn(&cm, member);
    cm.join_huddle(&conn_id, huddle);

    let payload = json!({
        "huddle_id": huddle.to_string(),
        "user_id": Uuid::new_v4().to_string(),
        "camera_on": true,
    });
    crate::event_consumer::handle_event("huddle.camera", &payload, &cm).await;

    let frame = next_json(&mut rx).expect("huddle subscriber should receive a frame");
    assert_eq!(
        frame.get("type").and_then(|t| t.as_str()),
        Some("huddle.camera")
    );
    assert_eq!(
        frame.get("camera_on").and_then(serde_json::Value::as_bool),
        Some(true)
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn huddle_offer_relayed_only_to_target(pool: PgPool) {
    let cm = manager(pool).await;
    let from = Uuid::new_v4();
    let to = Uuid::new_v4();
    let other = Uuid::new_v4();
    let huddle = Uuid::new_v4();

    let (_conn_to, mut rx_to) = fake_conn(&cm, to);
    let (_conn_other, mut rx_other) = fake_conn(&cm, other);

    let payload = json!({
        "huddle_id": huddle.to_string(),
        "from_user_id": from.to_string(),
        "to_user_id": to.to_string(),
        "sdp": { "type": "offer", "sdp": "v=0" },
    });
    crate::event_consumer::handle_event("huddle.offer", &payload, &cm).await;

    let to_frame = next_json(&mut rx_to).expect("target should receive the offer");
    assert_eq!(
        to_frame.get("type").and_then(|t| t.as_str()),
        Some("huddle.offer")
    );
    assert_eq!(
        to_frame.get("from_user_id").and_then(|v| v.as_str()),
        Some(from.to_string().as_str())
    );
    assert!(
        drain_json(&mut rx_other).is_empty(),
        "a non-target user must not receive the directed offer"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_quiet_ring_is_not_rung(pool: PgPool) {
    let cm = manager(pool).await;
    let to = Uuid::new_v4();
    let (_conn, mut rx) = fake_conn(&cm, to);
    let ring = |quiet: bool| {
        json!({
            "huddle_id": Uuid::new_v4().to_string(),
            "workspace_id": Uuid::new_v4().to_string(),
            "from_user_id": Uuid::new_v4().to_string(),
            "to_user_id": to.to_string(),
            "quiet": quiet,
        })
    };

    crate::event_consumer::handle_event("huddle.ring", &ring(true), &cm).await;
    assert!(drain_json(&mut rx).is_empty(), "paused: no ring");

    crate::event_consumer::handle_event("huddle.ring", &ring(false), &cm).await;
    let frame = next_json(&mut rx).expect("not paused: it rings");
    assert_eq!(frame["type"], "huddle.ring");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn huddle_join_member_returns_snapshot_with_self(pool: PgPool) {
    let cm = manager(pool).await;
    let a = seed_user(cm.db()).await;
    let ws = seed_workspace(cm.db(), a).await;
    add_ws_member(cm.db(), ws, a).await;
    let b = seed_user(cm.db()).await;
    add_ws_member(cm.db(), ws, b).await;

    let huddle = seed_huddle(cm.db(), ws, None, a, Some(b)).await;
    let (conn_id, mut rx) = fake_conn(&cm, a);

    let text = json!({ "type": "huddle.join", "huddle_id": huddle }).to_string();
    crate::ws_handler::handle_client_message(&text, &conn_id, a, &cm, &mut inbound()).await;

    let frames = drain_json(&mut rx);
    let snapshot = frames
        .iter()
        .find(|f| f.get("type").and_then(|v| v.as_str()) == Some("huddle.members"))
        .expect("member join should return a huddle.members snapshot");
    let ids = snapshot
        .get("user_ids")
        .and_then(|v| v.as_array())
        .expect("snapshot has user_ids array");
    assert!(
        ids.iter()
            .any(|u| u.as_str() == Some(a.to_string().as_str())),
        "snapshot should include the joining user, got: {ids:?}"
    );

    cm.huddle_redis_leave(huddle, a).await;
}

#[test_macros::db_test(migrations = "../migrations")]
async fn huddle_join_non_member_denied_no_snapshot(pool: PgPool) {
    let cm = manager(pool).await;
    let owner = seed_user(cm.db()).await;
    let ws = seed_workspace(cm.db(), owner).await;
    add_ws_member(cm.db(), ws, owner).await;
    let outsider = seed_user(cm.db()).await;

    let huddle = seed_huddle(cm.db(), ws, None, owner, Some(outsider)).await;
    let (conn_id, mut rx) = fake_conn(&cm, outsider);

    let text = json!({ "type": "huddle.join", "huddle_id": huddle }).to_string();
    crate::ws_handler::handle_client_message(&text, &conn_id, outsider, &cm, &mut inbound()).await;

    let frames = drain_json(&mut rx);
    assert!(
        !frames
            .iter()
            .any(|f| f.get("type").and_then(|v| v.as_str()) == Some("huddle.members")),
        "non-member huddle.join must NOT return a snapshot, got: {frames:?}"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn full_huddle_turns_away_the_next_person_but_not_a_member(pool: PgPool) {
    let cm = manager(pool).await;
    let (first, ws, ch) = seed_member_in_channel(cm.db()).await;
    let huddle = seed_huddle(cm.db(), ws, Some(ch), first, None).await;

    cm.huddle_redis_join(huddle, first).await;
    for _ in 1..crate::connection_manager::HUDDLE_MAX_PARTICIPANTS {
        let member = seed_user(cm.db()).await;
        cm.huddle_redis_join(huddle, member).await;
    }

    let late = seed_user(cm.db()).await;
    add_ws_member(cm.db(), ws, late).await;
    add_ch_member(cm.db(), ch, late).await;
    let (late_conn, mut late_rx) = fake_conn(&cm, late);
    let join = json!({ "type": "huddle.join", "huddle_id": huddle }).to_string();

    let lifecycle = tokio::spawn(collect_on(
        "events:huddle",
        huddle,
        Duration::from_millis(300),
    ));
    settle().await;

    crate::ws_handler::handle_client_message(&join, &late_conn, late, &cm, &mut inbound()).await;
    let leave = json!({ "type": "huddle.leave", "huddle_id": huddle }).to_string();
    crate::ws_handler::handle_client_message(&leave, &late_conn, late, &cm, &mut inbound()).await;

    let frames = drain_json(&mut late_rx);
    let full = frames
        .iter()
        .find(|f| f.get("type").and_then(|v| v.as_str()) == Some("huddle.full"))
        .unwrap_or_else(|| {
            panic!("the ninth person should be told the huddle is full, got: {frames:?}")
        });
    assert_eq!(
        full.get("limit").and_then(|v| v.as_u64()),
        Some(crate::connection_manager::HUDDLE_MAX_PARTICIPANTS as u64)
    );
    assert!(!cm.huddle_redis_is_member(huddle, late).await);
    assert_eq!(
        cm.huddle_redis_members(huddle).await.len(),
        crate::connection_manager::HUDDLE_MAX_PARTICIPANTS
    );

    let events = lifecycle.await.expect("lifecycle task");
    assert!(
        !has_event(&events, "huddle.member_joined") && !has_event(&events, "huddle.member_left"),
        "a person turned away must not appear to join or leave, got: {events:?}"
    );

    let (first_conn, mut first_rx) = fake_conn(&cm, first);
    crate::ws_handler::handle_client_message(&join, &first_conn, first, &cm, &mut inbound()).await;
    let frames = drain_json(&mut first_rx);
    assert!(
        frames
            .iter()
            .any(|f| f.get("type").and_then(|v| v.as_str()) == Some("huddle.members")),
        "a member rejoining a full huddle keeps their place, got: {frames:?}"
    );

    for member in cm.huddle_redis_members(huddle).await {
        cm.huddle_redis_leave(huddle, member).await;
    }
}

async fn joined(
    cm: &std::sync::Arc<crate::connection_manager::ConnectionManager>,
    huddle: Uuid,
    user: Uuid,
    extra: Value,
) -> bool {
    let (conn_id, mut rx) = fake_conn(cm, user);
    let mut msg = json!({ "type": "huddle.join", "huddle_id": huddle });
    if let (Some(m), Some(e)) = (msg.as_object_mut(), extra.as_object()) {
        m.extend(e.clone());
    }
    crate::ws_handler::handle_client_message(&msg.to_string(), &conn_id, user, cm, &mut inbound())
        .await;
    let got = drain_json(&mut rx)
        .iter()
        .any(|f| f.get("type").and_then(|v| v.as_str()) == Some("huddle.members"));
    cm.huddle_redis_leave(huddle, user).await;
    got
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_channel_the_joiner_belongs_to_does_not_open_another_huddle(pool: PgPool) {
    let cm = manager(pool).await;
    let (owner, ws, private) = seed_member_in_channel(cm.db()).await;
    let huddle = seed_huddle(cm.db(), ws, Some(private), owner, None).await;

    let intruder = seed_user(cm.db()).await;
    add_ws_member(cm.db(), ws, intruder).await;
    let own = seed_channel(cm.db(), ws, intruder).await;
    add_ch_member(cm.db(), own, intruder).await;

    assert!(!joined(&cm, huddle, intruder, json!({ "channel_id": own })).await);
    assert!(
        !joined(
            &cm,
            huddle,
            intruder,
            json!({ "workspace_id": ws, "dm_partner_id": owner })
        )
        .await
    );
    assert!(joined(&cm, huddle, owner, json!({})).await);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_dm_huddle_admits_its_two_people_and_whoever_they_invite(pool: PgPool) {
    let cm = manager(pool).await;
    let a = seed_user(cm.db()).await;
    let ws = seed_workspace(cm.db(), a).await;
    add_ws_member(cm.db(), ws, a).await;
    let b = seed_user(cm.db()).await;
    add_ws_member(cm.db(), ws, b).await;
    let third = seed_user(cm.db()).await;
    add_ws_member(cm.db(), ws, third).await;
    let huddle = seed_huddle(cm.db(), ws, None, a, Some(b)).await;

    assert!(joined(&cm, huddle, b, json!({})).await);
    assert!(!joined(&cm, huddle, third, json!({})).await);

    invite_to_huddle(cm.db(), huddle, third, a).await;
    assert!(joined(&cm, huddle, third, json!({})).await);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_ended_huddle_admits_nobody(pool: PgPool) {
    let cm = manager(pool).await;
    let (owner, ws, ch) = seed_member_in_channel(cm.db()).await;
    let huddle = seed_huddle(cm.db(), ws, Some(ch), owner, None).await;
    sqlx::query("UPDATE huddle_sessions SET ended_at = NOW() WHERE id = $1")
        .bind(huddle)
        .execute(cm.db())
        .await
        .expect("end huddle");

    assert!(!joined(&cm, huddle, owner, json!({})).await);
}

async fn join_frames(
    cm: &std::sync::Arc<crate::connection_manager::ConnectionManager>,
    huddle: Uuid,
    user: Uuid,
) -> Vec<Value> {
    let (conn_id, mut rx) = fake_conn(cm, user);
    let msg = json!({ "type": "huddle.join", "huddle_id": huddle }).to_string();
    crate::ws_handler::handle_client_message(&msg, &conn_id, user, cm, &mut inbound()).await;
    drain_json(&mut rx)
}

fn snapshot_transport(frames: &[Value]) -> Option<&str> {
    frames
        .iter()
        .find(|f| f["type"] == "huddle.members")
        .and_then(|f| f["transport"].as_str())
}

async fn channel_member(pool: &PgPool, ws: Uuid, ch: Uuid) -> Uuid {
    let user = seed_user(pool).await;
    add_ws_member(pool, ws, user).await;
    add_ch_member(pool, ch, user).await;
    user
}

async fn empty(cm: &crate::connection_manager::ConnectionManager, huddle: Uuid) {
    for member in cm.huddle_redis_members(huddle).await {
        cm.huddle_redis_leave(huddle, member).await;
    }
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_join_past_the_threshold_moves_the_huddle_to_the_sfu_once(pool: PgPool) {
    let cm = manager_with_sfu(pool, 2, 50).await;
    let (first, ws, ch) = seed_member_in_channel(cm.db()).await;
    let huddle = seed_huddle(cm.db(), ws, Some(ch), first, None).await;
    let second = channel_member(cm.db(), ws, ch).await;
    let third = channel_member(cm.db(), ws, ch).await;
    let fourth = channel_member(cm.db(), ws, ch).await;

    let signal = tokio::spawn(collect_on(
        "events:huddle-signal",
        huddle,
        Duration::from_millis(800),
    ));
    settle().await;

    assert_eq!(
        snapshot_transport(&join_frames(&cm, huddle, first).await),
        Some("mesh")
    );
    assert_eq!(
        snapshot_transport(&join_frames(&cm, huddle, second).await),
        Some("mesh")
    );
    assert_eq!(
        snapshot_transport(&join_frames(&cm, huddle, third).await),
        Some("sfu")
    );
    assert_eq!(
        snapshot_transport(&join_frames(&cm, huddle, fourth).await),
        Some("sfu")
    );

    let moves: Vec<Value> = signal
        .await
        .expect("signal task")
        .into_iter()
        .filter(|e| e["event_type"] == "huddle.transport")
        .collect();
    assert_eq!(
        moves.len(),
        1,
        "the third person crosses the threshold of two, and only once: {moves:?}"
    );
    assert_eq!(moves[0]["payload"]["transport"], "sfu");

    empty(&cm, huddle).await;
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_huddle_at_the_threshold_stays_on_the_mesh(pool: PgPool) {
    let cm = manager_with_sfu(pool, 2, 50).await;
    let (first, ws, ch) = seed_member_in_channel(cm.db()).await;
    let huddle = seed_huddle(cm.db(), ws, Some(ch), first, None).await;
    let second = channel_member(cm.db(), ws, ch).await;

    let signal = tokio::spawn(collect_on(
        "events:huddle-signal",
        huddle,
        Duration::from_millis(300),
    ));
    settle().await;

    join_frames(&cm, huddle, first).await;
    let frames = join_frames(&cm, huddle, second).await;
    assert_eq!(snapshot_transport(&frames), Some("mesh"));

    let events = signal.await.expect("signal task");
    assert!(
        !has_event(&events, "huddle.transport"),
        "two people are not past a threshold of two, got: {events:?}"
    );

    empty(&cm, huddle).await;
}

#[test_macros::db_test(migrations = "../migrations")]
async fn without_an_sfu_a_huddle_never_moves(pool: PgPool) {
    let cm = manager(pool).await;
    let (first, ws, ch) = seed_member_in_channel(cm.db()).await;
    let huddle = seed_huddle(cm.db(), ws, Some(ch), first, None).await;

    let signal = tokio::spawn(collect_on(
        "events:huddle-signal",
        huddle,
        Duration::from_millis(800),
    ));
    settle().await;

    join_frames(&cm, huddle, first).await;
    for _ in 1..crate::connection_manager::HUDDLE_MAX_PARTICIPANTS {
        let member = channel_member(cm.db(), ws, ch).await;
        assert_eq!(
            snapshot_transport(&join_frames(&cm, huddle, member).await),
            Some("mesh")
        );
    }

    let events = signal.await.expect("signal task");
    assert!(!has_event(&events, "huddle.transport"), "got: {events:?}");

    empty(&cm, huddle).await;
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_huddle_on_the_sfu_holds_more_than_the_mesh_and_then_is_full(pool: PgPool) {
    let limit = crate::connection_manager::HUDDLE_MAX_PARTICIPANTS + 2;
    let cm = manager_with_sfu(pool, 1, limit).await;
    let (first, ws, ch) = seed_member_in_channel(cm.db()).await;
    let huddle = seed_huddle(cm.db(), ws, Some(ch), first, None).await;

    join_frames(&cm, huddle, first).await;
    for _ in 1..limit {
        let member = channel_member(cm.db(), ws, ch).await;
        assert_eq!(
            snapshot_transport(&join_frames(&cm, huddle, member).await),
            Some("sfu")
        );
    }
    assert_eq!(cm.huddle_redis_members(huddle).await.len(), limit);

    let late = channel_member(cm.db(), ws, ch).await;
    let frames = join_frames(&cm, huddle, late).await;
    let full = frames
        .iter()
        .find(|f| f["type"] == "huddle.full")
        .unwrap_or_else(|| panic!("past the SFU's limit the huddle is full, got: {frames:?}"));
    assert_eq!(full["limit"].as_u64(), Some(limit as u64));
    assert!(!cm.huddle_redis_is_member(huddle, late).await);

    empty(&cm, huddle).await;
}

#[test_macros::db_test(migrations = "../migrations")]
async fn huddle_transport_broadcasts_to_huddle_subscribers(pool: PgPool) {
    let cm = manager(pool).await;
    let member = Uuid::new_v4();
    let huddle = Uuid::new_v4();

    let (conn_id, mut rx) = fake_conn(&cm, member);
    cm.join_huddle(&conn_id, huddle);

    let payload = json!({ "huddle_id": huddle.to_string(), "transport": "sfu" });
    crate::event_consumer::handle_event("huddle.transport", &payload, &cm).await;

    let frame = next_json(&mut rx).expect("huddle subscriber should receive a frame");
    assert_eq!(frame["type"], "huddle.transport");
    assert_eq!(frame["transport"], "sfu");
}

fn frame_types(rx: &mut tokio::sync::mpsc::Receiver<axum::extract::ws::Message>) -> Vec<String> {
    drain_json(rx)
        .iter()
        .filter_map(|v| v.get("type").and_then(|t| t.as_str()).map(String::from))
        .collect()
}

#[test_macros::db_test(migrations = "../migrations")]
async fn someone_in_the_huddle_from_outside_hears_its_thread_and_nothing_else(pool: PgPool) {
    let cm = manager(pool).await;
    let huddle = Uuid::new_v4();
    let channel = Uuid::new_v4();
    let root = Uuid::new_v4();
    let thread = crate::connection_manager::HuddleThread {
        channel_id: channel,
        root_id: root,
    };
    let (outsider, mut outsider_rx) = fake_conn(&cm, Uuid::new_v4());
    let (member, mut member_rx) = fake_conn(&cm, Uuid::new_v4());
    cm.join_channel(&member, channel);
    for conn in [outsider, member] {
        cm.join_huddle(&conn, huddle);
        cm.hear_huddle_thread(&conn, huddle, thread);
    }

    let reply = Uuid::new_v4();
    let elsewhere = Uuid::new_v4();
    let events = [
        (
            "message.created",
            json!({ "id": reply, "channel_id": channel, "thread_parent_id": root }),
        ),
        (
            "message.created",
            json!({ "id": elsewhere, "channel_id": channel, "thread_parent_id": null }),
        ),
        (
            "message.updated",
            json!({ "id": root, "channel_id": channel, "thread_parent_id": null }),
        ),
        (
            "reaction.added",
            json!({ "id": Uuid::new_v4(), "message_id": reply, "channel_id": channel,
                    "thread_parent_id": root, "user_id": Uuid::new_v4(), "emoji": "👍" }),
        ),
        (
            "reaction.added",
            json!({ "id": Uuid::new_v4(), "message_id": elsewhere, "channel_id": channel,
                    "thread_parent_id": null, "user_id": Uuid::new_v4(), "emoji": "👍" }),
        ),
        (
            "message.deleted",
            json!({ "message_id": reply, "channel_id": channel, "thread_parent_id": root }),
        ),
        (
            "message.created",
            json!({ "id": Uuid::new_v4(), "channel_id": Uuid::new_v4(), "thread_parent_id": root }),
        ),
    ];
    for (event_type, payload) in &events {
        crate::event_consumer::handle_event(event_type, payload, &cm).await;
    }

    assert_eq!(
        frame_types(&mut outsider_rx),
        [
            "message.new",
            "message.updated",
            "reaction.added",
            "message.deleted"
        ]
    );
    assert_eq!(
        frame_types(&mut member_rx),
        [
            "message.new",
            "message.new",
            "message.updated",
            "reaction.added",
            "reaction.added",
            "message.deleted"
        ],
        "a member hears the channel once, thread or not"
    );

    cm.leave_huddle(&outsider, huddle);
    crate::event_consumer::handle_event("message.created", &events[0].1, &cm).await;
    assert!(frame_types(&mut outsider_rx).is_empty());
}

async fn send(
    cm: &std::sync::Arc<crate::connection_manager::ConnectionManager>,
    conn_id: &Uuid,
    user: Uuid,
    msg: Value,
) {
    crate::ws_handler::handle_client_message(&msg.to_string(), conn_id, user, cm, &mut inbound())
        .await;
}

fn stroke(huddle: Uuid, sharer: Uuid, stroke: u32) -> Value {
    json!({
        "type": "huddle.draw",
        "huddle_id": huddle,
        "sharer_id": sharer,
        "stroke": stroke,
        "points": [[0.25, 0.5], [0.3, 0.55]],
        "done": false,
    })
}

fn drawn_strokes(events: &[Value]) -> Vec<u64> {
    events
        .iter()
        .filter(|e| e["event_type"] == "huddle.draw")
        .filter_map(|e| e["payload"]["stroke"].as_u64())
        .collect()
}

#[test_macros::db_test(migrations = "../migrations")]
async fn only_a_member_draws_and_only_on_a_screen_being_shared(pool: PgPool) {
    let cm = manager(pool).await;
    let sharer = seed_user(cm.db()).await;
    let viewer = seed_user(cm.db()).await;
    let outsider = seed_user(cm.db()).await;
    let huddle = Uuid::new_v4();
    cm.huddle_redis_join(huddle, sharer).await;
    cm.huddle_redis_join(huddle, viewer).await;
    let (sharer_conn, _sharer_rx) = fake_conn(&cm, sharer);
    let (viewer_conn, _viewer_rx) = fake_conn(&cm, viewer);
    let (outsider_conn, _outsider_rx) = fake_conn(&cm, outsider);

    let signal = tokio::spawn(collect_on(
        "events:huddle-signal",
        huddle,
        Duration::from_millis(400),
    ));
    settle().await;

    send(&cm, &viewer_conn, viewer, stroke(huddle, sharer, 1)).await;
    send(
        &cm,
        &sharer_conn,
        sharer,
        json!({ "type": "huddle.screenshare", "huddle_id": huddle, "sharing": true }),
    )
    .await;
    send(&cm, &viewer_conn, viewer, stroke(huddle, sharer, 2)).await;
    send(&cm, &outsider_conn, outsider, stroke(huddle, sharer, 3)).await;
    send(&cm, &viewer_conn, viewer, stroke(huddle, viewer, 4)).await;
    let mut too_long = stroke(huddle, sharer, 5);
    too_long["points"] = json!(vec![[0.5, 0.5]; 65]);
    send(&cm, &viewer_conn, viewer, too_long).await;
    let mut off_screen = stroke(huddle, sharer, 6);
    off_screen["points"] = json!([[1.5, 0.5]]);
    send(&cm, &viewer_conn, viewer, off_screen).await;
    let mut lifted = stroke(huddle, sharer, 7);
    lifted["points"] = json!([]);
    lifted["done"] = json!(true);
    send(&cm, &viewer_conn, viewer, lifted).await;

    let events = signal.await.expect("signal task");
    assert_eq!(drawn_strokes(&events), [2, 7]);
    let relayed = events
        .iter()
        .find(|e| e["event_type"] == "huddle.draw")
        .expect("a relayed stroke");
    assert_eq!(relayed["payload"]["user_id"], json!(viewer));
    assert_eq!(
        relayed["payload"]["points"],
        json!([[0.25, 0.5], [0.3, 0.55]])
    );

    empty(&cm, huddle).await;
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_sharer_turning_drawing_off_drops_strokes_until_it_is_back_on(pool: PgPool) {
    let cm = manager(pool).await;
    let sharer = seed_user(cm.db()).await;
    let viewer = seed_user(cm.db()).await;
    let huddle = Uuid::new_v4();
    cm.huddle_redis_join(huddle, sharer).await;
    cm.huddle_redis_join(huddle, viewer).await;
    let (sharer_conn, _sharer_rx) = fake_conn(&cm, sharer);
    let (viewer_conn, _viewer_rx) = fake_conn(&cm, viewer);
    let share = |sharing: bool| json!({ "type": "huddle.screenshare", "huddle_id": huddle, "sharing": sharing });
    let drawing = |enabled: bool| json!({ "type": "huddle.drawing", "huddle_id": huddle, "enabled": enabled });

    let signal = tokio::spawn(collect_on(
        "events:huddle-signal",
        huddle,
        Duration::from_millis(500),
    ));
    settle().await;

    send(&cm, &viewer_conn, viewer, drawing(false)).await;
    send(&cm, &sharer_conn, sharer, share(true)).await;
    send(&cm, &sharer_conn, sharer, drawing(false)).await;
    send(&cm, &viewer_conn, viewer, stroke(huddle, sharer, 1)).await;
    send(&cm, &sharer_conn, sharer, share(true)).await;
    send(&cm, &viewer_conn, viewer, stroke(huddle, sharer, 2)).await;
    send(&cm, &sharer_conn, sharer, drawing(true)).await;
    send(&cm, &viewer_conn, viewer, stroke(huddle, sharer, 3)).await;
    send(&cm, &sharer_conn, sharer, drawing(false)).await;
    send(&cm, &sharer_conn, sharer, share(false)).await;
    send(&cm, &sharer_conn, sharer, share(true)).await;
    send(&cm, &viewer_conn, viewer, stroke(huddle, sharer, 4)).await;
    cm.huddle_redis_leave(huddle, sharer).await;
    cm.huddle_redis_join(huddle, sharer).await;
    send(&cm, &viewer_conn, viewer, stroke(huddle, sharer, 5)).await;

    let events = signal.await.expect("signal task");
    assert_eq!(
        drawn_strokes(&events),
        [3, 4],
        "a re-announced share keeps drawing off; a new share starts with it on"
    );
    let switches: Vec<(Value, Value)> = events
        .iter()
        .filter(|e| e["event_type"] == "huddle.drawing")
        .map(|e| {
            (
                e["payload"]["user_id"].clone(),
                e["payload"]["enabled"].clone(),
            )
        })
        .collect();
    assert_eq!(
        switches,
        [
            (json!(sharer), json!(false)),
            (json!(sharer), json!(true)),
            (json!(sharer), json!(false)),
        ],
        "only someone sharing a screen switches its drawing"
    );

    empty(&cm, huddle).await;
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_flood_of_strokes_is_cut_to_the_per_person_rate_without_closing_the_socket(pool: PgPool) {
    use crate::connection_manager::{DRAW_BURST, DRAW_PER_SEC};
    let cm = manager(pool).await;
    let sharer = seed_user(cm.db()).await;
    let viewer = seed_user(cm.db()).await;
    let huddle = Uuid::new_v4();
    cm.huddle_redis_join(huddle, sharer).await;
    cm.huddle_redis_join(huddle, viewer).await;
    let (sharer_conn, _sharer_rx) = fake_conn(&cm, sharer);
    let (viewer_conn, _viewer_rx) = fake_conn(&cm, viewer);
    let (second_conn, _second_rx) = fake_conn(&cm, viewer);
    send(
        &cm,
        &sharer_conn,
        sharer,
        json!({ "type": "huddle.screenshare", "huddle_id": huddle, "sharing": true }),
    )
    .await;

    let signal = tokio::spawn(collect_on(
        "events:huddle-signal",
        huddle,
        Duration::from_millis(1_500),
    ));
    settle().await;

    let sent = 200u32;
    let mut inbound = inbound();
    let started = std::time::Instant::now();
    for n in 0..sent {
        let conn = if n % 2 == 0 {
            &viewer_conn
        } else {
            &second_conn
        };
        let handled = crate::ws_handler::handle_client_message(
            &stroke(huddle, sharer, n).to_string(),
            conn,
            viewer,
            &cm,
            &mut inbound,
        )
        .await;
        assert_eq!(handled, crate::ws_handler::Inbound::Handled);
    }
    let elapsed = started.elapsed().as_secs_f32();

    let relayed = drawn_strokes(&signal.await.expect("signal task")).len() as f32;
    assert!(relayed >= DRAW_BURST, "the burst passes, got {relayed}");
    assert!(
        relayed <= DRAW_BURST + elapsed * DRAW_PER_SEC + 1.0,
        "{relayed} strokes in {elapsed}s is past the per-person rate"
    );

    empty(&cm, huddle).await;
}
