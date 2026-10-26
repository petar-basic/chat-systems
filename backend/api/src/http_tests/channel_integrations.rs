use axum::http::StatusCode;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;
use crate::state::AppState;
use crate::workspace::models::ChannelRole;

async fn created(app: &axum::Router, token: &str, path: &str, body: Value) -> Value {
    let (status, body) = send(app, "POST", path, Some(token), Some(body)).await;
    assert_eq!(status, StatusCode::OK, "{path}: {body:?}");
    body
}

async fn integrations(app: &axum::Router, token: &str, channel: Uuid) -> (StatusCode, Value) {
    send(
        app,
        "GET",
        &format!("/api/channels/{channel}/integrations"),
        Some(token),
        None,
    )
    .await
}

async fn join(state: &AppState, channel: Uuid, user: Uuid, role: ChannelRole) {
    state
        .workspace_service
        .repo
        .add_channel_member(channel, user, &role)
        .await
        .unwrap();
}

fn incoming(channel: Uuid, name: &str) -> Value {
    json!({ "hook_type": "incoming_webhook", "name": name, "config": { "channel_id": channel } })
}

fn names(list: &Value) -> Vec<&str> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|item| item["name"].as_str().unwrap())
        .collect()
}

async fn audited(state: &AppState, hook: &str, user: Uuid) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT action FROM audit_log WHERE resource_id = $1::uuid AND user_id = $2 ORDER BY created_at",
    )
    .bind(hook)
    .bind(user)
    .fetch_all(&state.pool)
    .await
    .unwrap()
}

