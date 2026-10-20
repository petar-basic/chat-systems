use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;
use crate::notifications::consumer::notify_mentions;
use crate::push::sender::{PushSender, VapidKeys};
use crate::state::AppState;

/// RFC 8291's example subscription keys. They are a published test vector, not a
/// secret, and they are a real point on the curve -- an invented `p256dh` fails
/// in the encryption step rather than in the assertion.
const UA_PUBLIC: &str =
    "BCVxsr7N_eNgVRqvHtD0zTZsEc6-VV-JvLexhqUzORcxaOzi6-AYWXvTBHm4bjyPjs7Vd8pZGH6SRpkNtoIAiw4";
const UA_AUTH: &str = "BTBZMqHH6r4Tts7J_aSIgg";
const TEST_VAPID_PRIVATE: &str = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8";

struct FakePushService {
    address: String,
    hits: Arc<AtomicUsize>,
}

/// A push service that records what reached it. The alternative -- asserting on
/// a mock of our own sender -- would prove the code called itself.
async fn fake_push_service(status: StatusCode) -> FakePushService {
    let hits = Arc::new(AtomicUsize::new(0));
    let counter = hits.clone();

    let app = axum::Router::new().fallback(move || {
        let counter = counter.clone();
        async move {
            counter.fetch_add(1, Ordering::SeqCst);
            status
        }
    });

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind a port for the fake push service");
    let address = format!("http://{}", listener.local_addr().expect("local addr"));
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    FakePushService { address, hits }
}

fn sender_for(state: &AppState) -> PushSender {
    PushSender::new(
        state.push_repo.clone(),
        VapidKeys {
            public_key: "test-public".into(),
            private_key: TEST_VAPID_PRIVATE.into(),
            subject: "mailto:ops@test.local".into(),
        },
    )
}

