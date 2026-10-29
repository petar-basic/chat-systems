use axum::extract::ws::Message;
use axum::http::StatusCode;
use serde_json::{json, Value};
use sqlx::PgPool;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::common::*;
use crate::app_socket::AppSockets;

struct Seeded {
    app_id: Uuid,
    channel: Uuid,
    other_channel: Uuid,
}

async fn seed_app(pool: &PgPool) -> Seeded {
    let (owner, ws, channel) = seed_member_in_channel(pool).await;
    let other_channel = seed_channel(pool, ws, owner).await;
    let bot = seed_user(pool).await;
    let app_id: Uuid = sqlx::query_scalar(
        "INSERT INTO bot_apps (workspace_id, bot_user_id, name, token_hash, socket_mode, app_token_hash) \
         VALUES ($1, $2, 'Socket', $3, true, $4) RETURNING id",
    )
    .bind(ws)
    .bind(bot)
    .bind(Uuid::new_v4().simple().to_string())
    .bind(Uuid::new_v4().simple().to_string())
    .fetch_one(pool)
    .await
    .expect("seed app");
    Seeded {
        app_id,
        channel,
        other_channel,
    }
}

async fn ticket(pool: &PgPool, app_id: Uuid, age_secs: i64) -> String {
    let ticket = Uuid::new_v4().simple().to_string();
    sqlx::query(
        "INSERT INTO app_sockets (app_id, ticket_hash, opened_at) \
         VALUES ($1, encode(sha256(convert_to($2, 'UTF8')), 'hex'), NOW() - make_interval(secs => $3))",
    )
    .bind(app_id)
    .bind(&ticket)
    .bind(age_secs as f64)
    .execute(pool)
    .await
    .expect("ticket");
    ticket
}

async fn connect(
    apps: &AppSockets,
    pool: &PgPool,
    app_id: Uuid,
) -> (Uuid, mpsc::Receiver<Message>) {
    let admitted = apps
        .admit(&ticket(pool, app_id, 0).await)
        .await
        .expect("admitted");
    let (tx, rx) = mpsc::channel(64);
    apps.add(app_id, admitted.socket_id, tx);
    (admitted.socket_id, rx)
}

async fn queue(pool: &PgPool, app_id: Uuid, channel: Uuid, text: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO app_deliveries (app_id, event_id, channel_id, kind, body, expires_at) \
         VALUES ($1, $2, $3, 'event', $4, NOW() + INTERVAL '1 hour') RETURNING id",
    )
    .bind(app_id)
    .bind(Uuid::new_v4())
    .bind(channel)
    .bind(json!({ "type": "message", "event": { "text": text } }))
    .fetch_one(pool)
    .await
    .expect("queue")
}

async fn expire_leases(pool: &PgPool) {
    sqlx::query("UPDATE app_deliveries SET leased_until = NOW() - INTERVAL '1 second'")
        .execute(pool)
        .await
        .expect("expire");
}

