use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;

/// Stands in for whatever a team points a command at. Records the body so the
/// signature and the payload can be asserted, not just the status code.
async fn fake_command_endpoint(response: serde_json::Value) -> (String, Arc<AtomicUsize>) {
    let hits = Arc::new(AtomicUsize::new(0));
    let counter = hits.clone();

    let app = axum::Router::new().fallback(move |body: String| {
        let counter = counter.clone();
        let response = response.clone();
        async move {
            counter.fetch_add(1, Ordering::SeqCst);
            let _ = body;
            axum::Json(response)
        }
    });

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind the fake command endpoint");
    let address = format!("http://{}", listener.local_addr().expect("addr"));
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    (address, hits)
}

async fn register_command(
    app: &axum::Router,
    token: &str,
    ws_id: Uuid,
    command: &str,
    url: &str,
    channel_ids: Vec<Uuid>,
) -> (StatusCode, serde_json::Value) {
    send(
        app,
        "POST",
        &format!("/api/workspaces/{ws_id}/hooks"),
        Some(token),
        Some(json!({
            "hook_type": "slash_command",
            "name": command,
            "config": {
                "command": command,
                "url": url,
                "channel_ids": channel_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>(),
            }
        })),
    )
    .await
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_unknown_command_is_a_404_so_the_client_can_send_it_as_text(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "cmd-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Commands").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "main", false).await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/commands"),
        Some(&token),
        Some(json!({ "command": "nosuchthing", "text": "" })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "a typo must not vanish into an error"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn dnd_and_topic_are_built_in_and_change_real_state(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "cmd-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Commands").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "main", false).await;

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/commands"),
        Some(&token),
        Some(json!({ "command": "/dnd", "text": "30" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(
        body["response_type"], "ephemeral",
        "only the person who ran it"
    );
    assert!(
        state
            .notification_repo
            .is_dnd_active(ws_id, owner_id)
            .await
            .expect("read dnd"),
        "the command set the state, not just the reply"
    );
    let elsewhere = seed_workspace(&state, owner_id, "Elsewhere").await;
    assert!(
        !state
            .notification_repo
            .is_dnd_active(elsewhere, owner_id)
            .await
            .expect("read dnd"),
        "it pauses the workspace the channel is in"
    );

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/commands"),
        Some(&token),
        Some(json!({ "command": "topic", "text": "release week" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(
        body["response_type"], "in_channel",
        "a topic change is everybody's business"
    );

    let channel = state
        .workspace_service
        .repo
        .find_channel_by_id(ch_id)
        .await
        .expect("query")
        .expect("channel");
    assert_eq!(channel.topic.as_deref(), Some("release week"));

    let posted: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM messages WHERE channel_id = $1 AND metadata ? 'bot'",
    )
    .bind(ch_id)
    .fetch_one(&state.pool)
    .await
    .expect("count");
    assert_eq!(
        posted, 1,
        "the in-channel answer is a message from the command"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_registered_command_is_called_and_its_answer_comes_back(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "cmd-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Commands").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "main", false).await;

    let (endpoint, hits) =
        fake_command_endpoint(json!({ "response_type": "ephemeral", "text": "deploying prod" }))
            .await;

    let (status, created) =
        register_command(&app, &token, ws_id, "deploy", &endpoint, vec![ch_id]).await;
    assert_eq!(status, StatusCode::OK, "{created:?}");

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/commands"),
        Some(&token),
        Some(json!({ "command": "deploy", "text": "prod" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["text"], "deploying prod");
    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "called once, with nobody retrying"
    );

    let audited: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM audit_log WHERE action = 'command.invoked'")
            .fetch_one(&state.pool)
            .await
            .expect("count audit");
    assert_eq!(audited, 1);
}

/// The scoping from CS-019 applies here for the same reason it applies there:
/// invoking a command sends what somebody typed to a third-party URL.
#[test_macros::db_test(migrations = "../migrations")]
async fn a_command_is_refused_in_a_channel_it_was_not_enabled_for(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "cmd-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Commands").await;
    let enabled = seed_channel(&state, ws_id, owner_id, "ops", false).await;
    let other = seed_channel(&state, ws_id, owner_id, "random", false).await;

    let (endpoint, hits) = fake_command_endpoint(json!({ "text": "ok" })).await;
    let (status, created) =
        register_command(&app, &token, ws_id, "deploy", &endpoint, vec![enabled]).await;
    assert_eq!(status, StatusCode::OK, "{created:?}");

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{other}/commands"),
        Some(&token),
        Some(json!({ "command": "deploy", "text": "prod" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(hits.load(Ordering::SeqCst), 0, "nothing left the instance");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_command_name_is_claimed_once_and_never_shadows_a_built_in(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "cmd-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Commands").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "main", false).await;
    let (endpoint, _) = fake_command_endpoint(json!({ "text": "ok" })).await;

    let (status, _) = register_command(&app, &token, ws_id, "deploy", &endpoint, vec![ch_id]).await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = register_command(&app, &token, ws_id, "deploy", &endpoint, vec![ch_id]).await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "/deploy has to mean one thing"
    );

    let (status, _) = register_command(&app, &token, ws_id, "dnd", &endpoint, vec![ch_id]).await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "a built-in is not available"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_registry_lists_what_can_be_typed(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "cmd-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Commands").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "main", false).await;
    let (endpoint, _) = fake_command_endpoint(json!({ "text": "ok" })).await;
    register_command(&app, &token, ws_id, "deploy", &endpoint, vec![ch_id]).await;

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/commands"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let names: Vec<&str> = body["data"]
        .as_array()
        .expect("data")
        .iter()
        .filter_map(|c| c["command"].as_str())
        .collect();
    assert!(
        names.contains(&"dnd"),
        "built-ins are discoverable: {names:?}"
    );
    assert!(
        names.contains(&"deploy"),
        "so is what the workspace registered"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_command_cannot_be_run_from_outside_the_channel(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "cmd-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Commands").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "private-ops", true).await;

    let (outsider_id, _, outsider_token) = seed_and_login(&app, &state, "cmd-out", false).await;
    add_ws_member(&state, ws_id, outsider_id, "member").await;
    let _ = owner_token;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/commands"),
        Some(&outsider_token),
        Some(json!({ "command": "topic", "text": "mine now" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/commands"),
        None,
        Some(json!({ "command": "topic", "text": "" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn remind_creates_a_reminder_the_worker_will_deliver(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "cmd-remind", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Reminders").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "main", false).await;

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/commands"),
        Some(&token),
        Some(json!({ "command": "/remind", "text": "me in 30m to stretch" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["response_type"], "ephemeral");

    let (status, listing) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/later"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let reminders = listing["data"].as_array().expect("array");
    assert_eq!(reminders.len(), 1, "{listing:?}");
    assert_eq!(reminders[0]["text"], "stretch");
    assert_eq!(reminders[0]["channel_id"], ch_id.to_string());
}

#[test_macros::db_test(migrations = "../migrations")]
async fn remind_reads_a_clock_time_in_the_users_own_timezone(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "cmd-remind-tz", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Reminders").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "main", false).await;

    let (status, _) = send(
        &app,
        "PATCH",
        "/api/users/me",
        Some(&token),
        Some(json!({ "timezone": "Pacific/Kiritimati" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/commands"),
        Some(&token),
        Some(json!({ "command": "/remind", "text": "me tomorrow at 9am to file the report" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");

    let (_, listing) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/later"),
        Some(&token),
        None,
    )
    .await;
    let remind_at: chrono::DateTime<chrono::Utc> = listing["data"][0]["remind_at"]
        .as_str()
        .expect("remind_at")
        .parse()
        .expect("timestamp");
    // UTC+14: 09:00 there is 19:00 UTC the day before, so a naive UTC reading
    // of "9am" would be off by most of a day.
    assert_eq!(
        remind_at.format("%H:%M").to_string(),
        "19:00",
        "{listing:?}"
    );
    assert!(
        remind_at > chrono::Utc::now(),
        "a reminder is in the future"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn remind_rejects_a_time_it_cannot_read(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "cmd-remind-bad", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Reminders").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "main", false).await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/commands"),
        Some(&token),
        Some(json!({ "command": "/remind", "text": "me sometime to stretch" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (_, listing) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/later"),
        Some(&token),
        None,
    )
    .await;
    assert!(
        listing["data"].as_array().expect("array").is_empty(),
        "a command that failed leaves nothing behind"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn only_an_admin_reminds_somebody_else(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "cmd-remind-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Reminders").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "main", false).await;

    let (member_id, _, member_token) =
        seed_and_login(&app, &state, "cmd-remind-member", false).await;
    add_ws_member(&state, ws_id, member_id, "member").await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/commands"),
        Some(&member_token),
        Some(json!({
            "command": "/remind",
            "text": format!("@[Owner]({owner_id}) in 1h to review"),
        })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/commands"),
        Some(&owner_token),
        Some(json!({
            "command": "/remind",
            "text": format!("@[Member]({member_id}) in 1h to review"),
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (_, listing) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/later"),
        Some(&member_token),
        None,
    )
    .await;
    assert_eq!(listing["data"].as_array().expect("array").len(), 1);
}

type Recorded = Arc<std::sync::Mutex<Vec<(Option<String>, serde_json::Value)>>>;

async fn scripted_command_endpoint(script: Vec<(u16, &'static str)>) -> (String, Recorded) {
    let calls: Recorded = Arc::default();
    let seen = calls.clone();
    let script = Arc::new(script);

    let app = axum::Router::new().fallback(move |headers: axum::http::HeaderMap, body: String| {
        let seen = seen.clone();
        let script = script.clone();
        async move {
            let mut calls = seen.lock().expect("calls");
            calls.push((
                headers
                    .get("x-chat-request-id")
                    .and_then(|v| v.to_str().ok())
                    .map(str::to_string),
                serde_json::from_str(&body).unwrap_or_default(),
            ));
            let (status, reply) = script[(calls.len() - 1).min(script.len() - 1)];
            (StatusCode::from_u16(status).expect("status"), reply)
        }
    });

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind the scripted command endpoint");
    let address = format!("http://{}", listener.local_addr().expect("addr"));
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (address, calls)
}

async fn closed_port() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let address = format!("http://{}", listener.local_addr().expect("addr"));
    drop(listener);
    address
}

struct Commanded {
    app: axum::Router,
    state: Arc<crate::state::AppState>,
    caller: Uuid,
    token: String,
    ch_id: Uuid,
}

async fn commanded(pool: PgPool, url: &str) -> Commanded {
    let (app, state) = app_and_state(pool).await;
    let (caller, _, token) = seed_and_login(&app, &state, "cmd-later", false).await;
    let ws_id = seed_workspace(&state, caller, &format!("Later {}", Uuid::new_v4())).await;
    let ch_id = seed_channel(&state, ws_id, caller, "main", false).await;
    let (status, created) = register_command(&app, &token, ws_id, "deploy", url, vec![ch_id]).await;
    assert_eq!(status, StatusCode::OK, "{created:?}");
    Commanded {
        app,
        state,
        caller,
        token,
        ch_id,
    }
}

async fn run(c: &Commanded) -> (StatusCode, serde_json::Value) {
    send(
        &c.app,
        "POST",
        &format!("/api/channels/{}/commands", c.ch_id),
        Some(&c.token),
        Some(json!({ "command": "deploy", "text": "prod" })),
    )
    .await
}

async fn answer_later(c: &Commanded, url: &str, body: serde_json::Value) -> StatusCode {
    let path = url
        .split_once("/api/")
        .map(|(_, rest)| format!("/api/{rest}"))
        .expect("an /api/ url");
    send(&c.app, "POST", &path, None, Some(body)).await.0
}

async fn bot_messages(c: &Commanded) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT content FROM messages WHERE channel_id = $1 AND metadata ? 'bot' ORDER BY created_at",
    )
    .bind(c.ch_id)
    .fetch_all(&c.state.pool)
    .await
    .expect("bot messages")
}

async fn retry_now(c: &Commanded) {
    sqlx::query("UPDATE command_invocations SET retry_at = NOW() - INTERVAL '1 second' WHERE retry_at IS NOT NULL")
        .execute(&c.state.pool)
        .await
        .expect("make the retry due");
    crate::commands::delivery::retry_due(&c.state).await;
}

async fn command_events() -> impl futures_util::Stream<Item = redis::Msg> + Unpin {
    let url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into());
    let mut pubsub = redis::Client::open(url)
        .expect("redis client")
        .get_async_pubsub()
        .await
        .expect("pubsub");
    pubsub.subscribe("events:command").await.expect("subscribe");
    pubsub.into_on_message()
}

async fn answer_for(
    events: &mut (impl futures_util::Stream<Item = redis::Msg> + Unpin),
    user_id: Uuid,
) -> Option<serde_json::Value> {
    use futures_util::StreamExt;
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        let message = tokio::time::timeout_at(deadline, events.next())
            .await
            .ok()??;
        let Ok(raw) = message.get_payload::<String>() else {
            continue;
        };
        let Ok(event) = serde_json::from_str::<serde_json::Value>(&raw) else {
            continue;
        };
        if event["payload"]["user_id"] == user_id.to_string() {
            return Some(event["payload"].clone());
        }
    }
}

fn response_url_of(calls: &Recorded) -> String {
    calls.lock().expect("calls")[0].1["response_url"]
        .as_str()
        .expect("the request carries a response_url")
        .to_string()
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_command_that_acknowledges_answers_later_in_the_channel(pool: PgPool) {
    let (url, calls) = scripted_command_endpoint(vec![(200, "")]).await;
    let c = commanded(pool, &url).await;

    let (status, body) = run(&c).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["response_type"], "pending");
    assert!(bot_messages(&c).await.is_empty());

    let response_url = response_url_of(&calls);
    assert_eq!(
        answer_later(
            &c,
            &response_url,
            json!({ "text": "deploying", "response_type": "in_channel" })
        )
        .await,
        StatusCode::OK
    );
    assert_eq!(bot_messages(&c).await, vec!["deploying".to_string()]);

    assert_eq!(
        answer_later(
            &c,
            &response_url,
            json!({ "text": "deployed", "response_type": "in_channel", "replace_original": true })
        )
        .await,
        StatusCode::OK
    );
    assert_eq!(
        bot_messages(&c).await,
        vec!["deployed".to_string()],
        "replace_original edits the answer instead of adding one"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_ephemeral_answer_later_reaches_only_the_caller(pool: PgPool) {
    let (url, calls) = scripted_command_endpoint(vec![(200, "")]).await;
    let c = commanded(pool, &url).await;
    let mut events = command_events().await;

    run(&c).await;
    assert_eq!(
        answer_later(&c, &response_url_of(&calls), json!({ "text": "only you" })).await,
        StatusCode::OK
    );

    let event = answer_for(&mut events, c.caller)
        .await
        .expect("the caller's sockets are told");
    assert_eq!(event["response_type"], "ephemeral");
    assert_eq!(event["text"], "only you");
    assert_eq!(event["channel_id"], c.ch_id.to_string());
    assert!(bot_messages(&c).await.is_empty(), "nothing is posted");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_response_url_takes_five_answers_for_thirty_minutes(pool: PgPool) {
    let (url, calls) = scripted_command_endpoint(vec![(200, "")]).await;
    let c = commanded(pool, &url).await;
    run(&c).await;
    let response_url = response_url_of(&calls);

    for n in 1..=5 {
        assert_eq!(
            answer_later(&c, &response_url, json!({ "text": format!("answer {n}") })).await,
            StatusCode::OK
        );
    }
    assert_eq!(
        answer_later(&c, &response_url, json!({ "text": "sixth" })).await,
        StatusCode::NOT_FOUND,
        "five uses and no more"
    );

    run(&c).await;
    let second = calls.lock().expect("calls")[1].1["response_url"]
        .as_str()
        .expect("url")
        .to_string();
    sqlx::query("UPDATE command_invocations SET expires_at = NOW() - INTERVAL '1 second'")
        .execute(&c.state.pool)
        .await
        .expect("expire");
    assert_eq!(
        answer_later(&c, &second, json!({ "text": "too late" })).await,
        StatusCode::NOT_FOUND,
        "thirty minutes and no longer"
    );

    let last = if second.ends_with('x') { 'y' } else { 'x' };
    let forged = format!("{}{last}", &second[..second.len() - 1]);
    assert_eq!(
        answer_later(&c, &forged, json!({ "text": "forged" })).await,
        StatusCode::NOT_FOUND
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_503_is_retried_with_the_same_request_id(pool: PgPool) {
    let (url, calls) = scripted_command_endpoint(vec![
        (503, ""),
        (200, r#"{"response_type":"ephemeral","text":"deployed"}"#),
    ])
    .await;
    let c = commanded(pool, &url).await;
    let mut events = command_events().await;

    let (status, body) = run(&c).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["response_type"], "pending");
    assert!(body["text"]
        .as_str()
        .unwrap_or_default()
        .contains("taking a moment"));

    retry_now(&c).await;

    let calls = calls.lock().expect("calls").clone();
    assert_eq!(calls.len(), 2);
    assert!(calls[0].0.is_some());
    assert_eq!(
        calls[0].0, calls[1].0,
        "the app can drop a repeat by its id"
    );
    assert_eq!(
        calls[0].1["request_id"].as_str(),
        calls[0].0.as_deref(),
        "the id travels in the body too"
    );

    let event = answer_for(&mut events, c.caller)
        .await
        .expect("the answer reaches the caller");
    assert_eq!(event["text"], "deployed");
    let pending: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM command_invocations WHERE retry_at IS NOT NULL")
            .fetch_one(&c.state.pool)
            .await
            .expect("count");
    assert_eq!(pending, 0);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_unreachable_command_is_tried_three_times_then_reported(pool: PgPool) {
    let c = commanded(pool, &closed_port().await).await;
    let mut events = command_events().await;

    let (status, body) = run(&c).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["response_type"], "pending");

    retry_now(&c).await;
    let attempts: i32 = sqlx::query_scalar("SELECT attempts FROM command_invocations")
        .fetch_one(&c.state.pool)
        .await
        .expect("attempts");
    assert_eq!(attempts, 2);

    retry_now(&c).await;
    let (attempts, retry_at): (i32, Option<chrono::DateTime<chrono::Utc>>) =
        sqlx::query_as("SELECT attempts, retry_at FROM command_invocations")
            .fetch_one(&c.state.pool)
            .await
            .expect("attempts");
    assert_eq!(attempts, 3);
    assert!(retry_at.is_none(), "two retries and no more");

    let event = answer_for(&mut events, c.caller)
        .await
        .expect("the caller is told");
    assert_eq!(event["response_type"], "error");
    assert!(event["text"]
        .as_str()
        .unwrap_or_default()
        .contains("could not be reached"));
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_400_or_a_500_is_an_answer_not_a_retry(pool: PgPool) {
    for status in [400u16, 500] {
        let (url, calls) = scripted_command_endpoint(vec![(status, "no")]).await;
        let c = commanded(pool.clone(), &url).await;

        let (got, body) = run(&c).await;
        assert_eq!(got, StatusCode::BAD_REQUEST, "{status}: {body:?}");
        retry_now(&c).await;
        assert_eq!(
            calls.lock().expect("calls").len(),
            1,
            "{status} is not retried"
        );
    }
}
