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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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
            Some(json!({ "content": "sent twice", "client_message_id": client_id })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "retry is accepted: {body:?}");
        assert_eq!(
            body["client_message_id"],
            client_id.to_string(),
            "the retry is matched on the sender's key, not on the message id"
        );
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

#[test_macros::db_test(migrations = "../migrations")]
async fn a_client_id_from_another_conversation_never_returns_its_message(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (alice_id, _, alice) = seed_and_login(&app, &state, "idem-alice", false).await;
    let ws_id = seed_workspace(&state, alice_id, "Idempotency WS").await;
    let (bob_id, _, bob) = seed_and_login(&app, &state, "idem-bob", false).await;
    let (carol_id, _, carol) = seed_and_login(&app, &state, "idem-carol", false).await;
    add_ws_member(&state, ws_id, bob_id, "member").await;
    add_ws_member(&state, ws_id, carol_id, "member").await;

    let (_, first) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&alice),
        Some(json!({ "participant_ids": [bob_id] })),
    )
    .await;
    let conv_a = first["id"].as_str().expect("id").to_string();

    let (_, second) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&bob),
        Some(json!({ "participant_ids": [carol_id] })),
    )
    .await;
    let conv_b = second["id"].as_str().expect("id").to_string();

    let shared_id = Uuid::new_v4();
    let (status, sent) = send(
        &app,
        "POST",
        &format!("/api/conversations/{conv_a}/messages"),
        Some(&alice),
        Some(json!({ "content": "a private thing", "client_message_id": shared_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{sent:?}");
    assert_ne!(
        sent["id"].as_str(),
        Some(shared_id.to_string().as_str()),
        "the server owns the message id, not the client"
    );

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/conversations/{conv_b}/messages"),
        Some(&carol),
        Some(json!({ "content": "unrelated", "client_message_id": shared_id })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the same client id in another conversation is not a conflict at all: {body:?}"
    );
    assert_eq!(
        body["content"], "unrelated",
        "and it certainly does not hand back the other conversation's message"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn resending_with_the_same_client_id_is_idempotent(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (alice_id, _, alice) = seed_and_login(&app, &state, "idem-alice", false).await;
    let ws_id = seed_workspace(&state, alice_id, "Idempotency WS").await;
    let (bob_id, _) = seed(&state, "idem-bob", false).await;
    add_ws_member(&state, ws_id, bob_id, "member").await;

    let (_, conv) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&alice),
        Some(json!({ "participant_ids": [bob_id] })),
    )
    .await;
    let conv_id = conv["id"].as_str().expect("id").to_string();

    let client_id = Uuid::new_v4();
    let body = json!({ "content": "sent once", "client_message_id": client_id });

    let (first_status, first) = send(
        &app,
        "POST",
        &format!("/api/conversations/{conv_id}/messages"),
        Some(&alice),
        Some(body.clone()),
    )
    .await;
    assert_eq!(first_status, StatusCode::OK);

    let (retry_status, retry) = send(
        &app,
        "POST",
        &format!("/api/conversations/{conv_id}/messages"),
        Some(&alice),
        Some(body),
    )
    .await;
    assert_eq!(retry_status, StatusCode::OK, "a retry is not an error");
    assert_eq!(retry["id"], first["id"], "and returns the original message");

    let stored: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM conversation_messages WHERE conversation_id::text = $1",
    )
    .bind(&conv_id)
    .fetch_one(&state.pool)
    .await
    .expect("count");
    assert_eq!(stored, 1, "one row, not two");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_nil_or_non_random_client_id_is_refused(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (alice_id, _, alice) = seed_and_login(&app, &state, "idem-alice", false).await;
    let ws_id = seed_workspace(&state, alice_id, "Idempotency WS").await;
    let (bob_id, _) = seed(&state, "idem-bob", false).await;
    add_ws_member(&state, ws_id, bob_id, "member").await;

    let (_, conv) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&alice),
        Some(json!({ "participant_ids": [bob_id] })),
    )
    .await;
    let conv_id = conv["id"].as_str().expect("id").to_string();

    for bad in [
        "00000000-0000-0000-0000-000000000000",
        "00000000-0000-1000-8000-000000000000",
    ] {
        let (status, _) = send(
            &app,
            "POST",
            &format!("/api/conversations/{conv_id}/messages"),
            Some(&alice),
            Some(json!({ "content": "hello", "client_message_id": bad })),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "a client that picks {bad} will collide with itself"
        );
    }
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_over_long_conversation_reaction_is_a_400_not_a_500(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (alice_id, _, alice) = seed_and_login(&app, &state, "emoji-alice", false).await;
    let ws_id = seed_workspace(&state, alice_id, "Emoji WS").await;
    let (bob_id, _) = seed(&state, "emoji-bob", false).await;
    add_ws_member(&state, ws_id, bob_id, "member").await;

    let (_, conv) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&alice),
        Some(json!({ "participant_ids": [bob_id] })),
    )
    .await;
    let conv_id = conv["id"].as_str().expect("id").to_string();

    let (_, msg) = send(
        &app,
        "POST",
        &format!("/api/conversations/{conv_id}/messages"),
        Some(&alice),
        Some(json!({ "content": "react to me" })),
    )
    .await;
    let msg_id = msg["id"].as_str().expect("id").to_string();

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/conversations/messages/{msg_id}/reactions"),
        Some(&alice),
        Some(json!({ "emoji": "x".repeat(60) })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_dm_reply_hangs_off_its_parent_and_stays_out_of_the_feed(pool: PgPool) {
    let (app, _state, ws_id, _owner_id, owner_token, second_id, second_token, _t, _tt) =
        seed_trio(pool).await;

    let (_, conversation) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&owner_token),
        Some(json!({ "participant_ids": [second_id] })),
    )
    .await;
    let conv_id = conversation["id"].as_str().expect("id").to_string();

    let (_, parent) = send(
        &app,
        "POST",
        &format!("/api/conversations/{conv_id}/messages"),
        Some(&owner_token),
        Some(json!({ "content": "can you look at the deploy?" })),
    )
    .await;
    let parent_id = parent["id"].as_str().expect("id").to_string();

    let (status, reply) = send(
        &app,
        "POST",
        &format!("/api/conversations/{conv_id}/messages"),
        Some(&second_token),
        Some(json!({ "content": "on it", "thread_parent_id": parent_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reply:?}");
    assert_eq!(reply["thread_parent_id"], parent_id);

    let (status, thread) = send(
        &app,
        "GET",
        &format!("/api/conversations/messages/{parent_id}/thread"),
        Some(&owner_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let replies = thread["data"].as_array().expect("array");
    assert_eq!(replies.len(), 1, "{thread:?}");
    assert_eq!(replies[0]["content"], "on it");

    let (_, feed) = send(
        &app,
        "GET",
        &format!("/api/conversations/{conv_id}/messages"),
        Some(&owner_token),
        None,
    )
    .await;
    let messages = feed["data"].as_array().expect("array");
    assert_eq!(
        messages.len(),
        1,
        "a reply belongs to its thread, not the feed: {feed:?}"
    );
    assert_eq!(messages[0]["id"], parent_id);
    assert_eq!(messages[0]["reply_count"], 1);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_dm_thread_is_one_level_deep(pool: PgPool) {
    let (app, _state, ws_id, _owner_id, owner_token, second_id, _second_token, _t, _tt) =
        seed_trio(pool).await;

    let (_, conversation) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&owner_token),
        Some(json!({ "participant_ids": [second_id] })),
    )
    .await;
    let conv_id = conversation["id"].as_str().expect("id").to_string();

    let (_, parent) = send(
        &app,
        "POST",
        &format!("/api/conversations/{conv_id}/messages"),
        Some(&owner_token),
        Some(json!({ "content": "root" })),
    )
    .await;
    let parent_id = parent["id"].as_str().expect("id").to_string();

    let (_, reply) = send(
        &app,
        "POST",
        &format!("/api/conversations/{conv_id}/messages"),
        Some(&owner_token),
        Some(json!({ "content": "first", "thread_parent_id": parent_id })),
    )
    .await;
    let reply_id = reply["id"].as_str().expect("id").to_string();

    let (_, nested) = send(
        &app,
        "POST",
        &format!("/api/conversations/{conv_id}/messages"),
        Some(&owner_token),
        Some(json!({ "content": "second", "thread_parent_id": reply_id })),
    )
    .await;
    assert_eq!(
        nested["thread_parent_id"], parent_id,
        "replying to a reply joins the same thread"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_thread_belongs_to_its_own_conversation(pool: PgPool) {
    let (
        app,
        _state,
        ws_id,
        _owner_id,
        owner_token,
        second_id,
        _second_token,
        third_id,
        third_token,
    ) = seed_trio(pool).await;

    let (_, first) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&owner_token),
        Some(json!({ "participant_ids": [second_id] })),
    )
    .await;
    let first_id = first["id"].as_str().expect("id").to_string();

    let (_, other) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&owner_token),
        Some(json!({ "participant_ids": [third_id] })),
    )
    .await;
    let other_id = other["id"].as_str().expect("id").to_string();

    let (_, parent) = send(
        &app,
        "POST",
        &format!("/api/conversations/{first_id}/messages"),
        Some(&owner_token),
        Some(json!({ "content": "private" })),
    )
    .await;
    let parent_id = parent["id"].as_str().expect("id").to_string();

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/conversations/{other_id}/messages"),
        Some(&owner_token),
        Some(json!({ "content": "wrong room", "thread_parent_id": parent_id })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/conversations/messages/{parent_id}/thread"),
        Some(&third_token),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a thread is as private as the conversation it is in"
    );
}

/// CS-041 stopped a guest discovering the directory. This is the other way back
/// to it: a guest who already knows a user id could open a private channel to
/// anyone in the company.
#[sqlx::test(migrations = "../migrations")]
async fn a_guest_can_only_open_a_conversation_with_somebody_they_share_a_channel_with(
    pool: PgPool,
) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "dm-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Guest DMs").await;

    let (mate_id, _, _) = seed_and_login(&app, &state, "dm-mate", false).await;
    add_ws_member(&state, ws_id, mate_id, "member").await;
    let (stranger_id, _, _) = seed_and_login(&app, &state, "dm-stranger", false).await;
    add_ws_member(&state, ws_id, stranger_id, "member").await;

    let (guest_id, _, guest_token) = seed_and_login(&app, &state, "dm-guest", false).await;
    add_ws_member(&state, ws_id, guest_id, "guest").await;

    let shared = seed_channel(&state, ws_id, owner_id, "shared-room", true).await;
    for member in [mate_id, guest_id] {
        state
            .workspace_service
            .repo
            .add_channel_member(
                shared,
                member,
                &crate::workspace::models::ChannelRole::Member,
            )
            .await
            .expect("add to the shared channel");
    }

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&guest_token),
        Some(json!({ "participant_ids": [mate_id] })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "somebody they share a room with: {body:?}"
    );

    let (status, refused) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&guest_token),
        Some(json!({ "participant_ids": [stranger_id] })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "somebody they do not");

    let (status, unknown) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&guest_token),
        Some(json!({ "participant_ids": [Uuid::new_v4()] })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(
        unknown, refused,
        "an id that does not exist and one that is out of reach answer identically, \
         or the endpoint is the directory again"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn a_member_still_opens_a_conversation_with_anyone_in_the_workspace(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "dm-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Open DMs").await;
    let (one_id, _, one_token) = seed_and_login(&app, &state, "dm-one", false).await;
    add_ws_member(&state, ws_id, one_id, "member").await;
    let (two_id, _, _) = seed_and_login(&app, &state, "dm-two", false).await;
    add_ws_member(&state, ws_id, two_id, "member").await;

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&one_token),
        Some(json!({ "participant_ids": [two_id] })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the restriction is on the guest role, not on the workspace: {body:?}"
    );
}