async fn delivered(pool: &PgPool, id: Uuid) -> bool {
    sqlx::query_scalar("SELECT delivered_at IS NOT NULL FROM app_deliveries WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("delivered")
}

fn frames(rx: &mut mpsc::Receiver<Message>) -> Vec<Value> {
    drain_json(rx)
        .into_iter()
        .filter(|f| f.get("envelope_id").is_some())
        .collect()
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_ticket_opens_one_socket_while_it_is_fresh(pool: PgPool) {
    let apps = AppSockets::new(pool.clone());
    let s = seed_app(&pool).await;

    let fresh = ticket(&pool, s.app_id, 0).await;
    let admitted = apps.admit(&fresh).await.expect("a fresh ticket");
    assert_eq!(admitted.app_id, s.app_id);
    assert!(apps.admit(&fresh).await.is_none(), "a ticket is used once");

    let stale = ticket(&pool, s.app_id, 60).await;
    assert!(apps.admit(&stale).await.is_none(), "a ticket lasts seconds");
    assert!(apps.admit("not-a-ticket").await.is_none());

    let later = ticket(&pool, s.app_id, 0).await;
    sqlx::query("UPDATE bot_apps SET socket_mode = false WHERE id = $1")
        .bind(s.app_id)
        .execute(&pool)
        .await
        .expect("socket mode off");
    assert!(
        apps.admit(&later).await.is_none(),
        "socket mode was turned off"
    );

    let app = app(manager(pool.clone()).await);
    assert_eq!(
        get_status(&app, "/ws/apps?ticket=not-a-ticket").await,
        StatusCode::UNAUTHORIZED
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_event_comes_back_until_it_is_acknowledged(pool: PgPool) {
    let apps = AppSockets::new(pool.clone());
    let s = seed_app(&pool).await;
    let (_, mut rx) = connect(&apps, &pool, s.app_id).await;
    let id = queue(&pool, s.app_id, s.channel, "first").await;

    assert_eq!(apps.deliver_once().await, 1);
    let sent = frames(&mut rx);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0]["envelope_id"], json!(id));
    assert_eq!(sent[0]["type"], "event");
    assert_eq!(sent[0]["retry_attempt"], 0);
    assert_eq!(sent[0]["payload"]["event"]["text"], "first");

    assert_eq!(apps.deliver_once().await, 0, "it waits for the ack");
    expire_leases(&pool).await;
    assert_eq!(apps.deliver_once().await, 1);
    let again = frames(&mut rx);
    assert_eq!(
        again[0]["envelope_id"],
        json!(id),
        "unacknowledged, so sent again"
    );
    assert_eq!(again[0]["retry_attempt"], 1);

    apps.ack(s.app_id, id).await;
    assert!(delivered(&pool, id).await);
    expire_leases(&pool).await;
    assert_eq!(apps.deliver_once().await, 0, "an ack settles it");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_channel_is_delivered_in_order(pool: PgPool) {
    let apps = AppSockets::new(pool.clone());
    let s = seed_app(&pool).await;
    let (_, mut rx) = connect(&apps, &pool, s.app_id).await;
    let first = queue(&pool, s.app_id, s.channel, "first").await;
    let second = queue(&pool, s.app_id, s.channel, "second").await;

    apps.deliver_once().await;
    let sent = frames(&mut rx);
    assert_eq!(sent.len(), 1, "the second waits behind the first");
    assert_eq!(sent[0]["envelope_id"], json!(first));

    apps.ack(s.app_id, first).await;
    apps.deliver_once().await;
    assert_eq!(frames(&mut rx)[0]["envelope_id"], json!(second));
}

#[test_macros::db_test(migrations = "../migrations")]
async fn two_sockets_share_an_apps_events(pool: PgPool) {
    let apps = AppSockets::new(pool.clone());
    let s = seed_app(&pool).await;
    let (_, mut one) = connect(&apps, &pool, s.app_id).await;
    let (_, mut two) = connect(&apps, &pool, s.app_id).await;
    queue(&pool, s.app_id, s.channel, "here").await;
    queue(&pool, s.app_id, s.other_channel, "there").await;

    assert_eq!(apps.deliver_once().await, 2);
    assert_eq!(frames(&mut one).len(), 1, "each socket takes a share");
    assert_eq!(frames(&mut two).len(), 1);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_ack_for_another_app_settles_nothing(pool: PgPool) {
    let apps = AppSockets::new(pool.clone());
    let s = seed_app(&pool).await;
    let id = queue(&pool, s.app_id, s.channel, "mine").await;
    apps.ack(Uuid::new_v4(), id).await;
    assert!(!delivered(&pool, id).await);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_revoked_token_closes_the_socket(pool: PgPool) {
    let apps = AppSockets::new(pool.clone());
    let s = seed_app(&pool).await;
    let (_, mut rx) = connect(&apps, &pool, s.app_id).await;

    apps.check_alive().await;
    assert!(
        drain_json(&mut rx).is_empty(),
        "a live token keeps its socket"
    );

    sqlx::query("UPDATE bot_apps SET app_token_hash = NULL WHERE id = $1")
        .bind(s.app_id)
        .execute(&pool)
        .await
        .expect("revoke");
    sqlx::query("DELETE FROM app_sockets WHERE app_id = $1")
        .bind(s.app_id)
        .execute(&pool)
        .await
        .expect("drop sockets");
    apps.check_alive().await;

    let mut said_why = false;
    let mut closed = false;
    while let Ok(message) = rx.try_recv() {
        match message {
            Message::Text(text) => {
                said_why |=
                    serde_json::from_str::<Value>(&text).expect("json")["type"] == "disconnect";
            }
            Message::Close(_) => closed = true,
            _ => {}
        }
    }
    assert!(said_why && closed);

    queue(&pool, s.app_id, s.channel, "after").await;
    assert_eq!(
        apps.deliver_once().await,
        0,
        "nothing is sent to a closed app"
    );
}
