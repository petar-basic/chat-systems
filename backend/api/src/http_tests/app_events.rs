use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, Mutex};

use axum::http::{HeaderMap, StatusCode};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;
use crate::app_events::{consumer, sender};
use crate::hooks::executor::{sign_body, SIGNATURE_HEADER};
use crate::state::AppState;

type Seen = Arc<Mutex<Vec<(HeaderMap, String)>>>;

struct Endpoint {
    url: String,
    seen: Seen,
    status: Arc<AtomicU16>,
}

impl Endpoint {
    fn events(&self) -> Vec<(HeaderMap, String)> {
        self.seen
            .lock()
            .expect("seen")
            .iter()
            .filter(|(_, body)| !body.contains("url_verification"))
            .cloned()
            .collect()
    }

    fn answer(&self, status: u16) {
        self.status.store(status, Ordering::SeqCst);
    }
}

async fn endpoint(echoes_challenge: bool) -> Endpoint {
    let seen: Seen = Arc::new(Mutex::new(Vec::new()));
    let status = Arc::new(AtomicU16::new(200));
    let (recorded, answering) = (seen.clone(), status.clone());
    let app = axum::Router::new().fallback(move |headers: HeaderMap, body: String| {
        let recorded = recorded.clone();
        let answering = answering.clone();
        async move {
            recorded.lock().expect("seen").push((headers, body.clone()));
            let parsed: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
            if parsed["type"] == "url_verification" {
                let challenge = if echoes_challenge {
                    parsed["challenge"].clone()
                } else {
                    json!("nope")
                };
                return (
                    StatusCode::OK,
                    json!({ "challenge": challenge }).to_string(),
                );
            }
            let code = StatusCode::from_u16(answering.load(Ordering::SeqCst)).expect("status");
            (code, String::new())
        }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind the fake app");
    let url = format!("http://{}/events", listener.local_addr().expect("addr"));
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    Endpoint { url, seen, status }
}

struct Setup {
    app: axum::Router,
    state: Arc<AppState>,
    owner_id: Uuid,
    token: String,
    ws_id: Uuid,
    app_id: Uuid,
    bot_user_id: Uuid,
    bot_token: String,
}

async fn setup(pool: PgPool, scopes: &[&str]) -> Setup {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "events-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Events").await;
    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/bot-apps"),
        Some(&token),
        Some(json!({ "name": "Listener", "scopes": scopes })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    Setup {
        app_id: body["id"].as_str().expect("id").parse().expect("uuid"),
        bot_user_id: body["bot_user_id"]
            .as_str()
            .expect("bot")
            .parse()
            .expect("uuid"),
        bot_token: body["token"].as_str().expect("token").to_string(),
        app,
        state,
        owner_id,
        token,
        ws_id,
    }
}

impl Setup {
    async fn patch(&self, body: Value) -> (StatusCode, Value) {
        send(
            &self.app,
            "PATCH",
            &format!("/api/bot-apps/{}", self.app_id),
            Some(&self.token),
            Some(body),
        )
        .await
    }

    async fn get(&self) -> Value {
        let (status, body) = send(
            &self.app,
            "GET",
            &format!("/api/bot-apps/{}", self.app_id),
            Some(&self.token),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body:?}");
        body
    }

    async fn subscribe(&self, events: &[&str], to: &Endpoint) {
        let (status, body) = self
            .patch(json!({ "events": events, "request_url": to.url }))
            .await;
        assert_eq!(status, StatusCode::OK, "{body:?}");
    }

    async fn add(&self, ch_id: Uuid, user_id: Uuid) {
        let (status, body) = send(
            &self.app,
            "POST",
            &format!("/api/channels/{ch_id}/members"),
            Some(&self.token),
            Some(json!({ "user_id": user_id })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body:?}");
    }

    async fn post(&self, token: &str, ch_id: Uuid, content: &str) -> Value {
        let (status, body) = send(
            &self.app,
            "POST",
            &format!("/api/channels/{ch_id}/messages"),
            Some(token),
            Some(json!({ "content": content })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body:?}");
        body
    }

    async fn fan_out(&self) {
        let rows = sqlx::query_as::<_, (Uuid, String, Value, DateTime<Utc>)>(
            "SELECT event_id, event_type, payload, created_at FROM event_outbox ORDER BY id",
        )
        .fetch_all(&self.state.pool)
        .await
        .expect("outbox");
        for (id, event_type, payload, timestamp) in rows {
            let event = shared_events::Event {
                id,
                event_type,
                payload,
                timestamp,
                stream_id: None,
            };
            assert!(consumer::fan_out(&self.state, &event).await);
        }
    }

    async fn queued(&self) -> Vec<Value> {
        sqlx::query_scalar::<_, Value>(
            "SELECT body FROM app_deliveries WHERE app_id = $1 ORDER BY seq",
        )
        .bind(self.app_id)
        .fetch_all(&self.state.pool)
        .await
        .expect("deliveries")
    }

    async fn send_due(&self) {
        sender::send_due(&self.state, &sender::client().expect("client")).await;
    }

    async fn make_due(&self) {
        sqlx::query("UPDATE app_deliveries SET next_attempt_at = NOW() WHERE app_id = $1")
            .bind(self.app_id)
            .execute(&self.state.pool)
            .await
            .expect("make due");
    }

    async fn signing_secret(&self) -> String {
        let (status, body) = send(
            &self.app,
            "GET",
            &format!("/api/bot-apps/{}/signing-secret", self.app_id),
            Some(&self.token),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body:?}");
        body["signing_secret"].as_str().expect("secret").to_string()
    }
}

fn of_type<'a>(queued: &'a [Value], kind: &str) -> Vec<&'a Value> {
    queued.iter().filter(|e| e["type"] == kind).collect()
}

fn in_channel(queued: &[Value], ch_id: Uuid) -> Vec<String> {
    queued
        .iter()
        .filter(|e| e["event"]["channel_id"] == json!(ch_id))
        .map(|e| e["type"].as_str().expect("type").to_string())
        .collect()
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_request_url_is_saved_only_once_it_answers_the_challenge(pool: PgPool) {
    let s = setup(pool, &["channels:history"]).await;

    let silent = endpoint(false).await;
    let (status, body) = s.patch(json!({ "request_url": silent.url })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body:?}");
    assert!(s.get().await["request_url"].is_null());

    let answering = endpoint(true).await;
    let (status, body) = s.patch(json!({ "request_url": answering.url })).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(s.get().await["request_url"], json!(answering.url));

    let (headers, challenge) = answering.seen.lock().expect("seen")[0].clone();
    assert_eq!(
        headers[SIGNATURE_HEADER].to_str().expect("header"),
        sign_body(&s.signing_secret().await, challenge.as_bytes()),
        "the challenge is signed the way every event will be"
    );

    let (member_id, _, member_token) =
        seed_and_login(&s.app, &s.state, "events-member", false).await;
    add_ws_member(&s.state, s.ws_id, member_id, "member").await;
    let (status, _) = send(
        &s.app,
        "GET",
        &format!("/api/bot-apps/{}/signing-secret", s.app_id),
        Some(&member_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn each_event_reaches_an_app_only_in_the_channels_it_is_in(pool: PgPool) {
    let s = setup(pool, &["channels:history", "channels:read", "chat:write"]).await;
    let to = endpoint(true).await;
    s.subscribe(
        &[
            "message",
            "app_mention",
            "reaction_added",
            "reaction_removed",
            "member_joined_channel",
            "channel_created",
            "channel_archived",
        ],
        &to,
    )
    .await;

    let joined = seed_channel(&s.state, s.ws_id, s.owner_id, "joined", false).await;
    let elsewhere = seed_channel(&s.state, s.ws_id, s.owner_id, "elsewhere", false).await;
    let private = seed_channel(&s.state, s.ws_id, s.owner_id, "private", true).await;
    s.add(joined, s.bot_user_id).await;
    s.add(private, s.bot_user_id).await;

    let mention = format!("deploy please @[Listener]({})", s.bot_user_id);
    let asked = s.post(&s.token, joined, &mention).await;
    s.post(&s.token, elsewhere, &mention).await;
    s.post(&s.token, private, "a secret").await;
    s.post(&s.bot_token, joined, "on it").await;

    let msg_id = asked["id"].as_str().expect("id");
    let reacted = |method: &'static str, uri: String| {
        let app = s.app.clone();
        let token = s.token.clone();
        async move {
            let body = (method == "POST").then(|| json!({ "emoji": "🚀" }));
            let (status, body) = send(&app, method, &uri, Some(&token), body).await;
            assert_eq!(status, StatusCode::OK, "{body:?}");
        }
    };
    reacted("POST", format!("/api/messages/{msg_id}/reactions")).await;
    reacted(
        "DELETE",
        format!("/api/messages/{msg_id}/reactions/{}", urlencode("🚀")),
    )
    .await;

    let colleague = seed_user(&s.state, &unique_email("events-colleague"), false).await;
    add_ws_member(&s.state, s.ws_id, colleague, "member").await;
    s.add(joined, colleague).await;

    let (status, created) = send(
        &s.app,
        "POST",
        &format!("/api/workspaces/{}/channels", s.ws_id),
        Some(&s.token),
        Some(json!({ "name": "fresh" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created:?}");
    let (status, hidden) = send(
        &s.app,
        "POST",
        &format!("/api/workspaces/{}/channels", s.ws_id),
        Some(&s.token),
        Some(json!({ "name": "hidden", "channel_type": "private" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{hidden:?}");

    let (status, body) = send(
        &s.app,
        "DELETE",
        &format!("/api/channels/{joined}"),
        Some(&s.token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");

    s.fan_out().await;
    let queued = s.queued().await;

    let heard = in_channel(&queued, joined);
    for kind in [
        "member_joined_channel",
        "message",
        "app_mention",
        "reaction_added",
        "reaction_removed",
        "channel_archived",
    ] {
        assert!(
            heard.iter().any(|k| k == kind),
            "{kind} missing from {heard:?}"
        );
    }
    assert_eq!(
        of_type(&queued, "message")
            .iter()
            .filter(|e| e["event"]["user_id"] == json!(s.bot_user_id))
            .count(),
        0,
        "an app does not hear its own messages"
    );
    assert_eq!(
        of_type(&queued, "member_joined_channel")
            .iter()
            .filter(|e| e["event"]["channel_id"] == json!(joined))
            .count(),
        2,
        "the bot and the colleague"
    );
    assert!(
        in_channel(&queued, elsewhere).is_empty(),
        "not a channel it is in"
    );
    assert_eq!(
        in_channel(&queued, private),
        vec!["member_joined_channel".to_string()],
        "a private channel's messages need groups:history"
    );
    let created_ids: Vec<&Value> = of_type(&queued, "channel_created")
        .iter()
        .map(|e| &e["event"]["channel_id"])
        .collect();
    assert_eq!(
        created_ids,
        vec![&created["id"]],
        "only the public one is announced"
    );

    for envelope in &queued {
        assert_eq!(envelope["workspace_id"], json!(s.ws_id));
        assert!(envelope["event_id"].is_string() && envelope["event_time"].is_string());
    }
    let mut ids: Vec<&str> = queued
        .iter()
        .map(|e| e["event_id"].as_str().expect("event_id"))
        .collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), queued.len(), "each event has an id of its own");

    s.fan_out().await;
    assert_eq!(
        s.queued().await.len(),
        queued.len(),
        "a stream redelivery queues nothing twice"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_failed_delivery_is_retried_in_order_and_marked_as_a_retry(pool: PgPool) {
    let s = setup(pool, &["channels:history"]).await;
    let to = endpoint(true).await;
    s.subscribe(&["message"], &to).await;
    let ch = seed_channel(&s.state, s.ws_id, s.owner_id, "deploys", false).await;
    s.add(ch, s.bot_user_id).await;

    to.answer(500);
    s.post(&s.token, ch, "first").await;
    s.fan_out().await;
    s.send_due().await;
    assert_eq!(to.events().len(), 1);
    assert!(to.events()[0].0.get(sender::RETRY_NUM_HEADER).is_none());

    s.post(&s.token, ch, "second").await;
    s.fan_out().await;
    s.send_due().await;
    assert_eq!(
        to.events().len(),
        1,
        "nothing is sent until the retry is due, and the second waits behind the first"
    );

    to.answer(200);
    s.make_due().await;
    s.send_due().await;
    let seen = to.events();
    assert_eq!(seen.len(), 2);
    let (headers, body) = &seen[1];
    assert_eq!(headers[sender::RETRY_NUM_HEADER], "1");
    assert_eq!(
        headers[SIGNATURE_HEADER].to_str().expect("header"),
        sign_body(&s.signing_secret().await, body.as_bytes())
    );
    let first: Value = serde_json::from_str(body).expect("json");
    assert_eq!(first["event"]["text"], "first");

    s.send_due().await;
    let seen = to.events();
    assert_eq!(seen.len(), 3, "then the second goes");
    let second: Value = serde_json::from_str(&seen[2].1).expect("json");
    assert_eq!(second["event"]["text"], "second");
    assert!(seen[2].0.get(sender::RETRY_NUM_HEADER).is_none());

    s.make_due().await;
    s.send_due().await;
    assert_eq!(to.events().len(), 3, "a delivered event is sent once");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_app_nothing_reaches_for_an_hour_is_shown_failing(pool: PgPool) {
    let s = setup(pool, &["channels:history"]).await;
    let to = endpoint(true).await;
    s.subscribe(&["message"], &to).await;
    let ch = seed_channel(&s.state, s.ws_id, s.owner_id, "alerts", false).await;
    s.add(ch, s.bot_user_id).await;

    to.answer(503);
    s.post(&s.token, ch, "disk full").await;
    s.fan_out().await;
    s.send_due().await;
    assert!(
        s.get().await["failing_since"].is_null(),
        "one miss is not failing"
    );

    sqlx::query("UPDATE app_deliveries SET expires_at = NOW() - INTERVAL '1 second'")
        .execute(&s.state.pool)
        .await
        .expect("age");
    sender::sweep(&s.state).await;
    let app = s.get().await;
    assert!(app["failing_since"].is_string(), "{app:?}");
    assert!(
        app["last_failure"].as_str().expect("why").contains("503"),
        "{app:?}"
    );

    to.answer(200);
    s.post(&s.token, ch, "disk fine").await;
    s.fan_out().await;
    s.send_due().await;
    assert!(
        s.get().await["failing_since"].is_null(),
        "a delivery clears it"
    );
}

async fn open_connection(s: &Setup, token: Option<&str>) -> (StatusCode, Value) {
    send(&s.app, "POST", "/api/apps/connections.open", token, None).await
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_app_token_opens_a_short_lived_socket_url(pool: PgPool) {
    let s = setup(pool, &["channels:history"]).await;

    let (status, _) = open_connection(&s, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = open_connection(&s, Some(&s.bot_token)).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "a bot token opens nothing"
    );

    let (member_id, _, member_token) =
        seed_and_login(&s.app, &s.state, "events-member", false).await;
    add_ws_member(&s.state, s.ws_id, member_id, "member").await;
    let (status, _) = send(
        &s.app,
        "POST",
        &format!("/api/bot-apps/{}/app-token", s.app_id),
        Some(&member_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let make = || async {
        let (status, body) = send(
            &s.app,
            "POST",
            &format!("/api/bot-apps/{}/app-token", s.app_id),
            Some(&s.token),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body:?}");
        body["app_token"].as_str().expect("token").to_string()
    };
    let app_token = make().await;
    assert!(app_token.starts_with("capp-"));
    assert_eq!(s.get().await["has_app_token"], true);

    let (status, _) = open_connection(&s, Some(&app_token)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "socket mode is off");

    let (status, body) = s.patch(json!({ "socket_mode": true })).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let (status, body) = open_connection(&s, Some(&app_token)).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let url = body["url"].as_str().expect("url");
    let ticket = url
        .strip_prefix("ws://localhost/ws/apps?ticket=")
        .expect("a socket url on this instance");
    let stored: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM app_sockets WHERE app_id = $1 AND ticket_hash = $2",
    )
    .bind(s.app_id)
    .bind(crate::bots::hash_token(ticket))
    .fetch_one(&s.state.pool)
    .await
    .expect("sockets");
    assert_eq!(stored, 1, "the ticket is kept hashed");

    let replaced = make().await;
    let (status, _) = open_connection(&s, Some(&app_token)).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "the old token stops at once"
    );
    let left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM app_sockets WHERE app_id = $1")
        .bind(s.app_id)
        .fetch_one(&s.state.pool)
        .await
        .expect("sockets");
    assert_eq!(left, 0, "its sockets are dropped with it");

    let (status, body) = send(
        &s.app,
        "DELETE",
        &format!("/api/bot-apps/{}/app-token", s.app_id),
        Some(&s.token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let (status, _) = open_connection(&s, Some(&replaced)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_socket_mode_app_takes_slash_commands_without_a_url(pool: PgPool) {
    let s = setup(pool, &["commands"]).await;
    let (status, body) = s.patch(json!({ "socket_mode": true })).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let ch = seed_channel(&s.state, s.ws_id, s.owner_id, "ops", false).await;

    let (status, body) = send(
        &s.app,
        "POST",
        &format!("/api/workspaces/{}/hooks", s.ws_id),
        Some(&s.token),
        Some(json!({
            "hook_type": "slash_command",
            "name": "deploy",
            "app_id": s.app_id,
            "config": { "command": "deploy", "channel_ids": [ch.to_string()] },
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");

    let uri = format!("/api/channels/{ch}/commands");
    let invoke = || {
        send(
            &s.app,
            "POST",
            &uri,
            Some(&s.token),
            Some(json!({ "command": "deploy", "text": "api" })),
        )
    };
    let (status, body) = invoke().await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body:?}");
    assert!(
        body.to_string().contains("could not be reached"),
        "{body:?}"
    );

    sqlx::query(
        "INSERT INTO app_sockets (app_id, connected_at, seen_at) VALUES ($1, NOW(), NOW())",
    )
    .bind(s.app_id)
    .execute(&s.state.pool)
    .await
    .expect("socket");

    let (status, body) = invoke().await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert!(body["text"]
        .as_str()
        .expect("text")
        .contains("working on it"));

    let (kind, payload) = sqlx::query_as::<_, (String, Value)>(
        "SELECT kind, body FROM app_deliveries WHERE app_id = $1",
    )
    .bind(s.app_id)
    .fetch_one(&s.state.pool)
    .await
    .expect("delivery");
    assert_eq!(kind, "slash_command");
    assert_eq!(payload["command"], "deploy");
    assert_eq!(payload["text"], "api");
    assert!(payload["response_url"]
        .as_str()
        .expect("response_url")
        .contains("/api/commands/responses/"));
}
