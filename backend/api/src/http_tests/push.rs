use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;
use crate::notifications::consumer::notify_message;
use crate::notifications::models::{NotificationType, NotifyLevel};
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

async fn join_channel(state: &AppState, ch_id: Uuid, user_id: Uuid) {
    state
        .workspace_service
        .repo
        .add_channel_member(
            ch_id,
            user_id,
            &crate::workspace::models::ChannelRole::Member,
        )
        .await
        .expect("add the user to the channel");
}

async fn queue_due(state: &AppState, user_id: Uuid, workspace_id: Uuid, direct: bool) -> Uuid {
    let message_id = Uuid::new_v4();
    let kind = if direct {
        NotificationType::Dm
    } else {
        NotificationType::Mention
    };
    state
        .notification_repo
        .create(
            user_id,
            workspace_id,
            &kind,
            "You were mentioned",
            None,
            &json!({ "message_id": message_id }),
        )
        .await
        .expect("an unread notification");
    sqlx::query(
        "INSERT INTO pending_mention_emails \
           (user_id, workspace_id, message_id, sender_name, channel_name, direct, created_at) \
         VALUES ($1, $2, $3, 'Ana', 'general', $4, NOW() - INTERVAL '10 minutes')",
    )
    .bind(user_id)
    .bind(workspace_id)
    .bind(message_id)
    .bind(direct)
    .execute(&state.pool)
    .await
    .expect("queue a due digest");
    message_id
}

