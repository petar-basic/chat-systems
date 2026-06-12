use super::common::*;
use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

use crate::notifications::models::NotificationType;

async fn seed_notification(
    state: &crate::state::AppState,
    user_id: uuid::Uuid,
    ws_id: uuid::Uuid,
) -> uuid::Uuid {
    state
        .notification_repo
        .create(
            user_id,
            ws_id,
            &NotificationType::Mention,
            "You were mentioned",
            Some("hello"),
            &json!({ "message_id": "m1" }),
        )
        .await
        .expect("seed notification")
        .id
}

#[sqlx::test(migrations = "../migrations")]
async fn list_notifications_returns_seeded_rows(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (uid, _email, token) = seed_and_login(&app, &state, "notif-list", false).await;
    let ws = seed_workspace(&state, uid, "Notif WS").await;
    let n1 = seed_notification(&state, uid, ws).await;
    let n2 = seed_notification(&state, uid, ws).await;

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/notifications"),
        Some(&token),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::OK, "list should be 200: {body:?}");
    let data = body["data"].as_array().expect("data array");
    assert_eq!(
        data.len(),
        2,
        "both seeded notifications must be listed: {body:?}"
    );
    let ids: Vec<&str> = data.iter().filter_map(|n| n["id"].as_str()).collect();
    assert!(ids.contains(&n1.to_string().as_str()));
    assert!(ids.contains(&n2.to_string().as_str()));
}

#[sqlx::test(migrations = "../migrations")]
async fn list_notifications_without_token_is_401(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (uid, _email, _token) = seed_and_login(&app, &state, "notif-list-noauth", false).await;
    let ws = seed_workspace(&state, uid, "Notif WS").await;

    let (status, _body) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/notifications"),
        None,
        None,
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED, "no token must be 401");
}

#[sqlx::test(migrations = "../migrations")]
async fn list_notifications_non_member_forbidden(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner, _e1, _t1) = seed_and_login(&app, &state, "notif-owner", false).await;
    let ws = seed_workspace(&state, owner, "Notif WS").await;
    let _n = seed_notification(&state, owner, ws).await;

    let (_other, _e2, other_token) = seed_and_login(&app, &state, "notif-other", false).await;

    let (status, _body) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/notifications"),
        Some(&other_token),
        None,
    )
    .await;

    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a non-member of the workspace must be rejected"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn unread_count_reflects_seeded_unread(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (uid, _email, token) = seed_and_login(&app, &state, "notif-count", false).await;
    let ws = seed_workspace(&state, uid, "Notif WS").await;
    seed_notification(&state, uid, ws).await;
    seed_notification(&state, uid, ws).await;
    seed_notification(&state, uid, ws).await;

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/notifications/unread-count"),
        Some(&token),
        None,
    )
    .await;

    assert_eq!(
        status,
        StatusCode::OK,
        "unread-count should be 200: {body:?}"
    );
    assert_eq!(
        body["unread_count"].as_i64(),
        Some(3),
        "three unread notifications expected: {body:?}"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn unread_count_without_token_is_401(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (uid, _email, _token) = seed_and_login(&app, &state, "notif-count-noauth", false).await;
    let ws = seed_workspace(&state, uid, "Notif WS").await;

    let (status, _body) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/notifications/unread-count"),
        None,
        None,
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED, "no token must be 401");
}