/// The rule is on creating one. Somebody who has since left the shared channel
/// keeps the thread they already had.
#[sqlx::test(migrations = "../migrations")]
async fn an_existing_conversation_survives_the_guest_leaving_the_shared_channel(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "dm-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Leftovers").await;
    let (mate_id, _, _) = seed_and_login(&app, &state, "dm-mate", false).await;
    add_ws_member(&state, ws_id, mate_id, "member").await;
    let (guest_id, _, guest_token) = seed_and_login(&app, &state, "dm-guest", false).await;
    add_ws_member(&state, ws_id, guest_id, "guest").await;

    let shared = seed_channel(&state, ws_id, owner_id, "temporary", true).await;
    for member in [mate_id, guest_id] {
        state
            .workspace_service
            .repo
            .add_channel_member(
                shared,
                member,
                &crate::workspace::models::ChannelRole::Member,
            )
            .await
            .expect("add to the shared channel");
    }

    let (status, conversation) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&guest_token),
        Some(json!({ "participant_ids": [mate_id] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{conversation:?}");
    let conversation_id = conversation["id"].as_str().expect("conversation id");

    sqlx::query("DELETE FROM channel_members WHERE channel_id = $1 AND user_id = $2")
        .bind(shared)
        .bind(guest_id)
        .execute(&state.pool)
        .await
        .expect("remove the guest from the channel");

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/conversations/{conversation_id}/messages"),
        Some(&guest_token),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "participation still guards reading it"
    );
}
