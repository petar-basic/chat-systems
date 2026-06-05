use super::common::*;
use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

async fn setup_channel(
    pool: PgPool,
) -> (
    axum::Router,
    std::sync::Arc<crate::state::AppState>,
    uuid::Uuid,
    String,
    uuid::Uuid,
    uuid::Uuid,
) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _email, token) = seed_and_login(&app, &state, "owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, &format!("ws-{}", uuid::Uuid::new_v4())).await;
    let ch_id = seed_channel(
        &state,
        ws_id,
        owner_id,
        &format!("chan-{}", uuid::Uuid::new_v4()),
        false,
    )
    .await;
    (app, state, owner_id, token, ws_id, ch_id)
}

async fn create_message(
    app: &axum::Router,
    token: &str,
    ch_id: uuid::Uuid,
    content: &str,
) -> uuid::Uuid {
    let (status, body) = send(
        app,
        "POST",
        &format!("/api/channels/{ch_id}/messages"),
        Some(token),
        Some(json!({ "content": content })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "create_message should succeed: {body:?}"
    );
    uuid::Uuid::parse_str(body["id"].as_str().expect("message id")).expect("valid uuid")
}

#[sqlx::test(migrations = "../migrations")]
async fn send_message_member_succeeds(pool: PgPool) {
    let (app, _state, _owner, token, _ws, ch_id) = setup_channel(pool).await;
    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/messages"),
        Some(&token),
        Some(json!({ "content": "hello world" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert!(
        body["id"].is_string(),
        "expected message id in body: {body:?}"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn send_message_non_member_forbidden(pool: PgPool) {
    let (app, state, _owner, _token, _ws, ch_id) = setup_channel(pool).await;
    let (_outsider_id, _email, outsider_token) =
        seed_and_login(&app, &state, "outsider", false).await;
    let (status, _body) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/messages"),
        Some(&outsider_token),
        Some(json!({ "content": "let me in" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../migrations")]
async fn send_message_no_token_unauthorized(pool: PgPool) {
    let (app, _state, _owner, _token, _ws, ch_id) = setup_channel(pool).await;
    let (status, _body) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/messages"),
        None,
        Some(json!({ "content": "anon" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn send_message_empty_content_unprocessable(pool: PgPool) {
    let (app, _state, _owner, token, _ws, ch_id) = setup_channel(pool).await;
    let (status, _body) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/messages"),
        Some(&token),
        Some(json!({ "content": "   " })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrations = "../migrations")]
async fn send_message_unknown_channel_not_found(pool: PgPool) {
    let (app, _state, _owner, token, _ws, _ch_id) = setup_channel(pool).await;
    let ghost = uuid::Uuid::new_v4();
    let (status, _body) = send(
        &app,
        "POST",
        &format!("/api/channels/{ghost}/messages"),
        Some(&token),
        Some(json!({ "content": "into the void" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../migrations")]
async fn list_messages_member_succeeds(pool: PgPool) {
    let (app, _state, _owner, token, _ws, ch_id) = setup_channel(pool).await;
    create_message(&app, &token, ch_id, "first").await;
    create_message(&app, &token, ch_id, "second").await;
    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/channels/{ch_id}/messages"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert!(body["data"].is_array(), "expected data array: {body:?}");
}

#[sqlx::test(migrations = "../migrations")]
async fn list_messages_non_member_forbidden(pool: PgPool) {
    let (app, state, _owner, _token, _ws, ch_id) = setup_channel(pool).await;
    let (_outsider, _email, outsider_token) = seed_and_login(&app, &state, "outsider", false).await;
    let (status, _body) = send(
        &app,
        "GET",
        &format!("/api/channels/{ch_id}/messages"),
        Some(&outsider_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../migrations")]
async fn list_messages_no_token_unauthorized(pool: PgPool) {
    let (app, _state, _owner, _token, _ws, ch_id) = setup_channel(pool).await;
    let (status, _body) = send(
        &app,
        "GET",
        &format!("/api/channels/{ch_id}/messages"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn update_message_author_succeeds(pool: PgPool) {
    let (app, _state, _owner, token, _ws, ch_id) = setup_channel(pool).await;
    let msg_id = create_message(&app, &token, ch_id, "original").await;
    let (status, body) = send(
        &app,
        "PATCH",
        &format!("/api/messages/{msg_id}"),
        Some(&token),
        Some(json!({ "content": "edited" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
}

#[sqlx::test(migrations = "../migrations")]
async fn update_message_non_author_forbidden(pool: PgPool) {
    let (app, state, _owner, token, ws_id, ch_id) = setup_channel(pool).await;
    let msg_id = create_message(&app, &token, ch_id, "owner's message").await;
    let (other_id, _email, other_token) = seed_and_login(&app, &state, "member", false).await;
    add_ws_member(&state, ws_id, other_id, "member").await;
    let (status, _body) = send(
        &app,
        "PATCH",
        &format!("/api/messages/{msg_id}"),
        Some(&other_token),
        Some(json!({ "content": "hijack" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../migrations")]
async fn update_message_no_token_unauthorized(pool: PgPool) {
    let (app, _state, _owner, token, _ws, ch_id) = setup_channel(pool).await;
    let msg_id = create_message(&app, &token, ch_id, "original").await;
    let (status, _body) = send(
        &app,
        "PATCH",
        &format!("/api/messages/{msg_id}"),
        None,
        Some(json!({ "content": "edited" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn update_message_unknown_not_found(pool: PgPool) {
    let (app, _state, _owner, token, _ws, _ch_id) = setup_channel(pool).await;
    let ghost = uuid::Uuid::new_v4();
    let (status, _body) = send(
        &app,
        "PATCH",
        &format!("/api/messages/{ghost}"),
        Some(&token),
        Some(json!({ "content": "edited" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../migrations")]
async fn delete_message_author_succeeds(pool: PgPool) {
    let (app, _state, _owner, token, _ws, ch_id) = setup_channel(pool).await;
    let msg_id = create_message(&app, &token, ch_id, "delete me").await;
    let (status, _body) = send(
        &app,
        "DELETE",
        &format!("/api/messages/{msg_id}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[sqlx::test(migrations = "../migrations")]
async fn delete_message_admin_succeeds(pool: PgPool) {
    let (app, state, _owner, token, ws_id, ch_id) = setup_channel(pool).await;
    let msg_id = create_message(&app, &token, ch_id, "owner's message").await;
    let (admin_id, _email, admin_token) = seed_and_login(&app, &state, "wsadmin", false).await;
    add_ws_member(&state, ws_id, admin_id, "admin").await;
    let (status, _body) = send(
        &app,
        "DELETE",
        &format!("/api/messages/{msg_id}"),
        Some(&admin_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[sqlx::test(migrations = "../migrations")]
async fn delete_message_non_author_member_forbidden(pool: PgPool) {
    let (app, state, _owner, token, ws_id, ch_id) = setup_channel(pool).await;
    let msg_id = create_message(&app, &token, ch_id, "owner's message").await;
    let (member_id, _email, member_token) = seed_and_login(&app, &state, "member", false).await;
    add_ws_member(&state, ws_id, member_id, "member").await;
    let (status, _body) = send(
        &app,
        "DELETE",
        &format!("/api/messages/{msg_id}"),
        Some(&member_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../migrations")]
async fn delete_message_no_token_unauthorized(pool: PgPool) {
    let (app, _state, _owner, token, _ws, ch_id) = setup_channel(pool).await;
    let msg_id = create_message(&app, &token, ch_id, "delete me").await;
    let (status, _body) = send(
        &app,
        "DELETE",
        &format!("/api/messages/{msg_id}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn thread_reply_and_list_succeeds(pool: PgPool) {
    let (app, _state, _owner, token, _ws, ch_id) = setup_channel(pool).await;
    let parent = create_message(&app, &token, ch_id, "parent message").await;

    let (reply_status, reply_body) = send(
        &app,
        "POST",
        &format!("/api/messages/{parent}/thread"),
        Some(&token),
        Some(json!({ "content": "a reply" })),
    )
    .await;
    assert_eq!(reply_status, StatusCode::OK, "{reply_body:?}");
    assert!(reply_body["id"].is_string());

    let (list_status, list_body) = send(
        &app,
        "GET",
        &format!("/api/messages/{parent}/thread"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(list_status, StatusCode::OK, "{list_body:?}");
    assert!(list_body["data"].is_array());
}

#[sqlx::test(migrations = "../migrations")]
async fn thread_reply_non_member_forbidden(pool: PgPool) {
    let (app, state, _owner, token, _ws, ch_id) = setup_channel(pool).await;
    let parent = create_message(&app, &token, ch_id, "parent").await;
    let (_outsider, _email, outsider_token) = seed_and_login(&app, &state, "outsider", false).await;
    let (status, _body) = send(
        &app,
        "POST",
        &format!("/api/messages/{parent}/thread"),
        Some(&outsider_token),
        Some(json!({ "content": "intruder reply" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../migrations")]
async fn thread_reply_no_token_unauthorized(pool: PgPool) {
    let (app, _state, _owner, token, _ws, ch_id) = setup_channel(pool).await;
    let parent = create_message(&app, &token, ch_id, "parent").await;
    let (status, _body) = send(
        &app,
        "POST",
        &format!("/api/messages/{parent}/thread"),
        None,
        Some(json!({ "content": "anon reply" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn thread_list_unknown_parent_not_found(pool: PgPool) {
    let (app, _state, _owner, token, _ws, _ch_id) = setup_channel(pool).await;
    let ghost = uuid::Uuid::new_v4();
    let (status, _body) = send(
        &app,
        "GET",
        &format!("/api/messages/{ghost}/thread"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../migrations")]
async fn reaction_add_list_remove_succeeds(pool: PgPool) {
    let (app, _state, _owner, token, _ws, ch_id) = setup_channel(pool).await;
    let msg_id = create_message(&app, &token, ch_id, "react to me").await;

    let (add_status, add_body) = send(
        &app,
        "POST",
        &format!("/api/messages/{msg_id}/reactions"),
        Some(&token),
        Some(json!({ "emoji": "👍" })),
    )
    .await;
    assert_eq!(add_status, StatusCode::OK, "{add_body:?}");
    assert!(add_body["id"].is_string());

    let (list_status, list_body) = send(
        &app,
        "GET",
        &format!("/api/messages/{msg_id}/reactions"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(list_status, StatusCode::OK, "{list_body:?}");
    assert!(list_body["data"].is_array());

    let encoded = "%F0%9F%91%8D";
    let (rm_status, _rm_body) = send(
        &app,
        "DELETE",
        &format!("/api/messages/{msg_id}/reactions/{encoded}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(rm_status, StatusCode::OK);
}

#[sqlx::test(migrations = "../migrations")]
async fn reaction_add_non_member_forbidden(pool: PgPool) {
    let (app, state, _owner, token, _ws, ch_id) = setup_channel(pool).await;
    let msg_id = create_message(&app, &token, ch_id, "react to me").await;
    let (_outsider, _email, outsider_token) = seed_and_login(&app, &state, "outsider", false).await;
    let (status, _body) = send(
        &app,
        "POST",
        &format!("/api/messages/{msg_id}/reactions"),
        Some(&outsider_token),
        Some(json!({ "emoji": "👍" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../migrations")]
async fn reaction_list_no_token_unauthorized(pool: PgPool) {
    let (app, _state, _owner, token, _ws, ch_id) = setup_channel(pool).await;
    let msg_id = create_message(&app, &token, ch_id, "react to me").await;
    let (status, _body) = send(
        &app,
        "GET",
        &format!("/api/messages/{msg_id}/reactions"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn reaction_add_unknown_message_not_found(pool: PgPool) {
    let (app, _state, _owner, token, _ws, _ch_id) = setup_channel(pool).await;
    let ghost = uuid::Uuid::new_v4();
    let (status, _body) = send(
        &app,
        "POST",
        &format!("/api/messages/{ghost}/reactions"),
        Some(&token),
        Some(json!({ "emoji": "👍" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../migrations")]
async fn pin_unpin_and_list_succeeds(pool: PgPool) {
    let (app, _state, _owner, token, _ws, ch_id) = setup_channel(pool).await;
    let msg_id = create_message(&app, &token, ch_id, "pin me").await;

    let (pin_status, _pin_body) = send(
        &app,
        "POST",
        &format!("/api/messages/{msg_id}/pin"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(pin_status, StatusCode::OK);

    let (list_status, list_body) = send(
        &app,
        "GET",
        &format!("/api/channels/{ch_id}/pins"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(list_status, StatusCode::OK, "{list_body:?}");
    assert!(list_body["data"].is_array());

    let (unpin_status, _unpin_body) = send(
        &app,
        "DELETE",
        &format!("/api/messages/{msg_id}/pin"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(unpin_status, StatusCode::OK);
}

#[sqlx::test(migrations = "../migrations")]
async fn pin_message_non_moderator_forbidden(pool: PgPool) {
    let (app, state, _owner, token, ws_id, ch_id) = setup_channel(pool).await;
    let msg_id = create_message(&app, &token, ch_id, "pin me").await;
    let (member_id, _email, member_token) = seed_and_login(&app, &state, "member", false).await;
    add_ws_member(&state, ws_id, member_id, "member").await;
    let (status, _body) = send(
        &app,
        "POST",
        &format!("/api/messages/{msg_id}/pin"),
        Some(&member_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../migrations")]
async fn pin_message_no_token_unauthorized(pool: PgPool) {
    let (app, _state, _owner, token, _ws, ch_id) = setup_channel(pool).await;
    let msg_id = create_message(&app, &token, ch_id, "pin me").await;
    let (status, _body) = send(
        &app,
        "POST",
        &format!("/api/messages/{msg_id}/pin"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn list_pins_non_member_forbidden(pool: PgPool) {
    let (app, state, _owner, _token, _ws, ch_id) = setup_channel(pool).await;
    let (_outsider, _email, outsider_token) = seed_and_login(&app, &state, "outsider", false).await;
    let (status, _body) = send(
        &app,
        "GET",
        &format!("/api/channels/{ch_id}/pins"),
        Some(&outsider_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../migrations")]
async fn search_member_succeeds(pool: PgPool) {
    let (app, _state, _owner, token, ws_id, ch_id) = setup_channel(pool).await;
    create_message(&app, &token, ch_id, "needle in the haystack").await;
    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/search?q=needle&workspace_id={ws_id}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert!(body["data"].is_array(), "expected data array: {body:?}");
}

#[sqlx::test(migrations = "../migrations")]
async fn search_non_member_forbidden(pool: PgPool) {
    let (app, state, _owner, _token, ws_id, _ch_id) = setup_channel(pool).await;
    let (_outsider, _email, outsider_token) = seed_and_login(&app, &state, "outsider", false).await;
    let (status, _body) = send(
        &app,
        "GET",
        &format!("/api/search?q=needle&workspace_id={ws_id}"),
        Some(&outsider_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../migrations")]
async fn search_no_token_unauthorized(pool: PgPool) {
    let (app, _state, _owner, _token, ws_id, _ch_id) = setup_channel(pool).await;
    let (status, _body) = send(
        &app,
        "GET",
        &format!("/api/search?q=needle&workspace_id={ws_id}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn search_empty_query_unprocessable(pool: PgPool) {
    let (app, _state, _owner, token, ws_id, _ch_id) = setup_channel(pool).await;
    let (status, _body) = send(
        &app,
        "GET",
        &format!("/api/search?q=&workspace_id={ws_id}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrations = "../migrations")]
async fn mark_read_member_succeeds(pool: PgPool) {
    let (app, _state, _owner, token, _ws, ch_id) = setup_channel(pool).await;
    let msg_id = create_message(&app, &token, ch_id, "read me").await;
    let (status, _body) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/read"),
        Some(&token),
        Some(json!({ "message_id": msg_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[sqlx::test(migrations = "../migrations")]
async fn mark_read_non_member_forbidden(pool: PgPool) {
    let (app, state, _owner, token, _ws, ch_id) = setup_channel(pool).await;
    let msg_id = create_message(&app, &token, ch_id, "read me").await;
    let (_outsider, _email, outsider_token) = seed_and_login(&app, &state, "outsider", false).await;
    let (status, _body) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/read"),
        Some(&outsider_token),
        Some(json!({ "message_id": msg_id })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../migrations")]
async fn mark_read_no_token_unauthorized(pool: PgPool) {
    let (app, _state, _owner, token, _ws, ch_id) = setup_channel(pool).await;
    let msg_id = create_message(&app, &token, ch_id, "read me").await;
    let (status, _body) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/read"),
        None,
        Some(json!({ "message_id": msg_id })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
