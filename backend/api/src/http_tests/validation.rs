use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

use super::common::*;

#[test_macros::db_test(migrations = "../migrations")]
async fn an_over_long_channel_reaction_is_a_400_not_a_500(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "val-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Validation WS").await;
    let ch = seed_channel(&state, ws, owner_id, "main", false).await;

    let (_, msg) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch}/messages"),
        Some(&token),
        Some(json!({ "content": "react to me" })),
    )
    .await;
    let msg_id = msg["id"].as_str().expect("id").to_string();

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/messages/{msg_id}/reactions"),
        Some(&token),
        Some(json!({ "emoji": "x".repeat(60) })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "a value the column cannot hold is the caller's mistake, not a server error: {body:?}"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_reaction_the_websocket_would_accept_is_accepted_here_too(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "val-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Validation WS").await;
    let ch = seed_channel(&state, ws, owner_id, "main", false).await;

    let (_, msg) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch}/messages"),
        Some(&token),
        Some(json!({ "content": "react to me" })),
    )
    .await;
    let msg_id = msg["id"].as_str().expect("id").to_string();

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/messages/{msg_id}/reactions"),
        Some(&token),
        Some(json!({ "emoji": "🚀" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "the two paths must agree");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_duplicate_reaction_is_a_conflict_not_a_database_error(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "val-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Validation WS").await;
    let ch = seed_channel(&state, ws, owner_id, "main", false).await;

    let (_, msg) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch}/messages"),
        Some(&token),
        Some(json!({ "content": "react to me" })),
    )
    .await;
    let msg_id = msg["id"].as_str().expect("id").to_string();
    let body = json!({ "emoji": "👍" });

    let (first, _) = send(
        &app,
        "POST",
        &format!("/api/messages/{msg_id}/reactions"),
        Some(&token),
        Some(body.clone()),
    )
    .await;
    assert_eq!(first, StatusCode::OK);

    let (second, response) = send(
        &app,
        "POST",
        &format!("/api/messages/{msg_id}/reactions"),
        Some(&token),
        Some(body),
    )
    .await;
    assert_ne!(
        second,
        StatusCode::INTERNAL_SERVER_ERROR,
        "reacting twice is a user action, not a server fault: {response:?}"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn over_long_reminder_content_is_refused(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "val-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Validation WS").await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/reminders"),
        Some(&token),
        Some(json!({
            "target_user_id": owner_id,
            "content": "x".repeat(4001),
            "remind_at": "2099-01-01T09:00:00Z"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn over_long_channel_topic_and_description_are_refused(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "val-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Validation WS").await;
    let ch = seed_channel(&state, ws, owner_id, "main", false).await;

    let (status, _) = send(
        &app,
        "PATCH",
        &format!("/api/channels/{ch}"),
        Some(&token),
        Some(json!({ "topic": "x".repeat(501) })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "topic");

    let (status, _) = send(
        &app,
        "PATCH",
        &format!("/api/channels/{ch}"),
        Some(&token),
        Some(json!({ "description": "x".repeat(4001) })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "description");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_profile_bio_and_timezone_are_bounded(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_id, _, token) = seed_and_login(&app, &state, "val-owner", false).await;

    let (status, _) = send(
        &app,
        "PATCH",
        "/api/users/me",
        Some(&token),
        Some(json!({ "bio": "x".repeat(501) })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "bio");

    let (status, _) = send(
        &app,
        "PATCH",
        "/api/users/me",
        Some(&token),
        Some(json!({ "timezone": "Europe/Belgrade'; DROP TABLE users" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "timezone");

    let (status, _) = send(
        &app,
        "PATCH",
        "/api/users/me",
        Some(&token),
        Some(json!({ "timezone": "Europe/Belgrade" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "a real IANA name is accepted");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn workspace_and_hook_free_text_is_bounded(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "val-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Validation WS").await;

    let (status, _) = send(
        &app,
        "PATCH",
        &format!("/api/workspaces/{ws}"),
        Some(&token),
        Some(json!({ "description": "x".repeat(4001) })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "ws description");

    let (status, _) = send(
        &app,
        "PATCH",
        &format!("/api/workspaces/{ws}"),
        Some(&token),
        Some(json!({ "icon_url": "javascript:alert(1)" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "ws icon_url");

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/hooks"),
        Some(&token),
        Some(json!({ "hook_type": "bot", "name": "   " })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "hook name");

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/invites"),
        Some(&token),
        Some(json!({ "email": "not-an-address", "role": "member", "max_uses": 1 })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "invite email");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_channel_client_id_never_reaches_into_another_channel(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "idem-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Channel Idempotency WS").await;
    let secret = seed_channel(&state, ws, owner_id, "leadership", true).await;
    let open = seed_channel(&state, ws, owner_id, "main", false).await;

    let shared_id = uuid::Uuid::new_v4();
    let (status, hidden) = send(
        &app,
        "POST",
        &format!("/api/channels/{secret}/messages"),
        Some(&owner),
        Some(json!({ "content": "the private plan", "client_message_id": shared_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{hidden:?}");
    assert_ne!(
        hidden["id"].as_str(),
        Some(shared_id.to_string().as_str()),
        "the server owns the message id, not the client"
    );

    let (member_id, member_email) = seed(&state, "idem-member", false).await;
    add_ws_member(&state, ws, member_id, "member").await;
    let member = login(&app, &member_email, PASSWORD).await;

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/channels/{open}/messages"),
        Some(&member),
        Some(json!({ "content": "unrelated", "client_message_id": shared_id })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the same client id in another channel is not a conflict at all: {body:?}"
    );
    assert_eq!(
        body["content"], "unrelated",
        "and it must not hand back a message from a channel the caller cannot read"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn resending_a_channel_message_with_the_same_client_id_is_idempotent(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "idem-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Channel Idempotency WS").await;
    let ch = seed_channel(&state, ws, owner_id, "main", false).await;

    let body = json!({ "content": "sent once", "client_message_id": uuid::Uuid::new_v4() });

    let (_, first) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch}/messages"),
        Some(&owner),
        Some(body.clone()),
    )
    .await;
    let (status, retry) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch}/messages"),
        Some(&owner),
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "a retry is not an error");
    assert_eq!(retry["id"], first["id"], "and returns the original message");

    let stored: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE channel_id = $1")
        .bind(ch)
        .fetch_one(&state.pool)
        .await
        .expect("count");
    assert_eq!(stored, 1, "one row, not two");
}