async fn pending_for(state: &AppState, user_id: Uuid) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM pending_mention_emails WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(&state.pool)
        .await
        .expect("count")
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
    join_channel(&state, ch_id, target).await;
    let _ = author_token;

    let service = fake_push_service(StatusCode::CREATED).await;
    subscribe(&state, target, &format!("{}/push/laptop", service.address)).await;
    subscribe(&state, target, &format!("{}/push/phone", service.address)).await;

    let mut conn = state.redis.clone();
    notify_message(
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
async fn nothing_is_pushed_under_dnd_a_mute_or_an_active_session(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (author, _, _) = seed_and_login(&app, &state, "push-author", false).await;
    let (target, _, _) = seed_and_login(&app, &state, "push-target", false).await;
    let ws_id = seed_workspace(&state, author, "Quiet WS").await;
    add_ws_member(&state, ws_id, target, "member").await;
    let ch_id = seed_channel(&state, ws_id, author, "quiet", false).await;
    join_channel(&state, ch_id, target).await;
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
    notify_message(
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
            ws_id,
            target,
            Some(chrono::Utc::now() + chrono::Duration::hours(1)),
        )
        .await
        .expect("set dnd");
    notify_message(
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
        .set_dnd(ws_id, target, None)
        .await
        .expect("clear dnd");
    let expires_at = chrono::Utc::now().timestamp() + 60;
    let _: () = redis::AsyncCommands::hset(
        &mut conn,
        format!("active:user:{target}"),
        Uuid::new_v4().to_string(),
        expires_at,
    )
    .await
    .expect("mark them active");
    notify_message(
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

    let _: () = redis::AsyncCommands::del(&mut conn, format!("active:user:{target}"))
        .await
        .expect("clear activity");
    notify_message(
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

#[test_macros::db_test(migrations = "../migrations")]
async fn after_leaving_a_workspace_the_push_badge_agrees_with_the_app(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner, _, _) = seed_and_login(&app, &state, "badge-owner", false).await;
    let (target, _, target_token) = seed_and_login(&app, &state, "badge-target", false).await;
    let kept = seed_workspace(&state, owner, "Kept WS").await;
    let left = seed_workspace(&state, owner, "Left WS").await;
    for ws_id in [kept, left] {
        add_ws_member(&state, ws_id, target, "member").await;
        state
            .notification_repo
            .create(
                target,
                ws_id,
                &NotificationType::Mention,
                "You were mentioned",
                None,
                &json!({ "message_id": Uuid::new_v4() }),
            )
            .await
            .expect("an unread notification");
    }

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/workspaces/{left}/members/{target}"),
        Some(&target_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = send(
        &app,
        "GET",
        &format!("/workspaces/{kept}/notifications/unread-count"),
        Some(&target_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let in_app = body["unread_count"].as_i64().expect("unread_count");
    let badge = state
        .notification_repo
        .unread_count_everywhere(target)
        .await
        .expect("badge");
    assert_eq!(in_app, 1);
    assert_eq!(badge, in_app, "the phone counts what the app counts");
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
                thread_parent_id: None,
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
    join_channel(&state, ch_id, target).await;

    let mut conn = state.redis.clone();
    let event = json!({
        "user_id": author.to_string(),
        "workspace_id": ws_id.to_string(),
        "channel_id": ch_id.to_string(),
        "id": Uuid::new_v4().to_string(),
        "content": "are you there @target",
        "mentioned_user_ids": [target.to_string()],
    });

    notify_message(
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
async fn a_push_does_not_cancel_the_email_and_a_redelivery_queues_it_once(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (author, _, _) = seed_and_login(&app, &state, "mail-author", false).await;
    let (target, _, _) = seed_and_login(&app, &state, "mail-target", false).await;
    let ws_id = seed_workspace(&state, author, "Mail WS").await;
    add_ws_member(&state, ws_id, target, "member").await;
    let ch_id = seed_channel(&state, ws_id, author, "quiet", false).await;
    join_channel(&state, ch_id, target).await;

    let service = fake_push_service(StatusCode::CREATED).await;
    subscribe(&state, target, &format!("{}/push/laptop", service.address)).await;

    let message_id = Uuid::new_v4();
    let event = json!({
        "user_id": author.to_string(),
        "workspace_id": ws_id.to_string(),
        "channel_id": ch_id.to_string(),
        "id": message_id.to_string(),
        "content": "ping @target",
        "mentioned_user_ids": [target.to_string()],
    });
    let mut conn = state.redis.clone();
    for _ in 0..2 {
        notify_message(
            &state,
            &state.notification_repo,
            &sender_for(&state),
            &mut conn,
            &event,
        )
        .await;
    }

    assert!(
        service.hits.load(Ordering::SeqCst) >= 1,
        "the push went out"
    );
    assert_eq!(
        pending_for(&state, target).await,
        1,
        "a push accepted is not a push seen, and the redelivery is the same mention"
    );

    sqlx::query("UPDATE pending_mention_emails SET created_at = NOW() - INTERVAL '10 minutes'")
        .execute(&state.pool)
        .await
        .expect("age the queue");
    assert_eq!(crate::notifications::email::flush_due(&state).await, 1);
    assert_eq!(
        crate::notifications::email::flush_due(&state).await,
        0,
        "once"
    );

    let body: String = sqlx::query_scalar(
        "SELECT text_body FROM outbound_emails ORDER BY created_at DESC LIMIT 1",
    )
    .fetch_one(&state.pool)
    .await
    .expect("email");
    assert!(
        body.contains(&format!("/app/{ws_id}/{ch_id}/{message_id}")),
        "the link names the workspace and the message: {body}"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn somebody_who_turned_mention_emails_off_is_not_queued(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (author, _, _) = seed_and_login(&app, &state, "mail-author", false).await;
    let (target, _, target_token) = seed_and_login(&app, &state, "mail-target", false).await;
    let ws_id = seed_workspace(&state, author, "Mail WS").await;
    add_ws_member(&state, ws_id, target, "member").await;
    let ch_id = seed_channel(&state, ws_id, author, "quiet", false).await;
    join_channel(&state, ch_id, target).await;

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/notifications/email"),
        Some(&target_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["mention_emails"], json!(true), "on by default");

    let (status, _) = send(
        &app,
        "PATCH",
        &format!("/api/workspaces/{ws_id}/notifications/email"),
        Some(&target_token),
        Some(json!({ "mention_emails": false })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let mut conn = state.redis.clone();
    notify_message(
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

#[sqlx::test(migrations = "../migrations")]
async fn a_mention_read_inside_the_window_is_never_emailed(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (author, _, _) = seed_and_login(&app, &state, "mail-author", false).await;
    let (target, _, _) = seed_and_login(&app, &state, "mail-target", false).await;
    let ws_id = seed_workspace(&state, author, "Mail WS").await;
    add_ws_member(&state, ws_id, target, "member").await;

    let read = queue_due(&state, target, ws_id, false).await;
    queue_due(&state, target, ws_id, false).await;
    sqlx::query("UPDATE notifications SET is_read = TRUE WHERE data ->> 'message_id' = $1")
        .bind(read.to_string())
        .execute(&state.pool)
        .await
        .expect("read one");

    assert_eq!(crate::notifications::email::flush_due(&state).await, 1);
    let subject: String =
        sqlx::query_scalar("SELECT subject FROM outbound_emails ORDER BY created_at DESC LIMIT 1")
            .fetch_one(&state.pool)
            .await
            .expect("email");
    assert_eq!(
        subject, "You were mentioned",
        "only the unread one is counted"
    );
    assert_eq!(pending_for(&state, target).await, 0);

    let all_read = queue_due(&state, target, ws_id, false).await;
    sqlx::query("UPDATE notifications SET is_read = TRUE WHERE data ->> 'message_id' = $1")
        .bind(all_read.to_string())
        .execute(&state.pool)
        .await
        .expect("read it");
    assert_eq!(
        crate::notifications::email::flush_due(&state).await,
        0,
        "nothing left unread, nothing sent"
    );
    assert_eq!(
        pending_for(&state, target).await,
        0,
        "and the queue is cleared rather than retried forever"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn an_open_tab_nobody_is_using_does_not_swallow_the_push(pool: PgPool) {
    let s = scene(pool).await;
    let mut conn = s.state.redis.clone();
    let now = chrono::Utc::now().timestamp();
    let _: () = redis::AsyncCommands::zadd(
        &mut conn,
        format!("presence:ws:{}", s.ws_id),
        s.target.to_string(),
        now + 60,
    )
    .await
    .expect("a socket is open");

    assert_eq!(
        s.post(s.ch_id, "idle?", true, None).await,
        1,
        "idle: pushed"
    );

    let active = format!("active:user:{}", s.target);
    let _: () =
        redis::AsyncCommands::hset(&mut conn, &active, Uuid::new_v4().to_string(), now + 60)
            .await
            .expect("a session reports itself active");
    assert_eq!(
        s.post(s.ch_id, "active?", true, None).await,
        0,
        "active: held"
    );

    let _: () = redis::AsyncCommands::del(&mut conn, &active)
        .await
        .expect("clear");
    let _: () = redis::AsyncCommands::hset(&mut conn, &active, Uuid::new_v4().to_string(), now - 1)
        .await
        .expect("a session whose report lapsed");
    assert_eq!(
        s.post(s.ch_id, "lapsed?", true, None).await,
        1,
        "lapsed: pushed"
    );

    let _: () = redis::AsyncCommands::del(&mut conn, &active)
        .await
        .expect("clear");
    let _: () = redis::AsyncCommands::del(&mut conn, format!("presence:ws:{}", s.ws_id))
        .await
        .expect("clear presence");
}

struct Scene {
    app: axum::Router,
    state: Arc<AppState>,
    author: Uuid,
    author_token: String,
    target: Uuid,
    ws_id: Uuid,
    ch_id: Uuid,
    service: FakePushService,
}

async fn scene(pool: PgPool) -> Scene {
    let (app, state) = app_and_state(pool).await;
    let (author, _, author_token) = seed_and_login(&app, &state, "level-author", false).await;
    let (target, _, _) = seed_and_login(&app, &state, "level-target", false).await;
    let ws_id = seed_workspace(&state, author, "Levels WS").await;
    add_ws_member(&state, ws_id, target, "member").await;
    let ch_id = seed_channel(&state, ws_id, author, "levels", false).await;
    join_channel(&state, ch_id, target).await;
    let service = fake_push_service(StatusCode::CREATED).await;
    subscribe(&state, target, &format!("{}/push/phone", service.address)).await;
    Scene {
        app,
        state,
        author,
        author_token,
        target,
        ws_id,
        ch_id,
        service,
    }
}

impl Scene {
    async fn post(
        &self,
        channel_id: Uuid,
        content: &str,
        mention: bool,
        thread: Option<Uuid>,
    ) -> usize {
        let before = self.service.hits.load(Ordering::SeqCst);
        let mentioned: Vec<String> = if mention {
            vec![self.target.to_string()]
        } else {
            Vec::new()
        };
        let mut conn = self.state.redis.clone();
        notify_message(
            &self.state,
            &self.state.notification_repo,
            &sender_for(&self.state),
            &mut conn,
            &json!({
                "user_id": self.author.to_string(),
                "workspace_id": self.ws_id.to_string(),
                "channel_id": channel_id.to_string(),
                "id": Uuid::new_v4().to_string(),
                "thread_parent_id": thread.map(|t| t.to_string()),
                "content": content,
                "mentioned_user_ids": mentioned,
            }),
        )
        .await;
        self.service.hits.load(Ordering::SeqCst) - before
    }

    async fn reply_also_in_channel(&self, root: Uuid) -> usize {
        let before = self.service.hits.load(Ordering::SeqCst);
        let mut conn = self.state.redis.clone();
        notify_message(
            &self.state,
            &self.state.notification_repo,
            &sender_for(&self.state),
            &mut conn,
            &json!({
                "user_id": self.author.to_string(),
                "workspace_id": self.ws_id.to_string(),
                "channel_id": self.ch_id.to_string(),
                "id": Uuid::new_v4().to_string(),
                "thread_parent_id": root.to_string(),
                "content": "shipping it",
                "metadata": { "also_in_channel": true },
                "mentioned_user_ids": [],
            }),
        )
        .await;
        self.service.hits.load(Ordering::SeqCst) - before
    }

    async fn set_level(&self, channel_id: Uuid, level: Option<NotifyLevel>) {
        self.state
            .notification_repo
            .set_channel_level(channel_id, self.target, level)
            .await
            .expect("set level")
            .expect("the target is a member");
    }

    async fn keywords(&self, user_id: Uuid, keywords: &[&str]) {
        let keywords: Vec<String> = keywords.iter().map(|k| k.to_string()).collect();
        self.state
            .notification_repo
            .set_keywords(self.ws_id, user_id, &keywords)
            .await
            .expect("keywords");
    }

    async fn activity(&self) -> Vec<String> {
        sqlx::query_scalar("SELECT notification_type::text FROM notifications WHERE user_id = $1 ORDER BY created_at")
            .bind(self.target)
            .fetch_all(&self.state.pool)
            .await
            .expect("notifications")
    }

    async fn direct_conversation(&self) -> Uuid {
        let (status, body) = send(
            &self.app,
            "POST",
            &format!("/api/workspaces/{}/conversations", self.ws_id),
            Some(&self.author_token),
            Some(json!({ "participant_ids": [self.target] })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body:?}");
        body["id"].as_str().expect("id").parse().expect("uuid")
    }
}

#[test_macros::db_test(migrations = "../migrations")]
async fn each_level_decides_what_reaches_the_phone(pool: PgPool) {
    let s = scene(pool).await;
    let cases = [
        (NotifyLevel::All, false, 1),
        (NotifyLevel::All, true, 1),
        (NotifyLevel::Mentions, false, 0),
        (NotifyLevel::Mentions, true, 1),
        (NotifyLevel::Nothing, false, 0),
        (NotifyLevel::Nothing, true, 0),
    ];
    for (level, mention, expected) in cases {
        s.set_level(s.ch_id, Some(level)).await;
        assert_eq!(
            s.post(s.ch_id, "standup in five", mention, None).await,
            expected,
            "{level:?}, mention: {mention}"
        );
    }

    s.set_level(s.ch_id, Some(NotifyLevel::All)).await;
    sqlx::query("UPDATE channel_members SET muted = TRUE WHERE channel_id = $1 AND user_id = $2")
        .bind(s.ch_id)
        .bind(s.target)
        .execute(&s.state.pool)
        .await
        .expect("mute");
    assert_eq!(
        s.post(s.ch_id, "anyone?", true, None).await,
        0,
        "mute overrides every level"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn every_message_on_all_reaches_the_phone_but_not_the_activity_panel(pool: PgPool) {
    let s = scene(pool).await;
    s.set_level(s.ch_id, Some(NotifyLevel::All)).await;

    assert_eq!(s.post(s.ch_id, "deploy finished", false, None).await, 1);
    assert!(
        s.activity().await.is_empty(),
        "Activity is for what is addressed to you"
    );
    assert_eq!(
        s.post(s.ch_id, "reply in a thread", false, Some(Uuid::new_v4()))
            .await,
        0,
        "a thread reply is not a new message in the channel"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_reply_also_sent_to_the_channel_reaches_everyone_on_all(pool: PgPool) {
    let s = scene(pool).await;
    s.set_level(s.ch_id, Some(NotifyLevel::All)).await;

    assert_eq!(
        s.reply_also_in_channel(Uuid::new_v4()).await,
        1,
        "a reply sent to the channel too is a message in the channel"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_workspace_default_applies_until_a_channel_sets_its_own(pool: PgPool) {
    let s = scene(pool).await;
    assert_eq!(
        s.post(s.ch_id, "hello", false, None).await,
        0,
        "mentions by default"
    );

    s.state
        .notification_repo
        .set_default(s.ws_id, s.target, NotifyLevel::All)
        .await
        .expect("set default");
    assert_eq!(s.post(s.ch_id, "hello", false, None).await, 1);

    s.set_level(s.ch_id, Some(NotifyLevel::Mentions)).await;
    assert_eq!(
        s.post(s.ch_id, "hello", false, None).await,
        0,
        "a level set on the channel wins"
    );

    s.set_level(s.ch_id, None).await;
    assert_eq!(
        s.post(s.ch_id, "hello", false, None).await,
        1,
        "back to the default"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_direct_message_reaches_the_phone_and_the_activity_panel(pool: PgPool) {
    let s = scene(pool).await;
    s.state
        .notification_repo
        .set_default(s.ws_id, s.target, NotifyLevel::Nothing)
        .await
        .expect("set default");
    let dm = s.direct_conversation().await;

    assert_eq!(
        s.post(dm, "lunch?", false, None).await,
        1,
        "a DM is addressed to you whatever the workspace default"
    );
    assert_eq!(s.activity().await, vec!["dm".to_string()]);

    s.set_level(dm, Some(NotifyLevel::Mentions)).await;
    assert_eq!(
        s.post(dm, "lunch??", false, None).await,
        0,
        "unless you said otherwise"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_direct_message_under_dnd_waits_in_activity(pool: PgPool) {
    let s = scene(pool).await;
    let dm = s.direct_conversation().await;
    s.state
        .notification_repo
        .set_dnd(
            s.ws_id,
            s.target,
            Some(chrono::Utc::now() + chrono::Duration::hours(1)),
        )
        .await
        .expect("set dnd");

    assert_eq!(s.post(dm, "lunch?", false, None).await, 0);
    assert_eq!(s.activity().await, vec!["dm".to_string()]);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_direct_message_that_reached_nobody_is_queued_for_email(pool: PgPool) {
    let s = scene(pool).await;
    s.state
        .push_repo
        .delete(s.target, &format!("{}/push/phone", s.service.address))
        .await
        .expect("unsubscribe");
    let dm = s.direct_conversation().await;

    s.post(dm, "call me", false, None).await;

    let direct: Vec<bool> =
        sqlx::query_scalar("SELECT direct FROM pending_mention_emails WHERE user_id = $1")
            .bind(s.target)
            .fetch_all(&s.state.pool)
            .await
            .expect("pending");
    assert_eq!(direct, vec![true]);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_mention_of_somebody_outside_the_channel_reaches_nobody(pool: PgPool) {
    let s = scene(pool).await;
    sqlx::query("DELETE FROM channel_members WHERE channel_id = $1 AND user_id = $2")
        .bind(s.ch_id)
        .bind(s.target)
        .execute(&s.state.pool)
        .await
        .expect("leave");

    assert_eq!(s.post(s.ch_id, "ping", true, None).await, 0);
    assert!(s.activity().await.is_empty());
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_digest_counts_mentions_and_direct_messages_apart(pool: PgPool) {
    let s = scene(pool).await;
    for direct in [false, false, true] {
        queue_due(&s.state, s.target, s.ws_id, direct).await;
    }

    assert_eq!(crate::notifications::email::flush_due(&s.state).await, 1);
    let (subject, body): (String, String) = sqlx::query_as(
        "SELECT subject, text_body FROM outbound_emails ORDER BY created_at DESC LIMIT 1",
    )
    .fetch_one(&s.state.pool)
    .await
    .expect("email");
    assert_eq!(subject, "You have 2 mentions and 1 direct message");
    assert!(
        body.starts_with("You have 2 mentions and 1 direct message while you were away."),
        "{body}"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_keyword_matches_whole_words_whatever_the_case_or_accents(pool: PgPool) {
    let s = scene(pool).await;
    s.keywords(s.target, &["deploy", "c++", "a.b", "Čačak", "on call"])
        .await;

    let cases = [
        ("Deploy!", 1),
        ("deploý tonight", 1),
        ("we redeployed it", 0),
        ("deploy_script ran", 0),
        ("I love C++.", 1),
        ("cpp is not it", 0),
        ("a.b works", 1),
        ("axb does not", 0),
        ("vidimo se u cacak", 1),
        ("who is On Call?", 1),
    ];
    let mut expected = 0;
    for (content, hits) in cases {
        assert_eq!(
            s.post(s.ch_id, content, false, None).await,
            hits,
            "{content}"
        );
        expected += hits;
    }

    let activity = s.activity().await;
    assert_eq!(activity.len(), expected);
    assert!(activity.iter().all(|kind| kind == "keyword"));
    let title: String = sqlx::query_scalar(
        "SELECT title FROM notifications WHERE user_id = $1 ORDER BY created_at LIMIT 1",
    )
    .bind(s.target)
    .fetch_one(&s.state.pool)
    .await
    .expect("title");
    assert_eq!(title, "Matched \"deploy\" in #levels");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_mention_that_also_matches_a_keyword_is_one_mention(pool: PgPool) {
    let s = scene(pool).await;
    s.keywords(s.target, &["deploy"]).await;

    assert_eq!(s.post(s.ch_id, "can you deploy?", true, None).await, 1);
    assert_eq!(s.activity().await, vec!["mention"]);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_keyword_in_a_thread_notifies_even_somebody_who_does_not_follow_it(pool: PgPool) {
    let s = scene(pool).await;
    s.keywords(s.target, &["outage"]).await;

    assert_eq!(
        s.post(
            s.ch_id,
            "this looks like an outage",
            false,
            Some(Uuid::new_v4())
        )
        .await,
        1
    );
    assert_eq!(s.activity().await, vec!["keyword"]);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn muted_and_nothing_channels_never_notify_for_keywords(pool: PgPool) {
    let s = scene(pool).await;
    s.keywords(s.target, &["deploy"]).await;

    s.set_level(s.ch_id, Some(NotifyLevel::Nothing)).await;
    assert_eq!(s.post(s.ch_id, "deploy now", false, None).await, 0);

    s.set_level(s.ch_id, None).await;
    sqlx::query("UPDATE channel_members SET muted = TRUE WHERE channel_id = $1 AND user_id = $2")
        .bind(s.ch_id)
        .bind(s.target)
        .execute(&s.state.pool)
        .await
        .expect("mute");
    assert_eq!(s.post(s.ch_id, "deploy again", false, None).await, 0);
    assert!(s.activity().await.is_empty());
}

#[test_macros::db_test(migrations = "../migrations")]
async fn nobody_hears_their_own_keyword_or_one_in_a_channel_they_are_not_in(pool: PgPool) {
    let s = scene(pool).await;
    s.keywords(s.author, &["deploy"]).await;
    s.keywords(s.target, &["deploy"]).await;
    let private = seed_channel(&s.state, s.ws_id, s.author, "ops-private", true).await;

    assert_eq!(
        s.post(private, "deploy in the private room", false, None)
            .await,
        0
    );
    assert!(
        s.activity().await.is_empty(),
        "the target is not in that channel"
    );

    s.post(s.ch_id, "deploy here", false, None).await;
    let authors: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM notifications WHERE user_id = $1")
        .bind(s.author)
        .fetch_one(&s.state.pool)
        .await
        .expect("count");
    assert_eq!(authors, 0, "the author wrote the word");
    assert_eq!(s.activity().await, vec!["keyword"]);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_keyword_under_dnd_waits_in_activity(pool: PgPool) {
    let s = scene(pool).await;
    s.keywords(s.target, &["deploy"]).await;
    sqlx::query(
        "UPDATE workspace_members SET dnd_until = NOW() + INTERVAL '1 hour' \
          WHERE workspace_id = $1 AND user_id = $2",
    )
    .bind(s.ws_id)
    .bind(s.target)
    .execute(&s.state.pool)
    .await
    .expect("dnd");

    assert_eq!(s.post(s.ch_id, "deploy done", false, None).await, 0);
    assert_eq!(s.activity().await, vec!["keyword"]);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_digest_waits_while_the_schedule_pauses_notifications(pool: PgPool) {
    let s = scene(pool).await;
    queue_due(&s.state, s.target, s.ws_id, false).await;
    let now = chrono::Utc::now();
    let minute = |offset: i64| (((now.timestamp() / 60 + offset) % 1440 + 1440) % 1440) as i16;
    s.state
        .notification_repo
        .set_schedule(
            s.ws_id,
            s.target,
            true,
            &[1, 2, 3, 4, 5, 6, 7],
            minute(120),
            minute(180),
        )
        .await
        .expect("schedule");

    assert_eq!(crate::notifications::email::flush_due(&s.state).await, 0);

    s.state
        .notification_repo
        .set_schedule(
            s.ws_id,
            s.target,
            false,
            &[1, 2, 3, 4, 5, 6, 7],
            minute(120),
            minute(180),
        )
        .await
        .expect("schedule off");
    assert_eq!(crate::notifications::email::flush_due(&s.state).await, 1);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_pause_in_another_workspace_leaves_this_one_notifying(pool: PgPool) {
    let s = scene(pool).await;
    let elsewhere = seed_workspace(&s.state, s.target, "Elsewhere").await;
    s.state
        .notification_repo
        .set_dnd(
            elsewhere,
            s.target,
            Some(chrono::Utc::now() + chrono::Duration::hours(1)),
        )
        .await
        .expect("set dnd");

    assert_eq!(
        s.post(s.ch_id, "anyone?", true, None).await,
        1,
        "paused there, not here"
    );

    s.state
        .notification_repo
        .set_dnd(
            s.ws_id,
            s.target,
            Some(chrono::Utc::now() + chrono::Duration::hours(1)),
        )
        .await
        .expect("set dnd");
    assert_eq!(s.post(s.ch_id, "anyone?", true, None).await, 0);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_digest_waits_only_for_the_workspace_that_is_paused(pool: PgPool) {
    let s = scene(pool).await;
    let elsewhere = seed_workspace(&s.state, s.target, "Elsewhere").await;
    for ws in [s.ws_id, elsewhere] {
        queue_due(&s.state, s.target, ws, false).await;
    }
    s.state
        .notification_repo
        .set_dnd(
            elsewhere,
            s.target,
            Some(chrono::Utc::now() + chrono::Duration::hours(1)),
        )
        .await
        .expect("set dnd");

    assert_eq!(crate::notifications::email::flush_due(&s.state).await, 1);
    let waiting: Vec<Uuid> =
        sqlx::query_scalar("SELECT workspace_id FROM pending_mention_emails WHERE user_id = $1")
            .bind(s.target)
            .fetch_all(&s.state.pool)
            .await
            .expect("pending");
    assert_eq!(waiting, vec![elsewhere]);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_hook_message_notifies_its_creator_under_the_bot_name(pool: PgPool) {
    let s = scene(pool).await;
    let before = s.service.hits.load(Ordering::SeqCst);
    let mut conn = s.state.redis.clone();
    notify_message(
        &s.state,
        &s.state.notification_repo,
        &sender_for(&s.state),
        &mut conn,
        &json!({
            "user_id": s.target.to_string(),
            "workspace_id": s.ws_id.to_string(),
            "channel_id": s.ch_id.to_string(),
            "id": Uuid::new_v4().to_string(),
            "content": "build failed, @target",
            "mentioned_user_ids": [s.target.to_string()],
            "metadata": { "bot": { "hook_id": Uuid::new_v4(), "name": "Deploybot" } },
        }),
    )
    .await;

    assert_eq!(
        s.service.hits.load(Ordering::SeqCst) - before,
        1,
        "the hook posts under its creator's id, and still reaches them"
    );
    assert_eq!(s.activity().await, vec!["mention"]);
    let sender: String =
        sqlx::query_scalar("SELECT sender_name FROM pending_mention_emails WHERE user_id = $1")
            .bind(s.target)
            .fetch_one(&s.state.pool)
            .await
            .expect("pending");
    assert_eq!(sender, "Deploybot", "named after the bot, not the creator");
}

#[sqlx::test(migrations = "../migrations")]
async fn a_mention_in_a_reply_sent_to_the_channel_is_read_with_the_channel(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (author, _, author_token) = seed_and_login(&app, &state, "mail-author", false).await;
    let (target, _, target_token) = seed_and_login(&app, &state, "mail-target", false).await;
    let ws_id = seed_workspace(&state, author, "Mail WS").await;
    add_ws_member(&state, ws_id, target, "member").await;
    let ch_id = seed_channel(&state, ws_id, author, "quiet", false).await;
    join_channel(&state, ch_id, target).await;

    let (_, root) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/messages"),
        Some(&author_token),
        Some(json!({ "content": "root" })),
    )
    .await;
    let (status, reply) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/messages"),
        Some(&author_token),
        Some(json!({ "content": "look @target", "thread_parent_id": root["id"], "also_to_channel": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reply:?}");

    let mut conn = state.redis.clone();
    notify_message(
        &state,
        &state.notification_repo,
        &sender_for(&state),
        &mut conn,
        &json!({
            "user_id": author.to_string(),
            "workspace_id": ws_id.to_string(),
            "channel_id": ch_id.to_string(),
            "id": reply["id"],
            "thread_parent_id": root["id"],
            "metadata": { "also_in_channel": true },
            "content": "look @target",
            "mentioned_user_ids": [target.to_string()],
        }),
    )
    .await;
    assert_eq!(pending_for(&state, target).await, 1);

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/read"),
        Some(&target_token),
        Some(json!({ "message_id": reply["id"] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let unread: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM notifications WHERE user_id = $1 AND NOT is_read")
            .bind(target)
            .fetch_one(&state.pool)
            .await
            .expect("count");
    assert_eq!(unread, 0, "reading the channel reads the reply sent to it");

    sqlx::query("UPDATE pending_mention_emails SET created_at = NOW() - INTERVAL '10 minutes'")
        .execute(&state.pool)
        .await
        .expect("age the queue");
    assert_eq!(crate::notifications::email::flush_due(&state).await, 0);
    assert_eq!(pending_for(&state, target).await, 0, "and no email follows");
}

#[sqlx::test(migrations = "../migrations")]
async fn a_keyword_inside_a_forwarded_quote_alerts_nobody(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (author, _, _) = seed_and_login(&app, &state, "kw-author", false).await;
    let (target, _, target_token) = seed_and_login(&app, &state, "kw-target", false).await;
    let ws_id = seed_workspace(&state, author, "Keyword WS").await;
    add_ws_member(&state, ws_id, target, "member").await;
    let ch_id = seed_channel(&state, ws_id, author, "ops", false).await;
    join_channel(&state, ch_id, target).await;
    let (status, _) = send(
        &app,
        "PUT",
        &format!("/api/workspaces/{ws_id}/notification-keywords"),
        Some(&target_token),
        Some(json!({ "keywords": ["deploy"] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let keyword_alerts = || async {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM notifications WHERE user_id = $1 AND notification_type = 'keyword'",
        )
        .bind(target)
        .fetch_one(&state.pool)
        .await
        .expect("count")
    };
    let mut conn = state.redis.clone();
    let event = |content: &str, forwarded: bool| {
        let mut event = json!({
            "user_id": author.to_string(),
            "workspace_id": ws_id.to_string(),
            "channel_id": ch_id.to_string(),
            "id": Uuid::new_v4().to_string(),
            "content": content,
        });
        if forwarded {
            event["metadata"] = json!({ "forwarded_from": { "message_id": Uuid::new_v4() } });
        }
        event
    };

    notify_message(
        &state,
        &state.notification_repo,
        &sender_for(&state),
        &mut conn,
        &event("> the deploy is done\n\nfyi", true),
    )
    .await;
    assert_eq!(
        keyword_alerts().await,
        0,
        "the quote is somebody else's words"
    );

    notify_message(
        &state,
        &state.notification_repo,
        &sender_for(&state),
        &mut conn,
        &event("fyi, deploy at noon\n\n> earlier words", true),
    )
    .await;
    assert_eq!(
        keyword_alerts().await,
        1,
        "what the forwarder wrote still counts"
    );
}
