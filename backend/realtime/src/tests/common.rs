#![allow(dead_code)]

use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::ws::Message;
use axum::http::{header, Request, StatusCode};
use axum::Router;
use chrono::Utc;
use jsonwebtoken::{encode, EncodingKey, Header};
use serde_json::{json, Value};
use sqlx::PgPool;
use tokio::sync::mpsc;
use tower::ServiceExt;
use uuid::Uuid;

use crate::connection_manager::ConnectionManager;
use crate::{build_app, AppState};

pub const JWT_SECRET: &str = "integration-test-secret-key-0123456789-abcdef";

pub async fn redis_conn() -> redis::aio::ConnectionManager {
    let url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into());
    let client = redis::Client::open(url).expect("redis client");
    redis::aio::ConnectionManager::new(client)
        .await
        .expect("redis connect")
}

pub async fn manager(pool: PgPool) -> Arc<ConnectionManager> {
    Arc::new(ConnectionManager::new(pool, redis_conn().await))
}

pub fn app(cm: Arc<ConnectionManager>) -> Router {
    build_app(AppState {
        cm,
        jwt_secret: JWT_SECRET.to_string(),
        consumer_heartbeat: Arc::new(std::sync::atomic::AtomicI64::new(crate::now_unix())),
        cors_origins: "http://localhost".to_string(),
    })
}

pub fn fake_conn(cm: &ConnectionManager, user_id: Uuid) -> (Uuid, mpsc::Receiver<Message>) {
    let conn_id = Uuid::new_v4();
    let (tx, rx) = mpsc::channel::<Message>(256);
    cm.add_connection(conn_id, user_id, tx);
    (conn_id, rx)
}

pub fn next_json(rx: &mut mpsc::Receiver<Message>) -> Option<Value> {
    match rx.try_recv() {
        Ok(Message::Text(t)) => serde_json::from_str(&t).ok(),
        _ => None,
    }
}

pub fn drain_json(rx: &mut mpsc::Receiver<Message>) -> Vec<Value> {
    let mut out = Vec::new();
    while let Ok(msg) = rx.try_recv() {
        if let Message::Text(t) = msg {
            if let Ok(v) = serde_json::from_str(&t) {
                out.push(v);
            }
        }
    }
    out
}

pub async fn seed_user(pool: &PgPool) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, email, status) VALUES ($1, $2, 'active'::user_status)")
        .bind(id)
        .bind(format!("{id}@test.local"))
        .execute(pool)
        .await
        .expect("seed user");
    id
}

pub async fn seed_workspace(pool: &PgPool, owner: Uuid) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO workspaces (id, name, slug, owner_id) VALUES ($1, $2, $3, $4)")
        .bind(id)
        .bind("WS")
        .bind(format!("ws-{id}"))
        .bind(owner)
        .execute(pool)
        .await
        .expect("seed workspace");
    id
}

pub async fn add_ws_member(pool: &PgPool, ws: Uuid, user: Uuid) {
    sqlx::query(
        "INSERT INTO workspace_members (workspace_id, user_id, role) \
         VALUES ($1, $2, 'member'::workspace_role) ON CONFLICT DO NOTHING",
    )
    .bind(ws)
    .bind(user)
    .execute(pool)
    .await
    .expect("add ws member");
}

pub async fn seed_channel(pool: &PgPool, ws: Uuid, creator: Uuid) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO channels (id, workspace_id, name, channel_type, created_by) \
         VALUES ($1, $2, $3, 'public'::channel_type, $4)",
    )
    .bind(id)
    .bind(ws)
    .bind("general")
    .bind(creator)
    .execute(pool)
    .await
    .expect("seed channel");
    id
}

pub async fn add_ch_member(pool: &PgPool, ch: Uuid, user: Uuid) {
    sqlx::query(
        "INSERT INTO channel_members (channel_id, user_id, role) \
         VALUES ($1, $2, 'member'::channel_role) ON CONFLICT DO NOTHING",
    )
    .bind(ch)
    .bind(user)
    .execute(pool)
    .await
    .expect("add ch member");
}

pub async fn seed_member_in_channel(pool: &PgPool) -> (Uuid, Uuid, Uuid) {
    let user = seed_user(pool).await;
    let ws = seed_workspace(pool, user).await;
    add_ws_member(pool, ws, user).await;
    let ch = seed_channel(pool, ws, user).await;
    add_ch_member(pool, ch, user).await;
    (user, ws, ch)
}

pub fn mint_token(sub: Uuid, token_type: &str) -> String {
    let exp = (Utc::now() + chrono::Duration::hours(1)).timestamp();
    let claims = json!({ "sub": sub, "exp": exp, "token_type": token_type });
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(JWT_SECRET.as_bytes()),
    )
    .expect("mint token")
}

pub fn cookie_header(token: &str) -> axum::http::HeaderMap {
    let mut h = axum::http::HeaderMap::new();
    h.insert(
        header::COOKIE,
        format!("access_token={token}").parse().unwrap(),
    );
    h
}

pub fn protocol_header(token: &str) -> axum::http::HeaderMap {
    let mut h = axum::http::HeaderMap::new();
    h.insert(
        header::SEC_WEBSOCKET_PROTOCOL,
        format!("bearer, {token}").parse().unwrap(),
    );
    h
}

pub async fn get_status(app: &Router, uri: &str) -> StatusCode {
    let req = Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    app.clone().oneshot(req).await.unwrap().status()
}

pub async fn ws_upgrade_status(app: &Router, token: Option<&str>) -> StatusCode {
    let mut builder = Request::builder()
        .method("GET")
        .uri("/ws")
        .header(header::CONNECTION, "upgrade")
        .header(header::UPGRADE, "websocket")
        .header("sec-websocket-version", "13")
        .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==");
    if let Some(t) = token {
        builder = builder.header(header::COOKIE, format!("access_token={t}"));
    }
    let resp = app
        .clone()
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap();
    resp.status()
}

pub async fn settle() {
    tokio::time::sleep(Duration::from_millis(20)).await;
}
