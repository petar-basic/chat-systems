use super::common::*;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

#[sqlx::test(migrations = "../migrations")]
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

#[sqlx::test(migrations = "../migrations")]
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
    assert_eq!(frame.get("camera_on").and_then(|v| v.as_bool()), Some(true));
}

#[sqlx::test(migrations = "../migrations")]
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

#[sqlx::test(migrations = "../migrations")]
async fn huddle_join_member_returns_snapshot_with_self(pool: PgPool) {
    let cm = manager(pool).await;
    let a = seed_user(cm.db()).await;
    let ws = seed_workspace(cm.db(), a).await;
    add_ws_member(cm.db(), ws, a).await;
    let b = seed_user(cm.db()).await;
    add_ws_member(cm.db(), ws, b).await;

    let huddle = Uuid::new_v4();
    let (conn_id, mut rx) = fake_conn(&cm, a);

    let text = json!({
        "type": "huddle.join",
        "huddle_id": huddle,
        "workspace_id": ws,
        "dm_partner_id": b,
    })
    .to_string();
    crate::ws_handler::handle_client_message(&text, &conn_id, a, &cm).await;

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

#[sqlx::test(migrations = "../migrations")]
async fn huddle_join_non_member_denied_no_snapshot(pool: PgPool) {
    let cm = manager(pool).await;
    let owner = seed_user(cm.db()).await;
    let ws = seed_workspace(cm.db(), owner).await;
    add_ws_member(cm.db(), ws, owner).await;
    let outsider = seed_user(cm.db()).await;

    let huddle = Uuid::new_v4();
    let (conn_id, mut rx) = fake_conn(&cm, outsider);

    let text = json!({
        "type": "huddle.join",
        "huddle_id": huddle,
        "workspace_id": ws,
        "dm_partner_id": owner,
    })
    .to_string();
    crate::ws_handler::handle_client_message(&text, &conn_id, outsider, &cm).await;

    let frames = drain_json(&mut rx);
    assert!(
        !frames
            .iter()
            .any(|f| f.get("type").and_then(|v| v.as_str()) == Some("huddle.members")),
        "non-member huddle.join must NOT return a snapshot, got: {frames:?}"
    );
}
