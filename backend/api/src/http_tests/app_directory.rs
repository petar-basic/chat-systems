use axum::http::StatusCode;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;
use crate::state::AppState;

async fn created(app: &axum::Router, token: &str, path: &str, body: Value) -> Value {
    let (status, body) = send(app, "POST", path, Some(token), Some(body)).await;
    assert_eq!(status, StatusCode::OK, "{path}: {body:?}");
    body
}

async fn directory(app: &axum::Router, token: &str, ws: Uuid) -> Value {
    let (status, body) = send(
        app,
        "GET",
        &format!("/api/workspaces/{ws}/app-directory"),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    body
}

fn entry<'a>(body: &'a Value, kind: &str, name: &str) -> &'a Value {
    body["installed"]
        .as_array()
        .expect("installed")
        .iter()
        .find(|item| item["kind"] == kind && item["name"] == name)
        .unwrap_or_else(|| panic!("{kind} {name} missing: {body:?}"))
}

fn listed(body: &Value, name: &str) -> bool {
    body["installed"]
        .as_array()
        .expect("installed")
        .iter()
        .any(|item| item["name"] == name)
}

fn channel_names(item: &Value) -> Vec<&str> {
    item["channels"]
        .as_array()
        .expect("channels")
        .iter()
        .map(|c| c["name"].as_str().expect("name"))
        .collect()
}