async fn subscribe(state: &AppState, user_id: Uuid, endpoint: &str) {
    state
        .push_repo
        .upsert(user_id, endpoint, UA_PUBLIC, UA_AUTH, Some("Firefox"))
        .await
        .expect("store a subscription");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_browser_registers_once_however_often_it_re_subscribes(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_id, _email, token) = seed_and_login(&app, &state, "push", false).await;

    let body = json!({
        "endpoint": "https://push.example.test/abc",
        "keys": { "p256dh": UA_PUBLIC, "auth": UA_AUTH },
        "user_agent": "Firefox"
    });

    let (status, first) = send(
        &app,
        "POST",
        "/api/push/subscriptions",
        Some(&token),
        Some(body.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{first:?}");

    let (status, again) = send(
        &app,
        "POST",
        "/api/push/subscriptions",
        Some(&token),
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        again["id"], first["id"],
        "the same endpoint is the same device, not a second one"
    );

    let (status, listed) = send(
        &app,
        "GET",
        "/api/push/subscriptions/list",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed["data"].as_array().expect("data").len(), 1);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_subscription_needs_a_real_endpoint_and_a_session(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_id, _email, token) = seed_and_login(&app, &state, "push", false).await;

    let (status, _) = send(
        &app,
        "POST",
        "/api/push/subscriptions",
        Some(&token),
        Some(json!({ "endpoint": "http://push.example.test/abc", "keys": { "p256dh": UA_PUBLIC, "auth": UA_AUTH } })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "push endpoints are https"
    );

    let (status, _) = send(
        &app,
        "POST",
        "/api/push/subscriptions",
        Some(&token),
        Some(json!({ "endpoint": "https://push.example.test/abc", "keys": { "p256dh": "", "auth": "" } })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (status, _) = send(
        &app,
        "POST",
        "/api/push/subscriptions",
        None,
        Some(json!({ "endpoint": "https://push.example.test/abc", "keys": { "p256dh": UA_PUBLIC, "auth": UA_AUTH } })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn unsubscribing_only_removes_your_own_device(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (mine, _, my_token) = seed_and_login(&app, &state, "push-mine", false).await;
    let (theirs, _, _) = seed_and_login(&app, &state, "push-theirs", false).await;

    subscribe(&state, mine, "https://push.example.test/mine").await;
    subscribe(&state, theirs, "https://push.example.test/theirs").await;

    let (status, _) = send(
        &app,
        "DELETE",
        "/api/push/subscriptions",
        Some(&my_token),
        Some(json!({ "endpoint": "https://push.example.test/theirs" })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "deleting somebody else's is a no-op"
    );
    assert_eq!(
        state
            .push_repo
            .list_for_user(theirs)
            .await
            .expect("list")
            .len(),
        1,
        "their device is still registered"
    );

    let (status, _) = send(
        &app,
        "DELETE",
        "/api/push/subscriptions",
        Some(&my_token),
        Some(json!({ "endpoint": "https://push.example.test/mine" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(state
        .push_repo
        .list_for_user(mine)
        .await
        .expect("list")
        .is_empty());
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_mention_reaches_every_registered_device(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (author, _, author_token) = seed_and_login(&app, &state, "push-author", false).await;
    let (target, _, _) = seed_and_login(&app, &state, "push-target", false).await;
    let ws_id = seed_workspace(&state, author, "Push WS").await;
    add_ws_member(&state, ws_id, target, "member").await;
    let ch_id = seed_channel(&state, ws_id, author, "pushes", false).await;
    let _ = author_token;

    let service = fake_push_service(StatusCode::CREATED).await;
    subscribe(&state, target, &format!("{}/push/laptop", service.address)).await;
    subscribe(&state, target, &format!("{}/push/phone", service.address)).await;

    let mut conn = state.redis.clone();
    notify_mentions(
        &state,
        &state.notification_repo,
        &sender_for(&state),
        &mut conn,
        &json!({
            "user_id": author.to_string(),
            "workspace_id": ws_id.to_string(),
            "channel_id": ch_id.to_string(),
            "id": Uuid::new_v4().to_string(),
            "content": "look at this @target",
            "mentioned_user_ids": [target.to_string()],
        }),
    )
    .await;

    assert_eq!(
        service.hits.load(Ordering::SeqCst),
        2,
        "one request per device, not one per person"
    );
    let stored = state.push_repo.list_for_user(target).await.expect("list");
    assert!(
        stored.iter().all(|s| s.last_used_at.is_some()),
        "a delivered device records that it was used"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn nothing_is_pushed_under_dnd_a_mute_or_a_live_socket(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (author, _, _) = seed_and_login(&app, &state, "push-author", false).await;
    let (target, _, _) = seed_and_login(&app, &state, "push-target", false).await;
    let ws_id = seed_workspace(&state, author, "Quiet WS").await;
    add_ws_member(&state, ws_id, target, "member").await;
    let ch_id = seed_channel(&state, ws_id, author, "quiet", false).await;
    state
        .workspace_service
        .repo
        .add_channel_member(
            ch_id,
            target,
            &crate::workspace::models::ChannelRole::Member,
        )
        .await
        .expect("add the target to the channel");

    let service = fake_push_service(StatusCode::CREATED).await;
    subscribe(&state, target, &format!("{}/push/laptop", service.address)).await;

    let event = json!({
        "user_id": author.to_string(),
        "workspace_id": ws_id.to_string(),
        "channel_id": ch_id.to_string(),
        "id": Uuid::new_v4().to_string(),
        "content": "quiet please @target",
        "mentioned_user_ids": [target.to_string()],
    });
    let mut conn = state.redis.clone();

    sqlx::query("UPDATE channel_members SET muted = TRUE WHERE channel_id = $1 AND user_id = $2")
        .bind(ch_id)
        .bind(target)
        .execute(&state.pool)
        .await
        .expect("mute the channel");
    notify_mentions(
        &state,
        &state.notification_repo,
        &sender_for(&state),
        &mut conn,
        &event,
    )
    .await;
    assert_eq!(
        service.hits.load(Ordering::SeqCst),
        0,
        "a muted channel is silent"
    );

    sqlx::query("UPDATE channel_members SET muted = FALSE WHERE channel_id = $1 AND user_id = $2")
        .bind(ch_id)
        .bind(target)
        .execute(&state.pool)
        .await
        .expect("unmute");
    state
        .notification_repo
        .set_dnd(
            target,
            Some(chrono::Utc::now() + chrono::Duration::hours(1)),
        )
        .await
        .expect("set dnd");
    notify_mentions(
        &state,
        &state.notification_repo,
        &sender_for(&state),
        &mut conn,
        &event,
    )
    .await;
    assert_eq!(
        service.hits.load(Ordering::SeqCst),
        0,
        "do not disturb means do not disturb"
    );

    state
        .notification_repo
        .set_dnd(target, None)
        .await
        .expect("clear dnd");
    let expires_at = chrono::Utc::now().timestamp() + 60;
    let _: () = redis::AsyncCommands::zadd(
        &mut conn,
        format!("presence:ws:{ws_id}"),
        target.to_string(),
        expires_at,
    )
    .await
    .expect("mark them online");
    notify_mentions(
        &state,
        &state.notification_repo,
        &sender_for(&state),
        &mut conn,
        &event,
    )
    .await;
    assert_eq!(
        service.hits.load(Ordering::SeqCst),
        0,
        "somebody looking at the message does not need it on their phone too"
    );

    let _: () = redis::AsyncCommands::del(&mut conn, format!("presence:ws:{ws_id}"))
        .await
        .expect("clear presence");
    notify_mentions(
        &state,
        &state.notification_repo,
        &sender_for(&state),
        &mut conn,
        &event,
    )
    .await;
    assert_eq!(
        service.hits.load(Ordering::SeqCst),
        1,
        "with nothing suppressing it, it goes out"
    );
}

/// `410 Gone` is the only reliable signal that a subscription is dead, and a
/// dead subscription that is never pruned is a request per notification forever.
#[test_macros::db_test(migrations = "../migrations")]
async fn a_subscription_the_service_calls_gone_is_dropped(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_author, _, _) = seed_and_login(&app, &state, "push-author", false).await;
    let (target, _, _) = seed_and_login(&app, &state, "push-target", false).await;

    let service = fake_push_service(StatusCode::GONE).await;
    subscribe(&state, target, &format!("{}/push/stale", service.address)).await;

    sender_for(&state)
        .send_to_user(
            target,
            &crate::push::sender::PushPayload {
                title: "You were mentioned".into(),
                body: "hello".into(),
                workspace_id: None,
                channel_id: None,
                message_id: None,
                badge_count: 1,
            },
        )
        .await;

    assert_eq!(service.hits.load(Ordering::SeqCst), 1);
    assert!(
        state
            .push_repo
            .list_for_user(target)
            .await
            .expect("list")
            .is_empty(),
        "the dead subscription is gone"
    );
}

/// The last branch: no socket, no device that could be woken. Before this, the
/// mention waited until they next opened the app.
#[sqlx::test(migrations = "../migrations")]
async fn a_mention_that_reached_nobody_is_queued_for_email(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (author, _, _) = seed_and_login(&app, &state, "mail-author", false).await;
    let (target, _, _) = seed_and_login(&app, &state, "mail-target", false).await;
    let ws_id = seed_workspace(&state, author, "Mail WS").await;
    add_ws_member(&state, ws_id, target, "member").await;
    let ch_id = seed_channel(&state, ws_id, author, "quiet", false).await;

    let mut conn = state.redis.clone();
    let event = json!({
        "user_id": author.to_string(),
        "workspace_id": ws_id.to_string(),
        "channel_id": ch_id.to_string(),
        "id": Uuid::new_v4().to_string(),
        "content": "are you there @target",
        "mentioned_user_ids": [target.to_string()],
    });

    notify_mentions(
        &state,
        &state.notification_repo,
        &sender_for(&state),
        &mut conn,
        &event,
    )
    .await;

    let queued: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pending_mention_emails WHERE user_id = $1 AND workspace_id = $2",
    )
    .bind(target)
    .bind(ws_id)
    .fetch_one(&state.pool)
    .await
    .expect("count");
    assert_eq!(
        queued, 1,
        "with nothing else able to reach them, an email is queued"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn nothing_is_queued_for_somebody_a_push_reached(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (author, _, _) = seed_and_login(&app, &state, "mail-author", false).await;
    let (target, _, _) = seed_and_login(&app, &state, "mail-target", false).await;
    let ws_id = seed_workspace(&state, author, "Mail WS").await;
    add_ws_member(&state, ws_id, target, "member").await;
    let ch_id = seed_channel(&state, ws_id, author, "quiet", false).await;

    let service = fake_push_service(StatusCode::CREATED).await;
    subscribe(&state, target, &format!("{}/push/laptop", service.address)).await;

    let mut conn = state.redis.clone();
    notify_mentions(
        &state,
        &state.notification_repo,
        &sender_for(&state),
        &mut conn,
        &json!({
            "user_id": author.to_string(),
            "workspace_id": ws_id.to_string(),
            "channel_id": ch_id.to_string(),
            "id": Uuid::new_v4().to_string(),
            "content": "ping @target",
            "mentioned_user_ids": [target.to_string()],
        }),
    )
    .await;

    assert_eq!(service.hits.load(Ordering::SeqCst), 1, "the push went out");
    let queued: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM pending_mention_emails WHERE user_id = $1")
            .bind(target)
            .fetch_one(&state.pool)
            .await
            .expect("count");
    assert_eq!(queued, 0, "so there is nothing to email about");
}

#[sqlx::test(migrations = "../migrations")]
async fn somebody_who_turned_mention_emails_off_is_not_queued(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (author, _, _) = seed_and_login(&app, &state, "mail-author", false).await;
    let (target, _, target_token) = seed_and_login(&app, &state, "mail-target", false).await;
    let ws_id = seed_workspace(&state, author, "Mail WS").await;
    add_ws_member(&state, ws_id, target, "member").await;
    let ch_id = seed_channel(&state, ws_id, author, "quiet", false).await;

    let (status, body) = send(
        &app,
        "GET",
        "/api/notifications/email",
        Some(&target_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["mention_emails"], json!(true), "on by default");

    let (status, _) = send(
        &app,
        "PATCH",
        "/api/notifications/email",
        Some(&target_token),
        Some(json!({ "mention_emails": false })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let mut conn = state.redis.clone();
    notify_mentions(
        &state,
        &state.notification_repo,
        &sender_for(&state),
        &mut conn,
        &json!({
            "user_id": author.to_string(),
            "workspace_id": ws_id.to_string(),
            "channel_id": ch_id.to_string(),
            "id": Uuid::new_v4().to_string(),
            "content": "hello @target",
            "mentioned_user_ids": [target.to_string()],
        }),
    )
    .await;

    let queued: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM pending_mention_emails WHERE user_id = $1")
            .bind(target)
            .fetch_one(&state.pool)
            .await
            .expect("count");
    assert_eq!(queued, 0);
}

/// Coming back inside the digest window cancels it: the badge is already
/// waiting for them, so the email would be telling them what they can see.
#[sqlx::test(migrations = "../migrations")]
async fn coming_online_inside_the_window_cancels_the_email(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (author, _, _) = seed_and_login(&app, &state, "mail-author", false).await;
    let (target, _, _) = seed_and_login(&app, &state, "mail-target", false).await;
    let ws_id = seed_workspace(&state, author, "Mail WS").await;
    add_ws_member(&state, ws_id, target, "member").await;

    sqlx::query(
        "INSERT INTO pending_mention_emails \
           (user_id, workspace_id, sender_name, channel_name, created_at) \
         VALUES ($1, $2, 'Ana', 'general', NOW() - INTERVAL '10 minutes')",
    )
    .bind(target)
    .bind(ws_id)
    .execute(&state.pool)
    .await
    .expect("queue a due digest");

    let mut conn = state.redis.clone();
    let expires_at = chrono::Utc::now().timestamp() + 60;
    let _: () = redis::AsyncCommands::zadd(
        &mut conn,
        format!("presence:ws:{ws_id}"),
        target.to_string(),
        expires_at,
    )
    .await
    .expect("mark them online");

    let sent = crate::notifications::email::flush_due(&state).await;
    assert_eq!(sent, 0, "they are looking at it");

    let left: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM pending_mention_emails WHERE user_id = $1")
            .bind(target)
            .fetch_one(&state.pool)
            .await
            .expect("count");
    assert_eq!(
        left, 0,
        "and the queue is cleared rather than retried forever"
    );

    let _: () = redis::AsyncCommands::del(&mut conn, format!("presence:ws:{ws_id}"))
        .await
        .expect("clear presence");
}
