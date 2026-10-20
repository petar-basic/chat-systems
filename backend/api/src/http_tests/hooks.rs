use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

use super::common::*;

#[test_macros::db_test(migrations = "../migrations")]
async fn owner_can_create_hook(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "hook-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Hooks WS").await;
    let ch = seed_channel(&state, ws, owner_id, "deploys", false).await;

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/hooks"),
        Some(&token),
        Some(json!({
            "hook_type": "incoming_webhook",
            "name": "deploy-bot",
            "description": "Posts deploy events",
            "config": { "channel_id": ch }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "owner creates hook: {body:?}");
    assert!(body["id"].is_string(), "hook id returned: {body:?}");
    assert!(
        body["config"]["token"].is_string(),
        "server mints a token for incoming webhooks: {body:?}"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn incoming_webhook_requires_channel_id(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "hook-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Hooks WS").await;

    let (status, _body) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/hooks"),
        Some(&token),
        Some(json!({
            "hook_type": "incoming_webhook",
            "name": "no-channel",
            "config": { "url": "https://example.test/in" }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn incoming_webhook_posts_message_to_channel(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "hook-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Hooks WS").await;
    let ch = seed_channel(&state, ws, owner_id, "deploys", false).await;

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/hooks"),
        Some(&token),
        Some(json!({
            "hook_type": "incoming_webhook",
            "name": "deploy-bot",
            "config": { "channel_id": ch }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let webhook_token = body["config"]["token"].as_str().expect("token").to_string();

    // No auth header — the URL token is the only credential.
    let (post_status, post_body) = send(
        &app,
        "POST",
        &format!("/api/hooks/incoming/{webhook_token}"),
        None,
        Some(json!({ "text": "deploy succeeded :rocket:" })),
    )
    .await;
    assert_eq!(post_status, StatusCode::OK, "incoming post: {post_body:?}");

    let (list_status, list_body) = send(
        &app,
        "GET",
        &format!("/api/channels/{ch}/messages"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(list_status, StatusCode::OK, "{list_body:?}");
    let found = list_body["data"]
        .as_array()
        .expect("data array")
        .iter()
        .any(|m| m["content"].as_str() == Some("deploy succeeded :rocket:"));
    assert!(
        found,
        "webhook message must land in the channel: {list_body:?}"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn incoming_webhook_invalid_token_rejected(pool: PgPool) {
    let (app, _state) = app_and_state(pool).await;
    let (status, _body) = send(
        &app,
        "POST",
        "/api/hooks/incoming/not-a-real-token",
        None,
        Some(json!({ "text": "hello" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn ws_admin_can_create_hook(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _) = seed(&state, "hook-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Hooks WS").await;

    let (admin_id, admin_email) = seed(&state, "hook-admin", false).await;
    add_ws_member(&state, ws, admin_id, "admin").await;
    let admin_token = login(&app, &admin_email, PASSWORD).await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/hooks"),
        Some(&admin_token),
        Some(json!({ "hook_type": "bot", "name": "helper-bot" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn plain_member_cannot_create_hook(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _) = seed(&state, "hook-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Hooks WS").await;

    let (member_id, member_email) = seed(&state, "hook-member", false).await;
    add_ws_member(&state, ws, member_id, "member").await;
    let member_token = login(&app, &member_email, PASSWORD).await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/hooks"),
        Some(&member_token),
        Some(json!({ "hook_type": "bot", "name": "nope-bot" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "member lacks Admin role");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn create_hook_without_token_is_unauthorized(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _) = seed(&state, "hook-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Hooks WS").await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/hooks"),
        None,
        Some(json!({ "hook_type": "bot", "name": "anon-bot" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn create_hook_missing_required_field_is_unprocessable(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "hook-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Hooks WS").await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/hooks"),
        Some(&token),
        Some(json!({ "name": "no-type" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn list_hooks_redacts_secrets(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "hook-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Hooks WS").await;
    let ch = seed_channel(&state, ws, owner_id, "deploys", false).await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/hooks"),
        Some(&token),
        Some(json!({
            "hook_type": "outgoing_webhook",
            "name": "out-hook",
            "config": {
                "url": "https://example.test/out",
                "channel_ids": [ch],
                "secret": "s3cr3t",
                "token": "abc123"
            }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/hooks"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let arr = body["data"].as_array().expect("data is array");
    assert_eq!(arr.len(), 1, "one hook listed: {body:?}");
    let cfg = &arr[0]["config"];
    assert_eq!(cfg["secret"], "***", "secret redacted");
    assert_eq!(cfg["token"], "***", "token redacted");
    assert_eq!(
        cfg["url"], "https://example.test/out",
        "non-secret left intact"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn member_cannot_list_hooks(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _) = seed(&state, "hook-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Hooks WS").await;

    let (member_id, member_email) = seed(&state, "hook-member", false).await;
    add_ws_member(&state, ws, member_id, "member").await;
    let member_token = login(&app, &member_email, PASSWORD).await;

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/hooks"),
        Some(&member_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn list_hooks_without_token_is_unauthorized(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _) = seed(&state, "hook-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Hooks WS").await;

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/hooks"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn get_hook_by_id_redacts_secrets(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "hook-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Hooks WS").await;
    let ch = seed_channel(&state, ws, owner_id, "alerts", false).await;

    let (status, created) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/hooks"),
        Some(&token),
        Some(json!({
            "hook_type": "incoming_webhook",
            "name": "single-hook",
            "config": { "apiKey": "k-123", "channel_id": ch }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let hook_id = created["id"].as_str().expect("hook id");

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/hooks/{hook_id}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["id"], hook_id);
    assert_eq!(body["config"]["apiKey"], "***", "apiKey redacted");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn get_hook_unknown_id_is_not_found(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_owner_id, _, token) = seed_and_login(&app, &state, "hook-owner", false).await;

    let missing = uuid::Uuid::new_v4();
    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/hooks/{missing}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn get_hook_by_non_admin_member_is_forbidden(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "hook-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Hooks WS").await;

    let (status, created) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/hooks"),
        Some(&owner_token),
        Some(json!({ "hook_type": "bot", "name": "guarded-bot" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let hook_id = created["id"].as_str().expect("hook id");

    let (member_id, member_email) = seed(&state, "hook-member", false).await;
    add_ws_member(&state, ws, member_id, "member").await;
    let member_token = login(&app, &member_email, PASSWORD).await;

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/hooks/{hook_id}"),
        Some(&member_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn get_hook_by_outsider_is_forbidden(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "hook-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Hooks WS").await;

    let (status, created) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/hooks"),
        Some(&owner_token),
        Some(json!({ "hook_type": "bot", "name": "outsider-bot" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let hook_id = created["id"].as_str().expect("hook id");

    let (_outsider_id, _, outsider_token) =
        seed_and_login(&app, &state, "hook-outsider", false).await;
    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/hooks/{hook_id}"),
        Some(&outsider_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn owner_can_delete_hook(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "hook-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Hooks WS").await;

    let (status, created) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/hooks"),
        Some(&token),
        Some(json!({ "hook_type": "bot", "name": "delete-me" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let hook_id = created["id"].as_str().expect("hook id");

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/hooks/{hook_id}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/hooks/{hook_id}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn delete_hook_by_member_is_forbidden(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "hook-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Hooks WS").await;

    let (status, created) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/hooks"),
        Some(&owner_token),
        Some(json!({ "hook_type": "bot", "name": "protected-bot" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let hook_id = created["id"].as_str().expect("hook id");

    let (member_id, member_email) = seed(&state, "hook-member", false).await;
    add_ws_member(&state, ws, member_id, "member").await;
    let member_token = login(&app, &member_email, PASSWORD).await;

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/hooks/{hook_id}"),
        Some(&member_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn delete_hook_unknown_id_is_not_found(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_owner_id, _, token) = seed_and_login(&app, &state, "hook-owner", false).await;

    let missing = uuid::Uuid::new_v4();
    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/hooks/{missing}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn delete_hook_without_token_is_unauthorized(pool: PgPool) {
    let (app, _state) = app_and_state(pool).await;
    let missing = uuid::Uuid::new_v4();
    let (status, _) = send(&app, "DELETE", &format!("/api/hooks/{missing}"), None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn member_can_list_reminders(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _) = seed(&state, "rem-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Reminders WS").await;

    let (member_id, member_email) = seed(&state, "rem-member", false).await;
    add_ws_member(&state, ws, member_id, "member").await;
    let member_token = login(&app, &member_email, PASSWORD).await;

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/reminders"),
        Some(&member_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body["data"].is_array(),
        "reminders list returns data array: {body:?}"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn list_reminders_by_non_member_is_forbidden(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _) = seed(&state, "rem-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Reminders WS").await;

    let (_outsider_id, _, outsider_token) =
        seed_and_login(&app, &state, "rem-outsider", false).await;
    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/reminders"),
        Some(&outsider_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn list_reminders_without_token_is_unauthorized(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _) = seed(&state, "rem-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Reminders WS").await;

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/reminders"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn member_can_create_reminder_for_self(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _) = seed(&state, "rem-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Reminders WS").await;

    let (member_id, member_email) = seed(&state, "rem-member", false).await;
    add_ws_member(&state, ws, member_id, "member").await;
    let member_token = login(&app, &member_email, PASSWORD).await;

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/reminders"),
        Some(&member_token),
        Some(json!({
            "target_user_id": member_id,
            "content": "Stand-up in 10 minutes",
            "remind_at": "2099-01-01T09:00:00Z"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "self reminder: {body:?}");
    assert!(body["id"].is_string(), "reminder id returned: {body:?}");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn member_cannot_create_reminder_for_other_user(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _) = seed(&state, "rem-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Reminders WS").await;

    let (member_id, member_email) = seed(&state, "rem-member", false).await;
    add_ws_member(&state, ws, member_id, "member").await;
    let member_token = login(&app, &member_email, PASSWORD).await;

    let (other_id, _) = seed(&state, "rem-other", false).await;
    add_ws_member(&state, ws, other_id, "member").await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/reminders"),
        Some(&member_token),
        Some(json!({
            "target_user_id": other_id,
            "content": "Not your call",
            "remind_at": "2099-01-01T09:00:00Z"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn admin_can_create_reminder_for_another_member(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "rem-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Reminders WS").await;

    let (target_id, _) = seed(&state, "rem-target", false).await;
    add_ws_member(&state, ws, target_id, "member").await;

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/reminders"),
        Some(&owner_token),
        Some(json!({
            "target_user_id": target_id,
            "content": "Owner-assigned reminder",
            "remind_at": "2099-01-01T09:00:00Z"
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "admin reminder for member: {body:?}"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn admin_cannot_create_reminder_for_non_member_target(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "rem-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Reminders WS").await;

    let (outsider_id, _) = seed(&state, "rem-outsider", false).await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/reminders"),
        Some(&owner_token),
        Some(json!({
            "target_user_id": outsider_id,
            "content": "Target not in workspace",
            "remind_at": "2099-01-01T09:00:00Z"
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "target must be a workspace member"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn create_reminder_by_non_member_is_forbidden(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _) = seed(&state, "rem-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Reminders WS").await;

    let (outsider_id, _, outsider_token) =
        seed_and_login(&app, &state, "rem-outsider", false).await;
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/reminders"),
        Some(&outsider_token),
        Some(json!({
            "target_user_id": outsider_id,
            "content": "I am not in this workspace",
            "remind_at": "2099-01-01T09:00:00Z"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn create_reminder_without_token_is_unauthorized(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _) = seed(&state, "rem-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Reminders WS").await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/reminders"),
        None,
        Some(json!({
            "target_user_id": owner_id,
            "content": "no auth",
            "remind_at": "2099-01-01T09:00:00Z"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn create_reminder_missing_required_field_is_unprocessable(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "rem-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Reminders WS").await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/reminders"),
        Some(&token),
        Some(json!({ "target_user_id": owner_id })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn reveal_returns_the_full_incoming_url_that_list_redacts(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "hook-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Reveal WS").await;
    let ch = seed_channel(&state, ws, owner_id, "alerts", false).await;

    let (_, created) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/hooks"),
        Some(&token),
        Some(json!({
            "hook_type": "incoming_webhook",
            "name": "CI",
            "config": { "channel_id": ch }
        })),
    )
    .await;
    let hook_id = created["id"].as_str().expect("hook id").to_string();
    let minted = created["config"]["token"]
        .as_str()
        .expect("token")
        .to_string();

    let (_, listed) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/hooks"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(
        listed["data"][0]["config"]["token"], "***",
        "listing must keep the token hidden: {listed:?}"
    );

    let (status, revealed) = send(
        &app,
        "POST",
        &format!("/api/hooks/{hook_id}/reveal"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "reveal: {revealed:?}");
    assert_eq!(revealed["config"]["token"], minted);
    assert_eq!(
        revealed["incoming_url"],
        format!("http://localhost/api/hooks/incoming/{minted}"),
        "reveal returns a ready-to-paste URL: {revealed:?}"
    );

    let audited: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM audit_log WHERE action = 'hook.revealed' AND resource_id = $1",
    )
    .bind(uuid::Uuid::parse_str(&hook_id).expect("uuid"))
    .fetch_one(&state.pool)
    .await
    .expect("count audit rows");
    assert_eq!(audited.0, 1, "revealing a secret must be audited");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn rotate_replaces_the_incoming_token_and_retires_the_old_one(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "hook-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Rotate WS").await;
    let ch = seed_channel(&state, ws, owner_id, "alerts", false).await;

    let (_, created) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/hooks"),
        Some(&token),
        Some(json!({
            "hook_type": "incoming_webhook",
            "name": "CI",
            "config": { "channel_id": ch }
        })),
    )
    .await;
    let hook_id = created["id"].as_str().expect("hook id").to_string();
    let old_token = created["config"]["token"]
        .as_str()
        .expect("token")
        .to_string();

    let (status, rotated) = send(
        &app,
        "POST",
        &format!("/api/hooks/{hook_id}/rotate"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "rotate: {rotated:?}");
    let new_token = rotated["config"]["token"].as_str().expect("new token");
    assert_ne!(new_token, old_token, "rotation must mint a fresh token");

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/hooks/incoming/{old_token}"),
        None,
        Some(json!({ "text": "posted with the retired token" })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "the old token must stop working"
    );

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/hooks/incoming/{new_token}"),
        None,
        Some(json!({ "text": "posted with the fresh token" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "the new token must work");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn reveal_and_rotate_are_workspace_admin_only(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "hook-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Hook Guard WS").await;
    let ch = seed_channel(&state, ws, owner_id, "alerts", false).await;

    let (_, created) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/hooks"),
        Some(&token),
        Some(json!({
            "hook_type": "incoming_webhook",
            "name": "CI",
            "config": { "channel_id": ch }
        })),
    )
    .await;
    let hook_id = created["id"].as_str().expect("hook id").to_string();

    let (member_id, _, member_token) = seed_and_login(&app, &state, "hook-member", false).await;
    add_ws_member(&state, ws, member_id, "member").await;

    for route in ["reveal", "rotate"] {
        let (status, _) = send(
            &app,
            "POST",
            &format!("/api/hooks/{hook_id}/{route}"),
            Some(&member_token),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "member cannot {route}");

        let (status, _) = send(
            &app,
            "POST",
            &format!("/api/hooks/{hook_id}/{route}"),
            None,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "anon cannot {route}");
    }

    let unknown = uuid::Uuid::new_v4();
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/hooks/{unknown}/reveal"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn outgoing_webhook_gets_a_generated_secret_and_a_validated_url(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "hook-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Outgoing WS").await;
    let ch = seed_channel(&state, ws, owner_id, "deploys", false).await;

    let (status, created) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/hooks"),
        Some(&token),
        Some(json!({
            "hook_type": "outgoing_webhook",
            "name": "Deploy bot",
            "config": { "url": "https://example.com/hooks/chat", "channel_ids": [ch] }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "create outgoing hook: {created:?}");
    let secret = created["config"]["secret"]
        .as_str()
        .expect("a secret is minted for signing");
    assert!(!secret.is_empty());

    let hook_id = created["id"].as_str().expect("hook id").to_string();
    let (_, rotated) = send(
        &app,
        "POST",
        &format!("/api/hooks/{hook_id}/rotate"),
        Some(&token),
        None,
    )
    .await;
    assert_ne!(
        rotated["config"]["secret"].as_str().expect("new secret"),
        secret,
        "rotation must replace the signing secret"
    );
    assert!(
        rotated["incoming_url"].is_null(),
        "an outgoing hook has no incoming URL: {rotated:?}"
    );

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/hooks"),
        Some(&token),
        Some(json!({
            "hook_type": "outgoing_webhook",
            "name": "Internal",
            "config": { "url": "ftp://example.com/steal", "channel_ids": [ch] }
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "only http(s) targets are accepted: {body:?}"
    );

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/hooks"),
        Some(&token),
        Some(json!({ "hook_type": "outgoing_webhook", "name": "No URL" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "url is required");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn outgoing_webhook_requires_a_channel_scope(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "hook-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Scope WS").await;

    for config in [
        json!({ "url": "https://example.com/out" }),
        json!({ "url": "https://example.com/out", "channel_ids": [] }),
        json!({ "url": "https://example.com/out", "channel_ids": ["not-a-uuid"] }),
    ] {
        let (status, body) = send(
            &app,
            "POST",
            &format!("/api/workspaces/{ws}/hooks"),
            Some(&token),
            Some(json!({
                "hook_type": "outgoing_webhook",
                "name": "Unscoped",
                "config": config
            })),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "an outgoing webhook must name the channels it may read: {body:?}"
        );
    }
}

#[test_macros::db_test(migrations = "../migrations")]
async fn outgoing_webhook_cannot_be_scoped_to_a_foreign_channel(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "hook-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Scope WS").await;
    let other_ws = seed_workspace(&state, owner_id, "Other WS").await;
    let foreign = seed_channel(&state, other_ws, owner_id, "elsewhere", false).await;

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/hooks"),
        Some(&token),
        Some(json!({
            "hook_type": "outgoing_webhook",
            "name": "Cross tenant",
            "config": { "url": "https://example.com/out", "channel_ids": [foreign] }
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "a hook must not reach into another workspace: {body:?}"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn attaching_a_hook_to_a_private_channel_needs_membership(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _) = seed(&state, "hook-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Private WS").await;
    let secret_channel = seed_channel(&state, ws, owner_id, "leadership", true).await;

    let (admin_id, admin_email) = seed(&state, "hook-admin", false).await;
    add_ws_member(&state, ws, admin_id, "admin").await;
    let admin_token = login(&app, &admin_email, PASSWORD).await;

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/hooks"),
        Some(&admin_token),
        Some(json!({
            "hook_type": "outgoing_webhook",
            "name": "Eavesdropper",
            "config": { "url": "https://example.com/out", "channel_ids": [secret_channel] }
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "an admin outside a private channel must not forward it off the instance: {body:?}"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn hooked_channels_are_listed_for_every_member(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "hook-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Indicator WS").await;
    let hooked = seed_channel(&state, ws, owner_id, "deploys", false).await;
    let quiet = seed_channel(&state, ws, owner_id, "random", false).await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/hooks"),
        Some(&token),
        Some(json!({
            "hook_type": "outgoing_webhook",
            "name": "Deploy bot",
            "config": { "url": "https://example.com/out", "channel_ids": [hooked] }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (member_id, member_email) = seed(&state, "hook-member", false).await;
    add_ws_member(&state, ws, member_id, "member").await;
    let member_token = login(&app, &member_email, PASSWORD).await;

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/hooks/channels"),
        Some(&member_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let ids: Vec<&str> = body["channel_ids"]
        .as_array()
        .expect("channel_ids array")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(
        ids.contains(&hooked.to_string().as_str()),
        "a member has to be able to see that the channel is forwarded: {body:?}"
    );
    assert!(
        !ids.contains(&quiet.to_string().as_str()),
        "an unhooked channel must not be flagged: {body:?}"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn outgoing_hooks_only_match_their_own_channel(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "hook-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Match WS").await;
    let hooked = seed_channel(&state, ws, owner_id, "deploys", false).await;
    let quiet = seed_channel(&state, ws, owner_id, "random", false).await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/hooks"),
        Some(&token),
        Some(json!({
            "hook_type": "outgoing_webhook",
            "name": "Deploy bot",
            "config": { "url": "https://example.com/out", "channel_ids": [hooked] }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let matched = state
        .hook_repo
        .list_active_outgoing_hooks_for_channel(ws, hooked)
        .await
        .expect("lookup");
    assert_eq!(matched.len(), 1, "the scoped channel fires the hook");

    let unmatched = state
        .hook_repo
        .list_active_outgoing_hooks_for_channel(ws, quiet)
        .await
        .expect("lookup");
    assert!(
        unmatched.is_empty(),
        "a channel outside the scope must never reach the webhook"
    );
}

/// An integration posting into an announcement channel is the normal case — a
/// release feed is exactly what such a channel is for. The hook's own scoping
/// (CS-019) is what decides it, not the human posting policy.
#[sqlx::test(migrations = "../migrations")]
async fn an_incoming_webhook_still_posts_into_an_announcement_channel(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "hook-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Releases").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "releases", false).await;

    let (status, hook) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/hooks"),
        Some(&owner_token),
        Some(json!({
            "hook_type": "incoming_webhook",
            "name": "CI",
            "config": { "channel_id": ch_id }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{hook:?}");
    let token = hook["config"]["token"].as_str().expect("token").to_string();

    let (status, _) = send(
        &app,
        "PATCH",
        &format!("/api/channels/{ch_id}"),
        Some(&owner_token),
        Some(json!({ "post_policy": "moderators" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/hooks/incoming/{token}"),
        None,
        Some(json!({ "text": "build 42 shipped" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "the hook is not a person: {body:?}");

    let posted: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM messages WHERE channel_id = $1 AND metadata ? 'bot'",
    )
    .bind(ch_id)
    .fetch_one(&state.pool)
    .await
    .expect("count");
    assert_eq!(posted, 1);
}