#[sqlx::test(migrations = "../migrations")]
async fn mark_read_clears_specified_notifications(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (uid, _email, token) = seed_and_login(&app, &state, "notif-mark", false).await;
    let ws = seed_workspace(&state, uid, "Notif WS").await;
    let n1 = seed_notification(&state, uid, ws).await;
    let n2 = seed_notification(&state, uid, ws).await;

    let (_s, before) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/notifications/unread-count"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(before["unread_count"].as_i64(), Some(2));

    let (status, body) = send(
        &app,
        "POST",
        "/api/notifications/read",
        Some(&token),
        Some(json!({ "notification_ids": [n1.to_string()] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "mark-read should be 200: {body:?}");

    let (_s, after) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/notifications/unread-count"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(
        after["unread_count"].as_i64(),
        Some(1),
        "only n1 was marked read; n2 ({n2}) must remain unread: {after:?}"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn mark_read_with_empty_ids_is_ok(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (uid, _email, token) = seed_and_login(&app, &state, "notif-mark-empty", false).await;
    let ws = seed_workspace(&state, uid, "Notif WS").await;
    seed_notification(&state, uid, ws).await;

    let (status, body) = send(
        &app,
        "POST",
        "/api/notifications/read",
        Some(&token),
        Some(json!({ "notification_ids": [] })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "empty mark-read should be 200: {body:?}"
    );

    let (_s, after) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/notifications/unread-count"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(
        after["unread_count"].as_i64(),
        Some(1),
        "empty list must not change anything"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn mark_read_without_token_is_401(pool: PgPool) {
    let (app, _state) = app_and_state(pool).await;
    let (status, _body) = send(
        &app,
        "POST",
        "/api/notifications/read",
        None,
        Some(json!({ "notification_ids": [] })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "no token must be 401");
}

#[sqlx::test(migrations = "../migrations")]
async fn mark_read_missing_field_is_422(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_uid, _email, token) = seed_and_login(&app, &state, "notif-mark-bad", false).await;

    let (status, _body) = send(
        &app,
        "POST",
        "/api/notifications/read",
        Some(&token),
        Some(json!({})),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "missing notification_ids must be 422"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn mark_read_cannot_affect_other_users_notification(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner, _e1, owner_token) = seed_and_login(&app, &state, "notif-victim", false).await;
    let ws = seed_workspace(&state, owner, "Notif WS").await;
    let owners_notif = seed_notification(&state, owner, ws).await;

    let (_attacker, _e2, attacker_token) =
        seed_and_login(&app, &state, "notif-attacker", false).await;

    let (status, _body) = send(
        &app,
        "POST",
        "/api/notifications/read",
        Some(&attacker_token),
        Some(json!({ "notification_ids": [owners_notif.to_string()] })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "request itself is well-formed -> 200"
    );

    let (_s, after) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/notifications/unread-count"),
        Some(&owner_token),
        None,
    )
    .await;
    assert_eq!(
        after["unread_count"].as_i64(),
        Some(1),
        "another user must not be able to mark the owner's notification read"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn mark_all_read_clears_unread_count(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (uid, _email, token) = seed_and_login(&app, &state, "notif-markall", false).await;
    let ws = seed_workspace(&state, uid, "Notif WS").await;
    seed_notification(&state, uid, ws).await;
    seed_notification(&state, uid, ws).await;

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/notifications/read-all"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "read-all should be 200: {body:?}");

    let (_s, after) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/notifications/unread-count"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(
        after["unread_count"].as_i64(),
        Some(0),
        "after read-all there must be zero unread: {after:?}"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn mark_all_read_is_scoped_to_workspace(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (uid, _email, token) = seed_and_login(&app, &state, "notif-markall-scope", false).await;
    let ws_a = seed_workspace(&state, uid, "WS A").await;
    let ws_b = seed_workspace(&state, uid, "WS B").await;
    seed_notification(&state, uid, ws_a).await;
    seed_notification(&state, uid, ws_b).await;

    let (status, _body) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_a}/notifications/read-all"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (_sa, count_a) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_a}/notifications/unread-count"),
        Some(&token),
        None,
    )
    .await;
    let (_sb, count_b) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_b}/notifications/unread-count"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(count_a["unread_count"].as_i64(), Some(0), "WS A cleared");
    assert_eq!(count_b["unread_count"].as_i64(), Some(1), "WS B untouched");
}

#[sqlx::test(migrations = "../migrations")]
async fn mark_all_read_without_token_is_401(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (uid, _email, _token) = seed_and_login(&app, &state, "notif-markall-noauth", false).await;
    let ws = seed_workspace(&state, uid, "Notif WS").await;

    let (status, _body) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/notifications/read-all"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "no token must be 401");
}
