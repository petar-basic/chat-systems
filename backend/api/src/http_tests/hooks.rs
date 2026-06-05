use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

use super::common::*;

#[sqlx::test(migrations = "../migrations")]
async fn owner_can_create_hook(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "hook-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Hooks WS").await;

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/hooks"),
        Some(&token),
        Some(json!({
            "hook_type": "incoming_webhook",
            "name": "deploy-bot",
            "description": "Posts deploy events",
            "config": { "url": "https://example.test/in" }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "owner creates hook: {body:?}");
    assert!(body["id"].is_string(), "hook id returned: {body:?}");
}

#[sqlx::test(migrations = "../migrations")]
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

#[sqlx::test(migrations = "../migrations")]
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

#[sqlx::test(migrations = "../migrations")]
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

#[sqlx::test(migrations = "../migrations")]
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

#[sqlx::test(migrations = "../migrations")]
async fn list_hooks_redacts_secrets(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "hook-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Hooks WS").await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/hooks"),
        Some(&token),
        Some(json!({
            "hook_type": "outgoing_webhook",
            "name": "out-hook",
            "config": { "url": "https://example.test/out", "secret": "s3cr3t", "token": "abc123" }
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

#[sqlx::test(migrations = "../migrations")]
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

#[sqlx::test(migrations = "../migrations")]
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

#[sqlx::test(migrations = "../migrations")]
async fn get_hook_by_id_redacts_secrets(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "hook-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Hooks WS").await;

    let (status, created) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/hooks"),
        Some(&token),
        Some(json!({
            "hook_type": "incoming_webhook",
            "name": "single-hook",
            "config": { "apiKey": "k-123", "url": "https://example.test" }
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

#[sqlx::test(migrations = "../migrations")]
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

#[sqlx::test(migrations = "../migrations")]
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

#[sqlx::test(migrations = "../migrations")]
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

#[sqlx::test(migrations = "../migrations")]
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

#[sqlx::test(migrations = "../migrations")]
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

#[sqlx::test(migrations = "../migrations")]
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

#[sqlx::test(migrations = "../migrations")]
async fn delete_hook_without_token_is_unauthorized(pool: PgPool) {
    let (app, _state) = app_and_state(pool).await;
    let missing = uuid::Uuid::new_v4();
    let (status, _) = send(&app, "DELETE", &format!("/api/hooks/{missing}"), None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
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

#[sqlx::test(migrations = "../migrations")]
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

#[sqlx::test(migrations = "../migrations")]
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

#[sqlx::test(migrations = "../migrations")]
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

#[sqlx::test(migrations = "../migrations")]
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

#[sqlx::test(migrations = "../migrations")]
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

#[sqlx::test(migrations = "../migrations")]
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

#[sqlx::test(migrations = "../migrations")]
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

#[sqlx::test(migrations = "../migrations")]
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

#[sqlx::test(migrations = "../migrations")]
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
