use super::common::*;

use sqlx::PgPool;
use uuid::Uuid;

#[sqlx::test(migrations = "../migrations")]
async fn is_channel_member_true_for_member_false_for_outsider(pool: PgPool) {
    let cm = manager(pool).await;
    let (user, _ws, ch) = seed_member_in_channel(cm.db()).await;

    assert!(
        cm.is_channel_member(ch, user).await,
        "seeded channel member should be authorized"
    );

    let outsider = seed_user(cm.db()).await;
    assert!(
        !cm.is_channel_member(ch, outsider).await,
        "non-member must be denied"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn is_workspace_member_true_for_member_false_for_outsider(pool: PgPool) {
    let cm = manager(pool).await;
    let (user, ws, _ch) = seed_member_in_channel(cm.db()).await;

    assert!(
        cm.is_workspace_member(ws, user).await,
        "seeded workspace member should be authorized"
    );

    let outsider = seed_user(cm.db()).await;
    assert!(
        !cm.is_workspace_member(ws, outsider).await,
        "non-member must be denied"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn broadcast_to_channel_reaches_only_subscribed_conns(pool: PgPool) {
    let cm = manager(pool).await;
    let user = seed_user(cm.db()).await;
    let channel_id = Uuid::new_v4();

    let (joined_conn, mut rx_joined) = fake_conn(&cm, user);
    cm.join_channel(&joined_conn, channel_id);

    let (_other_conn, mut rx_other) = fake_conn(&cm, user);

    let frame = serde_json::json!({
        "type": "message.created",
        "channel_id": channel_id,
    })
    .to_string();

    cm.broadcast_to_channel(channel_id, &frame).await;

    let got = next_json(&mut rx_joined).expect("subscribed conn should receive the broadcast");
    assert_eq!(got["type"], "message.created");
    assert_eq!(got["channel_id"], channel_id.to_string());

    assert!(
        next_json(&mut rx_other).is_none(),
        "conn not joined to the channel must not receive the broadcast"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn broadcast_to_workspace_reaches_only_subscribed_conns(pool: PgPool) {
    let cm = manager(pool).await;
    let user = seed_user(cm.db()).await;
    let workspace_id = Uuid::new_v4();

    let (subbed_conn, mut rx_subbed) = fake_conn(&cm, user);
    cm.subscribe_workspace(&subbed_conn, workspace_id);

    let (_other_conn, mut rx_other) = fake_conn(&cm, user);

    let frame = serde_json::json!({
        "type": "channel.created",
        "workspace_id": workspace_id,
    })
    .to_string();

    cm.broadcast_to_workspace(workspace_id, &frame).await;

    let got = next_json(&mut rx_subbed).expect("workspace subscriber should receive the broadcast");
    assert_eq!(got["type"], "channel.created");
    assert_eq!(got["workspace_id"], workspace_id.to_string());

    assert!(
        next_json(&mut rx_other).is_none(),
        "conn not subscribed to the workspace must not receive the broadcast"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn send_to_user_reaches_that_users_connection(pool: PgPool) {
    let cm = manager(pool).await;
    let user = seed_user(cm.db()).await;
    let other = seed_user(cm.db()).await;

    let (_conn, mut rx_user) = fake_conn(&cm, user);
    let (_other_conn, mut rx_other) = fake_conn(&cm, other);

    let frame = serde_json::json!({
        "type": "dm.created",
        "user_id": user,
    })
    .to_string();

    cm.send_to_user(user, &frame).await;

    let got = next_json(&mut rx_user).expect("target user's conn should receive the message");
    assert_eq!(got["type"], "dm.created");
    assert_eq!(got["user_id"], user.to_string());

    assert!(
        next_json(&mut rx_other).is_none(),
        "send_to_user must not deliver to other users"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn send_to_user_with_no_connections_is_noop(pool: PgPool) {
    let cm = manager(pool).await;
    let absent = seed_user(cm.db()).await;
    cm.send_to_user(absent, "{\"type\":\"noop\"}").await;
}

#[sqlx::test(migrations = "../migrations")]
async fn leave_channel_stops_delivery(pool: PgPool) {
    let cm = manager(pool).await;
    let user = seed_user(cm.db()).await;
    let channel_id = Uuid::new_v4();

    let (conn, mut rx) = fake_conn(&cm, user);
    cm.join_channel(&conn, channel_id);

    cm.broadcast_to_channel(channel_id, "{\"type\":\"first\"}")
        .await;
    let first = next_json(&mut rx).expect("joined conn receives first frame");
    assert_eq!(first["type"], "first");

    cm.leave_channel(&conn, channel_id);
    cm.broadcast_to_channel(channel_id, "{\"type\":\"second\"}")
        .await;
    assert!(
        next_json(&mut rx).is_none(),
        "conn that left the channel must no longer receive broadcasts"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn remove_connection_drops_the_connection(pool: PgPool) {
    let cm = manager(pool).await;
    let user = seed_user(cm.db()).await;
    let channel_id = Uuid::new_v4();

    let (conn, mut rx) = fake_conn(&cm, user);
    cm.join_channel(&conn, channel_id);

    let (removed_user, was_last) = cm
        .remove_connection(&conn)
        .expect("removing a registered connection returns its owner");
    assert_eq!(removed_user, user);
    assert!(was_last, "the only connection was the user's last");

    cm.broadcast_to_channel(channel_id, "{\"type\":\"after-remove\"}")
        .await;
    assert!(
        next_json(&mut rx).is_none(),
        "removed connection must not receive broadcasts"
    );

    assert_eq!(
        cm.connection_count(),
        0,
        "no connections remain after removing the only one"
    );

    assert!(
        cm.remove_connection(&Uuid::new_v4()).is_none(),
        "removing an unknown connection returns None"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn connection_count_reflects_active_connections(pool: PgPool) {
    let cm = manager(pool).await;
    let user = seed_user(cm.db()).await;

    assert_eq!(cm.connection_count(), 0, "no connections at start");
    let (_conn, _rx) = fake_conn(&cm, user);
    assert_eq!(
        cm.connection_count(),
        1,
        "a connected user adds one active connection"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn presence_set_online_then_get_online_users_contains_user(pool: PgPool) {
    let cm = manager(pool).await;
    let user = seed_user(cm.db()).await;

    assert!(
        !cm.get_online_users().await.contains(&user),
        "freshly-seeded user should not be online yet"
    );

    cm.presence_set_online(user).await;
    assert!(
        cm.get_online_users().await.contains(&user),
        "user marked online must appear in get_online_users"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn online_users_in_workspace_excludes_non_members(pool: PgPool) {
    let cm = manager(pool).await;
    let member = seed_user(cm.db()).await;
    let stranger = seed_user(cm.db()).await;
    let ws = seed_workspace(cm.db(), member).await;
    add_ws_member(cm.db(), ws, member).await;

    cm.presence_set_online(member).await;
    cm.presence_set_online(stranger).await;

    let online = cm.online_users_in_workspace(ws).await;
    assert!(online.contains(&member), "workspace member must be listed");
    assert!(
        !online.contains(&stranger),
        "an online user who is not a workspace member must not leak into the roster"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn presence_clear_reports_offline_and_removes_user(pool: PgPool) {
    let cm = manager(pool).await;
    let user = seed_user(cm.db()).await;

    cm.presence_set_online(user).await;
    assert!(
        cm.get_online_users().await.contains(&user),
        "user must be online after presence_set_online"
    );

    let now_offline = cm.presence_clear(user).await;
    assert!(
        now_offline,
        "presence_clear must report true when no presence key remains for the user"
    );

    assert!(
        !cm.get_online_users().await.contains(&user),
        "user must not appear online after presence_clear"
    );
}