#[test_macros::db_test(migrations = "../migrations")]
async fn everyone_in_a_channel_sees_what_touches_it_and_no_secret(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "ints-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Integrations WS").await;
    let (member_id, _, member) = seed_and_login(&app, &state, "ints-member", false).await;
    add_ws_member(&state, ws, member_id, "member").await;
    let deploys = seed_channel(&state, ws, owner_id, "deploys", false).await;
    let elsewhere = seed_channel(&state, ws, owner_id, "elsewhere", false).await;
    let hooks = format!("/api/workspaces/{ws}/hooks");
    let workflows = format!("/api/workspaces/{ws}/workflows");

    let ci = created(&app, &owner, &hooks, incoming(deploys, "CI")).await;
    created(&app, &owner, &hooks, incoming(elsewhere, "Other CI")).await;
    let outgoing = created(
        &app,
        &owner,
        &hooks,
        json!({
            "hook_type": "outgoing_webhook",
            "name": "Mirror",
            "config": { "url": "https://example.com/out", "channel_ids": [deploys, elsewhere] }
        }),
    )
    .await;
    let command = created(
        &app,
        &owner,
        &hooks,
        json!({
            "hook_type": "slash_command",
            "name": "Deployer",
            "config": { "command": "deploy", "url": "https://example.com/cmd", "channel_ids": [deploys] }
        }),
    )
    .await;
    created(
        &app,
        &owner,
        &workflows,
        json!({
            "name": "Welcome",
            "definition": {
                "trigger": { "type": "member_joined", "channel_id": deploys },
                "steps": [{ "type": "send_dm", "to": { "kind": "trigger_user" }, "text": "hi" }],
            },
        }),
    )
    .await;
    created(
        &app,
        &owner,
        &workflows,
        json!({
            "name": "Standup",
            "definition": {
                "trigger": { "type": "schedule", "days": ["mon"], "time": "09:00", "timezone": "UTC" },
                "steps": [{ "type": "send_message", "channel_id": deploys, "text": "standup" }],
            },
        }),
    )
    .await;
    created(
        &app,
        &owner,
        &workflows,
        json!({
            "name": "Elsewhere",
            "definition": {
                "trigger": { "type": "member_joined", "channel_id": elsewhere },
                "steps": [{ "type": "send_message", "channel_id": elsewhere, "text": "x" }],
            },
        }),
    )
    .await;

    let (status, body) = integrations(&app, &member, deploys).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(names(&body["incoming_webhooks"]), ["CI"]);
    assert_eq!(body["incoming_webhooks"][0]["created_by"], json!(owner_id));
    assert_eq!(names(&body["outgoing_webhooks"]), ["Mirror"]);
    assert_eq!(body["slash_commands"][0]["command"], "deploy");
    assert_eq!(names(&body["workflows"]), ["Standup", "Welcome"]);
    assert_eq!(body["can_add_webhook"], false);

    let text = body.to_string();
    for secret in [
        ci["config"]["token"].as_str().unwrap(),
        outgoing["config"]["secret"].as_str().unwrap(),
        command["config"]["secret"].as_str().unwrap(),
        "example.com",
        "/hooks/incoming/",
    ] {
        assert!(!text.contains(secret), "{secret} leaked: {text}");
    }

    let (_, owners) = integrations(&app, &owner, deploys).await;
    assert_eq!(owners["can_add_webhook"], true);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_private_channel_shows_its_integrations_only_to_its_members(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "ints-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Private WS").await;
    let (member_id, _, member) = seed_and_login(&app, &state, "ints-member", false).await;
    add_ws_member(&state, ws, member_id, "member").await;
    let secret = seed_channel(&state, ws, owner_id, "leadership", true).await;
    created(
        &app,
        &owner,
        &format!("/api/workspaces/{ws}/hooks"),
        incoming(secret, "Board"),
    )
    .await;

    let (status, _) = integrations(&app, &member, secret).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    join(&state, secret, member_id, ChannelRole::Member).await;
    let (status, body) = integrations(&app, &member, secret).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(names(&body["incoming_webhooks"]), ["Board"]);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_channel_admin_runs_the_incoming_webhooks_of_their_channel(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _) = seed(&state, "ints-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Channel Admin WS").await;
    let (lead_id, _, lead) = seed_and_login(&app, &state, "ints-lead", false).await;
    add_ws_member(&state, ws, lead_id, "member").await;
    let (member_id, _, member) = seed_and_login(&app, &state, "ints-member", false).await;
    add_ws_member(&state, ws, member_id, "member").await;
    let deploys = seed_channel(&state, ws, owner_id, "deploys", false).await;
    let other = seed_channel(&state, ws, owner_id, "other", false).await;
    join(&state, deploys, lead_id, ChannelRole::Admin).await;
    join(&state, deploys, member_id, ChannelRole::Member).await;
    let hooks = format!("/api/workspaces/{ws}/hooks");

    let (_, body) = integrations(&app, &lead, deploys).await;
    assert_eq!(body["can_add_webhook"], true);

    let hook = created(&app, &lead, &hooks, incoming(deploys, "CI")).await;
    let hook_id = hook["id"].as_str().unwrap();
    let old_token = hook["config"]["token"].as_str().unwrap().to_string();

    let rotated = created(
        &app,
        &lead,
        &format!("/api/hooks/{hook_id}/rotate"),
        json!({}),
    )
    .await;
    let new_token = rotated["config"]["token"].as_str().unwrap();
    assert_ne!(new_token, old_token);
    let revealed = created(
        &app,
        &lead,
        &format!("/api/hooks/{hook_id}/reveal"),
        json!({}),
    )
    .await;
    assert!(revealed["incoming_url"]
        .as_str()
        .unwrap()
        .ends_with(new_token));

    for route in ["reveal", "rotate"] {
        let (status, _) = send(
            &app,
            "POST",
            &format!("/api/hooks/{hook_id}/{route}"),
            Some(&member),
            None,
        )
        .await;
        assert_eq!(
            status,
            StatusCode::FORBIDDEN,
            "a plain member cannot {route}"
        );
    }
    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/hooks/{hook_id}"),
        Some(&member),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = send(
        &app,
        "POST",
        &hooks,
        Some(&member),
        Some(incoming(deploys, "Mine")),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a plain member cannot add one"
    );

    let (status, _) = send(
        &app,
        "POST",
        &hooks,
        Some(&lead),
        Some(incoming(other, "Theirs")),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "not for a channel they do not run"
    );
    let (status, _) = send(
        &app,
        "POST",
        &hooks,
        Some(&lead),
        Some(json!({
            "hook_type": "outgoing_webhook",
            "name": "Mirror",
            "config": { "url": "https://example.com/out", "channel_ids": [deploys] }
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "outgoing webhooks stay with workspace admins"
    );
    let (status, _) = send(
        &app,
        "POST",
        &hooks,
        Some(&lead),
        Some(json!({
            "hook_type": "slash_command",
            "name": "Deployer",
            "config": { "command": "deploy", "url": "https://example.com/cmd", "channel_ids": [deploys] }
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "slash commands stay with workspace admins"
    );

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/hooks/{hook_id}"),
        Some(&lead),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    assert_eq!(
        audited(&state, hook_id, lead_id).await,
        [
            "hook.created",
            "hook.rotated",
            "hook.revealed",
            "hook.deleted"
        ]
    );
}