async fn notifications(app: &axum::Router, token: &str, ws: Uuid) -> Vec<Value> {
    let (status, body) = send(
        app,
        "GET",
        &format!("/api/workspaces/{ws}/notifications"),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    body["data"].as_array().expect("data").clone()
}

fn posts_into(channels: &[Uuid]) -> Vec<Value> {
    channels
        .iter()
        .map(|channel| json!({ "type": "send_message", "channel_id": channel, "text": "hello" }))
        .collect()
}

async fn workflow(
    app: &axum::Router,
    token: &str,
    ws: Uuid,
    name: &str,
    steps: Vec<Value>,
) -> Uuid {
    let body = created(
        app,
        token,
        &format!("/api/workspaces/{ws}/workflows"),
        json!({
            "name": name,
            "definition": {
                "trigger": { "type": "schedule", "days": ["mon"], "time": "09:00", "timezone": "UTC" },
                "steps": steps,
            },
        }),
    )
    .await;
    body["id"].as_str().expect("id").parse().expect("uuid")
}

async fn exec(state: &AppState, sql: &'static str, id: &str) {
    sqlx::query(sql)
        .bind(id)
        .execute(&state.pool)
        .await
        .expect(sql);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_directory_lists_every_kind_with_its_owner_and_health(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "dir-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Directory WS").await;
    let deploys = seed_channel(&state, ws, owner_id, "deploys", false).await;
    let hooks = format!("/api/workspaces/{ws}/hooks");

    let (status, body) = send(
        &app,
        "PUT",
        &format!("/api/workspaces/{ws}/apps/jira"),
        Some(&owner),
        Some(json!({ "client_id": "client-1", "client_secret": "jira-secret", "enabled": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");

    let bot = created(
        &app,
        &owner,
        &format!("/api/workspaces/{ws}/bot-apps"),
        json!({ "name": "Deploy bot", "scopes": ["chat:write", "channels:history"] }),
    )
    .await;
    let bot_id = bot["id"].as_str().expect("id").to_string();
    let bot_user: Uuid = bot["bot_user_id"]
        .as_str()
        .expect("bot user")
        .parse()
        .expect("uuid");
    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/channels/{deploys}/members"),
        Some(&owner),
        Some(json!({ "user_id": bot_user })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");

    created(
        &app,
        &owner,
        &hooks,
        json!({ "hook_type": "incoming_webhook", "name": "CI", "config": { "channel_id": deploys } }),
    )
    .await;
    let mirror = created(
        &app,
        &owner,
        &hooks,
        json!({
            "hook_type": "outgoing_webhook",
            "name": "Mirror",
            "config": { "url": "https://example.com/secret/path", "channel_ids": [deploys] }
        }),
    )
    .await;
    created(
        &app,
        &owner,
        &hooks,
        json!({
            "hook_type": "slash_command",
            "name": "Deployer",
            "config": { "command": "deploy", "url": "https://example.com/hidden/run", "channel_ids": [deploys] }
        }),
    )
    .await;
    let standup = workflow(&app, &owner, ws, "Standup", posts_into(&[deploys])).await;

    exec(
        &state,
        "UPDATE bot_apps SET failing_since = now(), last_failure = 'HTTP 500' WHERE id = $1::uuid",
        &bot_id,
    )
    .await;
    exec(
        &state,
        "UPDATE hooks SET is_active = false, disabled_reason = 'Its URL kept failing' WHERE id = $1::uuid",
        mirror["id"].as_str().expect("id"),
    )
    .await;
    exec(
        &state,
        "UPDATE workflows SET enabled = true WHERE id = $1::uuid",
        &standup.to_string(),
    )
    .await;
    exec(
        &state,
        "INSERT INTO workflow_runs (workflow_id, workflow_version, trigger_key, status, error)
         VALUES ($1::uuid, 1, 'directory-test', 'failed', 'The channel is gone')",
        &standup.to_string(),
    )
    .await;

    let body = directory(&app, &owner, ws).await;
    assert_eq!(body["can_manage"], true);

    let jira = entry(&body, "connected_app", "Jira Cloud");
    assert_eq!(jira["id"], "jira");
    assert_eq!(jira["added_by"], json!(owner_id));
    assert_eq!(jira["health"]["state"], "ok");
    assert!(!jira["permissions"]
        .as_array()
        .expect("permissions")
        .is_empty());

    let deploy_bot = entry(&body, "bot_app", "Deploy bot");
    assert_eq!(deploy_bot["added_by"], json!(owner_id));
    assert_eq!(deploy_bot["health"]["state"], "failing");
    assert_eq!(deploy_bot["health"]["reason"], "HTTP 500");
    assert_eq!(channel_names(deploy_bot), ["deploys"]);
    assert_eq!(
        deploy_bot["permissions"][0],
        json!({ "key": "chat:write", "text": "Post, edit and delete its own messages where it is a member" })
    );

    let ci = entry(&body, "incoming_webhook", "CI");
    assert_eq!(ci["health"]["state"], "ok");
    assert_eq!(channel_names(ci), ["deploys"]);

    let outgoing = entry(&body, "outgoing_webhook", "Mirror");
    assert_eq!(outgoing["health"]["state"], "disabled");
    assert_eq!(outgoing["health"]["reason"], "Its URL kept failing");
    assert!(outgoing["permissions"][0]["text"]
        .as_str()
        .expect("text")
        .ends_with("to example.com"));

    let command = entry(&body, "slash_command", "Deployer");
    assert_eq!(command["every_channel"], false);
    assert_eq!(channel_names(command), ["deploys"]);
    assert_eq!(command["added_by"], json!(owner_id));

    let flow = entry(&body, "workflow", "Standup");
    assert_eq!(flow["health"]["state"], "failing");
    assert_eq!(flow["health"]["reason"], "The channel is gone");
    assert_eq!(channel_names(flow), ["deploys"]);

    let available = body["available"].as_array().expect("available");
    assert_eq!(available.len(), 5);
    let jira_card = available
        .iter()
        .find(|a| a["provider"] == "jira")
        .expect("jira card");
    assert_eq!(jira_card["installed"], true);
    assert_eq!(body["bot_scopes"].as_array().expect("scopes").len(), 10);

    let text = body.to_string();
    for secret in ["jira-secret", "client-1", "/secret/path", "/hidden/run"] {
        assert!(!text.contains(secret), "{secret} leaked: {text}");
    }
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_member_sees_what_touches_the_channels_they_can_see(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "dir-vis-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Directory visibility WS").await;
    let (member_id, _, member) = seed_and_login(&app, &state, "dir-vis-member", false).await;
    add_ws_member(&state, ws, member_id, "member").await;
    let general = seed_channel(&state, ws, owner_id, "general-dir", false).await;
    let secret = seed_channel(&state, ws, owner_id, "secret-dir", true).await;
    let hooks = format!("/api/workspaces/{ws}/hooks");

    for (name, channel) in [("Public CI", general), ("Secret CI", secret)] {
        created(
            &app,
            &owner,
            &hooks,
            json!({ "hook_type": "incoming_webhook", "name": name, "config": { "channel_id": channel } }),
        )
        .await;
    }
    workflow(&app, &owner, ws, "Both", posts_into(&[general, secret])).await;
    workflow(&app, &owner, ws, "Only secret", posts_into(&[secret])).await;
    workflow(
        &app,
        &member,
        ws,
        "Mine",
        vec![json!({
            "type": "send_dm",
            "to": { "kind": "person", "user_id": owner_id },
            "text": "hello",
        })],
    )
    .await;

    let body = directory(&app, &member, ws).await;
    assert_eq!(body["can_manage"], false);
    assert!(listed(&body, "Public CI"));
    assert!(!listed(&body, "Secret CI"));
    assert!(!listed(&body, "Only secret"));
    assert!(listed(&body, "Mine"));
    let both = entry(&body, "workflow", "Both");
    assert_eq!(channel_names(both), ["general-dir"]);
    assert_eq!(both["hidden_channels"], 1);
    assert!(!body.to_string().contains("secret-dir"));

    let body = directory(&app, &owner, ws).await;
    assert!(listed(&body, "Secret CI"));
    assert!(listed(&body, "Mine"));
    assert_eq!(entry(&body, "workflow", "Both")["hidden_channels"], 0);

    let (_, _, outsider) = seed_and_login(&app, &state, "dir-vis-outsider", false).await;
    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/app-directory"),
        Some(&outsider),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_request_lands_with_every_admin(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "dir-req-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Directory request WS").await;
    let (admin_id, _, admin) = seed_and_login(&app, &state, "dir-req-admin", false).await;
    add_ws_member(&state, ws, admin_id, "admin").await;
    let (member_id, _, member) = seed_and_login(&app, &state, "dir-req-member", false).await;
    add_ws_member(&state, ws, member_id, "member").await;
    let (guest_id, _, guest) = seed_and_login(&app, &state, "dir-req-guest", false).await;
    add_ws_member(&state, ws, guest_id, "guest").await;
    let requests = format!("/api/workspaces/{ws}/app-requests");

    let (status, body) = send(
        &app,
        "POST",
        &requests,
        Some(&member),
        Some(json!({ "name": "PagerDuty", "reason": "On-call alerts in #ops" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");

    for token in [&owner, &admin] {
        let seen = notifications(&app, token, ws).await;
        let request = seen
            .iter()
            .find(|n| n["data"]["app_request"] == "PagerDuty")
            .unwrap_or_else(|| panic!("no request in {seen:?}"));
        assert_eq!(request["notification_type"], "system");
        assert!(request["title"]
            .as_str()
            .expect("title")
            .ends_with("asked for an app"));
        assert_eq!(request["body"], "PagerDuty: On-call alerts in #ops");
        assert_eq!(request["data"]["requested_by"], json!(member_id));
    }
    assert!(notifications(&app, &member, ws).await.is_empty());

    let (status, _) = send(
        &app,
        "POST",
        &requests,
        Some(&guest),
        Some(json!({ "name": "Anything" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = send(
        &app,
        "POST",
        &requests,
        Some(&member),
        Some(json!({ "name": "   " })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}
