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

#[sqlx::test(migrations = "../migrations")]
async fn an_unknown_command_is_a_404_so_the_client_can_send_it_as_text(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "cmd-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Commands").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "general", false).await;

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

#[sqlx::test(migrations = "../migrations")]
async fn dnd_and_topic_are_built_in_and_change_real_state(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "cmd-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Commands").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "general", false).await;

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
            .is_dnd_active(owner_id)
            .await
            .expect("read dnd"),
        "the command set the state, not just the reply"
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

#[sqlx::test(migrations = "../migrations")]
async fn a_registered_command_is_called_and_its_answer_comes_back(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "cmd-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Commands").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "general", false).await;

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
#[sqlx::test(migrations = "../migrations")]
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

#[sqlx::test(migrations = "../migrations")]
async fn a_command_name_is_claimed_once_and_never_shadows_a_built_in(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "cmd-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Commands").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "general", false).await;
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

#[sqlx::test(migrations = "../migrations")]
async fn the_registry_lists_what_can_be_typed(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "cmd-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Commands").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "general", false).await;
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

#[sqlx::test(migrations = "../migrations")]
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

#[sqlx::test(migrations = "../migrations")]
async fn remind_creates_a_reminder_the_worker_will_deliver(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "cmd-remind", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Reminders").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "general", false).await;

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
        &format!("/api/workspaces/{ws_id}/reminders"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let reminders = listing["data"].as_array().expect("array");
    assert_eq!(reminders.len(), 1, "{listing:?}");
    assert_eq!(reminders[0]["content"], "stretch");
    assert_eq!(reminders[0]["channel_id"], ch_id.to_string());
}

#[sqlx::test(migrations = "../migrations")]
async fn remind_reads_a_clock_time_in_the_users_own_timezone(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "cmd-remind-tz", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Reminders").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "general", false).await;

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
        &format!("/api/workspaces/{ws_id}/reminders"),
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

#[sqlx::test(migrations = "../migrations")]
async fn remind_rejects_a_time_it_cannot_read(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "cmd-remind-bad", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Reminders").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "general", false).await;

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
        &format!("/api/workspaces/{ws_id}/reminders"),
        Some(&token),
        None,
    )
    .await;
    assert!(
        listing["data"].as_array().expect("array").is_empty(),
        "a command that failed leaves nothing behind"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn only_an_admin_reminds_somebody_else(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "cmd-remind-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Reminders").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "general", false).await;

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
        &format!("/api/workspaces/{ws_id}/reminders"),
        Some(&member_token),
        None,
    )
    .await;
    assert_eq!(listing["data"].as_array().expect("array").len(), 1);
}

#[sqlx::test(migrations = "../migrations")]
async fn a_reminder_can_be_cancelled_only_by_the_person_it_is_for(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "rem-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Reminders").await;

    let (other_id, _, other_token) = seed_and_login(&app, &state, "rem-other", false).await;
    add_ws_member(&state, ws_id, other_id, "member").await;

    let (status, created) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/reminders"),
        Some(&owner_token),
        Some(json!({
            "target_user_id": owner_id,
            "content": "stand up",
            "remind_at": (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339(),
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created:?}");
    let reminder_id = created["id"].as_str().expect("id");

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/workspaces/{ws_id}/reminders/{reminder_id}"),
        Some(&other_token),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "somebody else's reminder does not exist as far as you are concerned"
    );

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/workspaces/{ws_id}/reminders/{reminder_id}"),
        Some(&owner_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (_, listing) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/reminders"),
        Some(&owner_token),
        None,
    )
    .await;
    assert!(listing["data"].as_array().expect("array").is_empty());
}
