use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;

async fn post_message(app: &axum::Router, token: &str, ch: Uuid, content: &str) -> String {
    let (status, body) = send(
        app,
        "POST",
        &format!("/api/channels/{ch}/messages"),
        Some(token),
        Some(json!({ "content": content })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    body["id"].as_str().expect("message id").to_string()
}

async fn edit(app: &axum::Router, token: &str, msg_id: &str, content: &str) {
    let (status, body) = send(
        app,
        "PATCH",
        &format!("/api/messages/{msg_id}"),
        Some(token),
        Some(json!({ "content": content })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn every_edit_leaves_the_text_it_replaced(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "edit-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Edits WS").await;
    let ch = seed_channel(&state, ws, owner_id, "main", false).await;

    let msg_id = post_message(&app, &token, ch, "first").await;
    edit(&app, &token, &msg_id, "second").await;
    edit(&app, &token, &msg_id, "third").await;
    edit(&app, &token, &msg_id, "fourth").await;

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/messages/{msg_id}/history"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");

    let versions: Vec<String> = body["data"]
        .as_array()
        .expect("data array")
        .iter()
        .map(|e| {
            e["previous_content"]
                .as_str()
                .unwrap_or_default()
                .to_string()
        })
        .collect();
    assert_eq!(
        versions,
        vec!["third", "second", "first"],
        "newest replaced text first"
    );
    assert!(
        !versions.contains(&"fourth".to_string()),
        "the current text is not a previous version"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn history_is_for_the_author_and_workspace_admins_only(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "edit-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Edits WS").await;
    let ch = seed_channel(&state, ws, owner_id, "main", false).await;

    let (author_id, author_email) = seed(&state, "edit-author", false).await;
    add_ws_member(&state, ws, author_id, "member").await;
    let author = login(&app, &author_email, PASSWORD).await;

    let (bystander_id, bystander_email) = seed(&state, "edit-bystander", false).await;
    add_ws_member(&state, ws, bystander_id, "member").await;
    let bystander = login(&app, &bystander_email, PASSWORD).await;

    let msg_id = post_message(&app, &author, ch, "original").await;
    edit(&app, &author, &msg_id, "revised").await;

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/messages/{msg_id}/history"),
        Some(&author),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "the author reads their own");

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/messages/{msg_id}/history"),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "a workspace admin reads anyone's");

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/messages/{msg_id}/history"),
        Some(&bystander),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a reader of the channel is not entitled to earlier drafts"
    );

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/messages/{msg_id}/history"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_admin_reading_somebody_elses_history_is_audited(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "edit-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Edits WS").await;
    let ch = seed_channel(&state, ws, owner_id, "main", false).await;

    let (author_id, author_email) = seed(&state, "edit-author", false).await;
    add_ws_member(&state, ws, author_id, "member").await;
    let author = login(&app, &author_email, PASSWORD).await;

    let msg_id = post_message(&app, &author, ch, "original").await;
    edit(&app, &author, &msg_id, "revised").await;

    send(
        &app,
        "GET",
        &format!("/api/messages/{msg_id}/history"),
        Some(&author),
        None,
    )
    .await;
    let after_self: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM audit_log WHERE action = 'message.history_read'")
            .fetch_one(&state.pool)
            .await
            .expect("count");
    assert_eq!(after_self, 0, "reading your own history is not an event");

    send(
        &app,
        "GET",
        &format!("/api/messages/{msg_id}/history"),
        Some(&owner),
        None,
    )
    .await;
    let after_admin: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM audit_log WHERE action = 'message.history_read'")
            .fetch_one(&state.pool)
            .await
            .expect("count");
    assert_eq!(after_admin, 1, "an admin reading somebody else's is");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn stored_versions_are_capped(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "edit-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Edits WS").await;
    let ch = seed_channel(&state, ws, owner_id, "main", false).await;

    let msg_id = post_message(&app, &token, ch, "v0").await;
    let cap = crate::messaging::repo::MAX_STORED_EDITS;
    for i in 1..=(cap + 5) {
        edit(&app, &token, &msg_id, &format!("v{i}")).await;
    }

    let stored: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM message_edits WHERE message_id = $1")
            .bind(Uuid::parse_str(&msg_id).expect("uuid"))
            .fetch_one(&state.pool)
            .await
            .expect("count");
    assert_eq!(stored, cap, "an edit loop cannot grow without bound");

    let oldest_kept: Option<String> = sqlx::query_scalar(
        "SELECT previous_content FROM message_edits WHERE message_id = $1 ORDER BY edited_at LIMIT 1",
    )
    .bind(Uuid::parse_str(&msg_id).expect("uuid"))
    .fetch_one(&state.pool)
    .await
    .expect("oldest");
    assert_ne!(
        oldest_kept.as_deref(),
        Some("v0"),
        "the oldest versions are the ones dropped"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn conversation_messages_keep_history_too(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (alice_id, _, alice) = seed_and_login(&app, &state, "edit-alice", false).await;
    let ws = seed_workspace(&state, alice_id, "Edits WS").await;
    let (bob_id, bob_email) = seed(&state, "edit-bob", false).await;
    add_ws_member(&state, ws, bob_id, "member").await;
    let bob = login(&app, &bob_email, PASSWORD).await;

    let (_, conv) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/conversations"),
        Some(&alice),
        Some(json!({ "participant_ids": [bob_id] })),
    )
    .await;
    let conv_id = conv["id"].as_str().expect("id").to_string();

    let (_, msg) = send(
        &app,
        "POST",
        &format!("/api/channels/{conv_id}/messages"),
        Some(&alice),
        Some(json!({ "content": "original" })),
    )
    .await;
    let msg_id = msg["id"].as_str().expect("id").to_string();

    let (status, _) = send(
        &app,
        "PATCH",
        &format!("/api/messages/{msg_id}"),
        Some(&alice),
        Some(json!({ "content": "revised" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/messages/{msg_id}/history"),
        Some(&alice),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["data"][0]["previous_content"], "original");

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/messages/{msg_id}/history"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "the other participant is not the author and not an admin here"
    );
}
