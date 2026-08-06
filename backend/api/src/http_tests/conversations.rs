use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;

async fn seed_trio(
    pool: PgPool,
) -> (
    axum::Router,
    std::sync::Arc<crate::state::AppState>,
    Uuid,
    Uuid,
    String,
    Uuid,
    String,
    Uuid,
    String,
) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "conv-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Conversations WS").await;

    let (second_id, _, second_token) = seed_and_login(&app, &state, "conv-second", false).await;
    add_ws_member(&state, ws_id, second_id, "member").await;
    let (third_id, _, third_token) = seed_and_login(&app, &state, "conv-third", false).await;
    add_ws_member(&state, ws_id, third_id, "member").await;

    (
        app,
        state,
        ws_id,
        owner_id,
        owner_token,
        second_id,
        second_token,
        third_id,
        third_token,
    )
}

#[sqlx::test(migrations = "../migrations")]
async fn a_second_direct_conversation_reuses_the_first(pool: PgPool) {
    let (app, _state, ws_id, _owner_id, owner_token, second_id, second_token, _t, _tt) =
        seed_trio(pool).await;

    let (status, first) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&owner_token),
        Some(json!({ "participant_ids": [second_id] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "create direct: {first:?}");
    assert_eq!(first["kind"], "direct");

    let (status, again) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&owner_token),
        Some(json!({ "participant_ids": [second_id] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(again["id"], first["id"], "the same pair maps to one thread");

    let (status, from_other_side) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&second_token),
        Some(json!({ "participant_ids": [_owner_id] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        from_other_side["id"], first["id"],
        "the other participant lands in the same thread"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn a_group_conversation_carries_every_participant(pool: PgPool) {
    let (app, _state, ws_id, owner_id, owner_token, second_id, second_token, third_id, _tt) =
        seed_trio(pool).await;

    let (status, group) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&owner_token),
        Some(json!({ "participant_ids": [second_id, third_id] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "create group: {group:?}");
    assert_eq!(group["kind"], "group");
    let conv_id = group["id"].as_str().expect("id").to_string();

    let (status, listing) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&second_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let row = listing["data"]
        .as_array()
        .expect("array")
        .iter()
        .find(|c| c["id"].as_str() == Some(conv_id.as_str()))
        .expect("the group shows up for another participant");
    let participants: Vec<&str> = row["participant_ids"]
        .as_array()
        .expect("participants")
        .iter()
        .map(|v| v.as_str().unwrap_or_default())
        .collect();
    assert_eq!(participants.len(), 3);
    for id in [owner_id, second_id, third_id] {
        assert!(participants.contains(&id.to_string().as_str()));
    }
}

#[sqlx::test(migrations = "../migrations")]
async fn conversations_reject_outsiders_and_oversized_groups(pool: PgPool) {
    let (app, state, ws_id, _owner_id, owner_token, second_id, _st, _third, _tt) =
        seed_trio(pool).await;

    let (outsider_id, _) = seed(&state, "conv-outsider", false).await;
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&owner_token),
        Some(json!({ "participant_ids": [outsider_id] })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "cannot open a thread with someone outside the workspace"
    );

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&owner_token),
        Some(json!({ "participant_ids": [] })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "needs a partner");

    let mut crowd = Vec::new();
    for i in 0..9 {
        let (member_id, _) = seed(&state, &format!("conv-crowd-{i}"), false).await;
        add_ws_member(&state, ws_id, member_id, "member").await;
        crowd.push(member_id);
    }
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&owner_token),
        Some(json!({ "participant_ids": crowd })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "a conversation holds at most nine people"
    );

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/conversations"),
        None,
        Some(json!({ "participant_ids": [second_id] })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn messages_are_visible_to_participants_only(pool: PgPool) {
    let (app, state, ws_id, _owner_id, owner_token, second_id, second_token, _third, third_token) =
        seed_trio(pool).await;

    let (_, conv) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&owner_token),
        Some(json!({ "participant_ids": [second_id] })),
    )
    .await;
    let conv_id = conv["id"].as_str().expect("id").to_string();

    let (status, sent) = send(
        &app,
        "POST",
        &format!("/api/conversations/{conv_id}/messages"),
        Some(&owner_token),
        Some(json!({ "content": "just between us" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "send: {sent:?}");

    let (status, listing) = send(
        &app,
        "GET",
        &format!("/api/conversations/{conv_id}/messages"),
        Some(&second_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listing["data"][0]["content"], "just between us");

    for (token, expected) in [
        (Some(&third_token), StatusCode::FORBIDDEN),
        (None, StatusCode::UNAUTHORIZED),
    ] {
        let (status, _) = send(
            &app,
            "GET",
            &format!("/api/conversations/{conv_id}/messages"),
            token.map(|t| t.as_str()),
            None,
        )
        .await;
        assert_eq!(status, expected, "outsiders stay out");
    }

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/conversations/{conv_id}/messages"),
        Some(&third_token),
        Some(json!({ "content": "let me in" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let unknown = Uuid::new_v4();
    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/conversations/{unknown}/messages"),
        Some(&owner_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let _ = state;
}

#[sqlx::test(migrations = "../migrations")]
async fn sending_bumps_the_conversation_and_read_state_clears_it(pool: PgPool) {
    let (app, _state, ws_id, _owner_id, owner_token, second_id, second_token, _t, _tt) =
        seed_trio(pool).await;

    let (_, conv) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&owner_token),
        Some(json!({ "participant_ids": [second_id] })),
    )
    .await;
    let conv_id = conv["id"].as_str().expect("id").to_string();

    send(
        &app,
        "POST",
        &format!("/api/conversations/{conv_id}/messages"),
        Some(&owner_token),
        Some(json!({ "content": "ping" })),
    )
    .await;

    let (_, before) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&second_token),
        None,
    )
    .await;
    assert!(
        before["data"][0]["last_read_at"].is_null(),
        "unread until the reader says so: {before:?}"
    );

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/conversations/{conv_id}/read"),
        Some(&second_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (_, after) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&second_token),
        None,
    )
    .await;
    assert!(!after["data"][0]["last_read_at"].is_null());
}

#[sqlx::test(migrations = "../migrations")]
async fn edit_delete_and_reactions_follow_authorship(pool: PgPool) {
    let (app, _state, ws_id, _owner_id, owner_token, second_id, second_token, _t, _tt) =
        seed_trio(pool).await;

    let (_, conv) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&owner_token),
        Some(json!({ "participant_ids": [second_id] })),
    )
    .await;
    let conv_id = conv["id"].as_str().expect("id").to_string();

    let (_, sent) = send(
        &app,
        "POST",
        &format!("/api/conversations/{conv_id}/messages"),
        Some(&owner_token),
        Some(json!({ "content": "typo here" })),
    )
    .await;
    let msg_id = sent["id"].as_str().expect("id").to_string();

    let (status, _) = send(
        &app,
        "PATCH",
        &format!("/api/conversations/messages/{msg_id}"),
        Some(&second_token),
        Some(json!({ "content": "not mine to edit" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, edited) = send(
        &app,
        "PATCH",
        &format!("/api/conversations/messages/{msg_id}"),
        Some(&owner_token),
        Some(json!({ "content": "typo fixed" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "author edits: {edited:?}");
    assert_eq!(edited["content"], "typo fixed");
    assert!(!edited["edited_at"].is_null());

    let (status, reaction) = send(
        &app,
        "POST",
        &format!("/api/conversations/messages/{msg_id}/reactions"),
        Some(&second_token),
        Some(json!({ "emoji": "🎉" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "anyone in the thread reacts");
    assert_eq!(reaction["emoji"], "🎉");

    let (_, listing) = send(
        &app,
        "GET",
        &format!("/api/conversations/{conv_id}/messages"),
        Some(&owner_token),
        None,
    )
    .await;
    assert_eq!(listing["data"][0]["reactions"][0]["emoji"], "🎉");

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/conversations/messages/{msg_id}/reactions/%F0%9F%8E%89"),
        Some(&second_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/conversations/messages/{msg_id}"),
        Some(&second_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/conversations/messages/{msg_id}"),
        Some(&owner_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[sqlx::test(migrations = "../migrations")]
async fn message_ids_are_idempotent_across_retries(pool: PgPool) {
    let (app, _state, ws_id, _owner_id, owner_token, second_id, _st, _t, _tt) =
        seed_trio(pool).await;

    let (_, conv) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&owner_token),
        Some(json!({ "participant_ids": [second_id] })),
    )
    .await;
    let conv_id = conv["id"].as_str().expect("id").to_string();
    let client_id = Uuid::new_v4();

    for _ in 0..2 {
        let (status, body) = send(
            &app,
            "POST",
            &format!("/api/conversations/{conv_id}/messages"),
            Some(&owner_token),
            Some(json!({ "content": "sent twice", "id": client_id })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "retry is accepted: {body:?}");
        assert_eq!(body["id"], client_id.to_string());
    }

    let (_, listing) = send(
        &app,
        "GET",
        &format!("/api/conversations/{conv_id}/messages"),
        Some(&owner_token),
        None,
    )
    .await;
    assert_eq!(
        listing["data"].as_array().expect("array").len(),
        1,
        "a retried send stores one message"
    );
}
