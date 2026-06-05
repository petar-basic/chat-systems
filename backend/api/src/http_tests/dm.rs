use super::common::*;
use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

async fn seed_dm_pair(
    app: &axum::Router,
    state: &crate::state::AppState,
) -> (
    uuid::Uuid,
    (uuid::Uuid, String, String),
    (uuid::Uuid, String, String),
) {
    let (sender_id, sender_email, sender_token) =
        seed_and_login(app, state, "dm-sender", false).await;
    let (partner_id, partner_email, partner_token) =
        seed_and_login(app, state, "dm-partner", false).await;

    let ws_id = seed_workspace(state, sender_id, "dm-ws").await;
    add_ws_member(state, ws_id, partner_id, "member").await;

    (
        ws_id,
        (sender_id, sender_email, sender_token),
        (partner_id, partner_email, partner_token),
    )
}

#[sqlx::test(migrations = "../migrations")]
async fn send_dm_happy_path(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (ws_id, (_sender_id, _, sender_token), (partner_id, _, _)) =
        seed_dm_pair(&app, &state).await;

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/dm/{partner_id}"),
        Some(&sender_token),
        Some(json!({ "content": "hello there" })),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "send DM should succeed: {body:?}");
    assert!(
        body["id"].is_string(),
        "response should carry a message id: {body:?}"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn send_dm_idempotent_with_explicit_id(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (ws_id, (_sender_id, _, sender_token), (partner_id, _, _)) =
        seed_dm_pair(&app, &state).await;

    let msg_id = uuid::Uuid::new_v4().to_string();
    let uri = format!("/api/workspaces/{ws_id}/dm/{partner_id}");
    let body = json!({ "content": "idempotent", "id": msg_id });

    let (status1, b1) = send(&app, "POST", &uri, Some(&sender_token), Some(body.clone())).await;
    assert_eq!(status1, StatusCode::OK, "first send: {b1:?}");

    let (status2, b2) = send(&app, "POST", &uri, Some(&sender_token), Some(body)).await;
    assert_eq!(status2, StatusCode::OK, "idempotent retry: {b2:?}");
    assert_eq!(b1["id"], b2["id"], "retry must return the same message id");
}

#[sqlx::test(migrations = "../migrations")]
async fn send_dm_empty_content_rejected(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (ws_id, (_sender_id, _, sender_token), (partner_id, _, _)) =
        seed_dm_pair(&app, &state).await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/dm/{partner_id}"),
        Some(&sender_token),
        Some(json!({ "content": "   " })),
    )
    .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrations = "../migrations")]
async fn send_dm_requires_auth(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (ws_id, (_sender_id, _, _), (partner_id, _, _)) = seed_dm_pair(&app, &state).await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/dm/{partner_id}"),
        None,
        Some(json!({ "content": "no token" })),
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn send_dm_non_member_sender_forbidden(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (ws_id, (_sender_id, _, _), (partner_id, _, _)) = seed_dm_pair(&app, &state).await;

    let (_outsider_id, _, outsider_token) =
        seed_and_login(&app, &state, "dm-outsider", false).await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/dm/{partner_id}"),
        Some(&outsider_token),
        Some(json!({ "content": "let me in" })),
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../migrations")]
async fn send_dm_to_non_member_partner_forbidden(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (ws_id, (_sender_id, _, sender_token), (_partner_id, _, _)) =
        seed_dm_pair(&app, &state).await;

    let (outsider_id, _, _) = seed_and_login(&app, &state, "dm-target-out", false).await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/dm/{outsider_id}"),
        Some(&sender_token),
        Some(json!({ "content": "are you there" })),
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../migrations")]
async fn list_messages_happy_path(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (ws_id, (_sender_id, _, sender_token), (partner_id, _, _)) =
        seed_dm_pair(&app, &state).await;

    let uri = format!("/api/workspaces/{ws_id}/dm/{partner_id}");

    let (send_status, _) = send(
        &app,
        "POST",
        &uri,
        Some(&sender_token),
        Some(json!({ "content": "first message" })),
    )
    .await;
    assert_eq!(send_status, StatusCode::OK);

    let (status, body) = send(&app, "GET", &uri, Some(&sender_token), None).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "list messages should succeed: {body:?}"
    );
    assert!(
        body["data"].is_array(),
        "messages should be under data array: {body:?}"
    );
    assert_eq!(
        body["data"].as_array().map(|a| a.len()),
        Some(1),
        "the one seeded message should be returned: {body:?}"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn list_messages_requires_auth(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (ws_id, (_sender_id, _, _), (partner_id, _, _)) = seed_dm_pair(&app, &state).await;

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/dm/{partner_id}"),
        None,
        None,
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn list_messages_non_member_forbidden(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (ws_id, (_sender_id, _, _), (partner_id, _, _)) = seed_dm_pair(&app, &state).await;

    let (_outsider_id, _, outsider_token) =
        seed_and_login(&app, &state, "dm-list-out", false).await;

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/dm/{partner_id}"),
        Some(&outsider_token),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../migrations")]
async fn list_conversations_happy_path(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (ws_id, (_sender_id, _, sender_token), (partner_id, _, _)) =
        seed_dm_pair(&app, &state).await;

    let (send_status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/dm/{partner_id}"),
        Some(&sender_token),
        Some(json!({ "content": "start a convo" })),
    )
    .await;
    assert_eq!(send_status, StatusCode::OK);

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/dm"),
        Some(&sender_token),
        None,
    )
    .await;

    assert_eq!(
        status,
        StatusCode::OK,
        "list conversations should succeed: {body:?}"
    );
    assert!(
        body["data"].is_array(),
        "conversations should be under data array: {body:?}"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn list_conversations_requires_auth(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (ws_id, (_sender_id, _, _), (_partner_id, _, _)) = seed_dm_pair(&app, &state).await;

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/dm"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn list_conversations_non_member_forbidden(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (ws_id, (_sender_id, _, _), (_partner_id, _, _)) = seed_dm_pair(&app, &state).await;

    let (_outsider_id, _, outsider_token) =
        seed_and_login(&app, &state, "dm-conv-out", false).await;

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/dm"),
        Some(&outsider_token),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}
