use super::common::*;
use crate::messaging::routes::expand_mentions;
use crate::workspace::models::ChannelRole;
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

async fn store_preview(
    state: &crate::state::AppState,
    msg_id: uuid::Uuid,
    content: &str,
    source: &str,
) -> bool {
    let unfurls = json!([{ "url": "https://example.com", "title": "Example" }]);
    let mut tx = state.pool.begin().await.expect("begin");
    let stored = state
        .message_repo
        .store_unfurls_in(&mut tx, msg_id, content, source, &unfurls, &json!([]))
        .await
        .expect("store");
    tx.commit().await.expect("commit");
    stored.is_some()
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_preview_is_stored_once_per_set_of_links(pool: PgPool) {
    let (app, state, _owner, token, _ws, ch_id) = setup_channel(pool).await;
    let content = "see https://example.com";
    let msg_id = create_message(&app, &token, ch_id, content).await;

    assert!(store_preview(&state, msg_id, content, "a").await);
    assert!(
        !store_preview(&state, msg_id, content, "a").await,
        "a redelivered event must not write the same previews again"
    );
    assert!(
        !store_preview(&state, msg_id, "edited meanwhile", "b").await,
        "previews for content that has since changed are discarded"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn hide_unfurls_author_succeeds_and_sticks(pool: PgPool) {
    let (app, state, _owner, token, _ws, ch_id) = setup_channel(pool).await;
    let content = "see https://example.com";
    let msg_id = create_message(&app, &token, ch_id, content).await;
    assert!(store_preview(&state, msg_id, content, "a").await);

    let (status, body) = send(
        &app,
        "DELETE",
        &format!("/api/messages/{msg_id}/unfurls"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert!(body["metadata"].get("unfurls").is_none(), "{body:?}");
    assert_eq!(body["metadata"]["unfurls_hidden"], json!(true));
    assert!(
        !store_preview(&state, msg_id, content, "b").await,
        "a removed preview does not come back on the next unfurl"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn hide_unfurls_non_author_forbidden(pool: PgPool) {
    let (app, state, _owner, token, ws_id, ch_id) = setup_channel(pool).await;
    let msg_id = create_message(&app, &token, ch_id, "https://example.com").await;
    let (other_id, _email, other_token) = seed_and_login(&app, &state, "member", false).await;
    add_ws_member(&state, ws_id, other_id, "member").await;
    let (status, _body) = send(
        &app,
        "DELETE",
        &format!("/api/messages/{msg_id}/unfurls"),
        Some(&other_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn hide_unfurls_no_token_unauthorized(pool: PgPool) {
    let (app, _state, _owner, token, _ws, ch_id) = setup_channel(pool).await;
    let msg_id = create_message(&app, &token, ch_id, "https://example.com").await;
    let (status, _body) = send(
        &app,
        "DELETE",
        &format!("/api/messages/{msg_id}/unfurls"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn hide_unfurls_unknown_not_found(pool: PgPool) {
    let (app, _state, _owner, token, _ws, _ch_id) = setup_channel(pool).await;
    let ghost = uuid::Uuid::new_v4();
    let (status, _body) = send(
        &app,
        "DELETE",
        &format!("/api/messages/{ghost}/unfurls"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_workspace_can_turn_previews_off(pool: PgPool) {
    let (app, state, _owner, token, ws_id, ch_id) = setup_channel(pool).await;
    let msg_id = create_message(&app, &token, ch_id, "https://example.com").await;
    let target = state
        .message_repo
        .find_unfurl_target(msg_id)
        .await
        .expect("target")
        .expect("message");
    assert!(target.previews_enabled, "previews are on by default");

    let (status, body) = send(
        &app,
        "PATCH",
        &format!("/api/workspaces/{ws_id}"),
        Some(&token),
        Some(json!({ "link_previews": false })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["settings"]["link_previews"], json!(false));

    let target = state
        .message_repo
        .find_unfurl_target(msg_id)
        .await
        .expect("target")
        .expect("message");
    assert!(!target.previews_enabled);
}

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
async fn thread_reply_and_list_succeeds(pool: PgPool) {
    let (app, _state, _owner, token, _ws, ch_id) = setup_channel(pool).await;
    let parent = create_message(&app, &token, ch_id, "parent message").await;

    let (reply_status, reply_body) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/messages"),
        Some(&token),
        Some(json!({ "content": "a reply", "thread_parent_id": parent })),
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

#[test_macros::db_test(migrations = "../migrations")]
async fn thread_reply_non_member_forbidden(pool: PgPool) {
    let (app, state, _owner, token, _ws, ch_id) = setup_channel(pool).await;
    let parent = create_message(&app, &token, ch_id, "parent").await;
    let (_outsider, _email, outsider_token) = seed_and_login(&app, &state, "outsider", false).await;
    let (status, _body) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/messages"),
        Some(&outsider_token),
        Some(json!({ "content": "intruder reply", "thread_parent_id": parent })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn thread_reply_no_token_unauthorized(pool: PgPool) {
    let (app, _state, _owner, token, _ws, ch_id) = setup_channel(pool).await;
    let parent = create_message(&app, &token, ch_id, "parent").await;
    let (status, _body) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/messages"),
        None,
        Some(json!({ "content": "anon reply", "thread_parent_id": parent })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
async fn search_excludes_private_channels_for_non_channel_member(pool: PgPool) {
    let (app, state, owner_id, owner_token, ws_id, _ch_id) = setup_channel(pool).await;
    let private_ch = seed_channel(
        &state,
        ws_id,
        owner_id,
        &format!("secret-{}", uuid::Uuid::new_v4()),
        true,
    )
    .await;
    create_message(&app, &owner_token, private_ch, "classified needle").await;

    let (member_id, _email, member_token) = seed_and_login(&app, &state, "member", false).await;
    add_ws_member(&state, ws_id, member_id, "member").await;

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/search?q=needle&workspace_id={ws_id}"),
        Some(&member_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(
        body["data"].as_array().expect("data array").len(),
        0,
        "workspace member who is not in the private channel must not see its messages: {body:?}"
    );

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/search?q=needle&workspace_id={ws_id}"),
        Some(&owner_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(
        body["data"].as_array().expect("data array").len(),
        1,
        "channel member must see the private channel message: {body:?}"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
async fn mark_workspace_read_clears_every_channel(pool: PgPool) {
    let (app, state, owner, token, ws_id, ch_id) = setup_channel(pool).await;
    let (colleague, colleague_token) = colleague_in_channel(&app, &state, ws_id, ch_id).await;
    let other = seed_channel(&state, ws_id, owner, "elsewhere", false).await;
    state
        .workspace_service
        .repo
        .add_channel_member(other, colleague, &ChannelRole::Member)
        .await
        .expect("add channel member");
    create_message(&app, &colleague_token, ch_id, "one").await;
    create_message(&app, &colleague_token, other, "two").await;

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/read"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/channels/unread"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["channel_ids"], json!([]));
}

#[test_macros::db_test(migrations = "../migrations")]
async fn mark_workspace_read_access(pool: PgPool) {
    let (app, state, _owner, _token, ws_id, _ch_id) = setup_channel(pool).await;
    let (_outsider, _email, outsider_token) = seed_and_login(&app, &state, "outsider", false).await;
    let path = format!("/api/workspaces/{ws_id}/read");
    let (status, _) = send(&app, "POST", &path, Some(&outsider_token), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = send(&app, "POST", &path, None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

async fn colleague_in_channel(
    app: &axum::Router,
    state: &crate::state::AppState,
    ws_id: uuid::Uuid,
    ch_id: uuid::Uuid,
) -> (uuid::Uuid, String) {
    let (colleague_id, _email, colleague_token) =
        seed_and_login(app, state, "colleague", false).await;
    add_ws_member(state, ws_id, colleague_id, "member").await;
    state
        .workspace_service
        .repo
        .add_channel_member(ch_id, colleague_id, &ChannelRole::Member)
        .await
        .expect("add channel member");
    (colleague_id, colleague_token)
}

async fn mark_unread_as(
    app: &axum::Router,
    token: &str,
    ch_id: uuid::Uuid,
    msg_id: uuid::Uuid,
) -> (StatusCode, serde_json::Value) {
    send(
        app,
        "POST",
        &format!("/api/channels/{ch_id}/unread"),
        Some(token),
        Some(json!({ "message_id": msg_id })),
    )
    .await
}

#[test_macros::db_test(migrations = "../migrations")]
async fn mark_unread_counts_from_the_marked_message(pool: PgPool) {
    let (app, state, _owner, token, ws_id, ch_id) = setup_channel(pool).await;
    let (_colleague, colleague_token) = colleague_in_channel(&app, &state, ws_id, ch_id).await;
    let first = create_message(&app, &colleague_token, ch_id, "one").await;
    let second = create_message(&app, &colleague_token, ch_id, "two").await;
    let third = create_message(&app, &colleague_token, ch_id, "three").await;
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/read"),
        Some(&token),
        Some(json!({ "message_id": third })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = mark_unread_as(&app, &token, ch_id, second).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["unread_count"], json!(2));
    assert_eq!(body["last_read_message_id"], json!(first.to_string()));

    let (_, body) = mark_unread_as(&app, &token, ch_id, first).await;
    assert_eq!(body["unread_count"], json!(3));
    assert_eq!(body["last_read_message_id"], json!(null));

    let (_, body) = mark_unread_as(&app, &token, ch_id, third).await;
    assert_eq!(body["unread_count"], json!(1));
    assert_eq!(body["last_read_message_id"], json!(second.to_string()));

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/channels/unread"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert!(
        body["channel_ids"]
            .as_array()
            .expect("channel ids")
            .contains(&json!(ch_id.to_string())),
        "the channel is unread again: {body:?}"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn mark_unread_restores_a_mention(pool: PgPool) {
    let (app, state, owner, token, ws_id, ch_id) = setup_channel(pool).await;
    let (_colleague, colleague_token) = colleague_in_channel(&app, &state, ws_id, ch_id).await;
    let mention = create_message(
        &app,
        &colleague_token,
        ch_id,
        &format!("@[Owner]({owner}) look"),
    )
    .await;
    state
        .notification_repo
        .create(
            owner,
            ws_id,
            &crate::notifications::models::NotificationType::Mention,
            "You were mentioned",
            None,
            &json!({ "channel_id": ch_id, "message_id": mention }),
        )
        .await
        .expect("notification");

    let (status, body) = mark_unread_as(&app, &token, ch_id, mention).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["unread_count"], json!(1));
    assert_eq!(body["mention_count"], json!(1));
}

#[test_macros::db_test(migrations = "../migrations")]
async fn mark_unread_message_from_another_channel_not_found(pool: PgPool) {
    let (app, state, owner, token, ws_id, ch_id) = setup_channel(pool).await;
    let other = seed_channel(&state, ws_id, owner, "elsewhere", false).await;
    let msg_id = create_message(&app, &token, other, "not here").await;
    let (status, _body) = mark_unread_as(&app, &token, ch_id, msg_id).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn mark_unread_non_member_forbidden(pool: PgPool) {
    let (app, state, _owner, token, _ws, ch_id) = setup_channel(pool).await;
    let msg_id = create_message(&app, &token, ch_id, "read me").await;
    let (_outsider, _email, outsider_token) = seed_and_login(&app, &state, "outsider", false).await;
    let (status, _body) = mark_unread_as(&app, &outsider_token, ch_id, msg_id).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn mark_unread_no_token_unauthorized(pool: PgPool) {
    let (app, _state, _owner, token, _ws, ch_id) = setup_channel(pool).await;
    let msg_id = create_message(&app, &token, ch_id, "read me").await;
    let (status, _body) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/unread"),
        None,
        Some(json!({ "message_id": msg_id })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

async fn join_channel(state: &crate::state::AppState, ch_id: uuid::Uuid, user_id: uuid::Uuid) {
    state
        .workspace_service
        .repo
        .add_channel_member(ch_id, user_id, &ChannelRole::Member)
        .await
        .expect("add channel member");
}

async fn unreads(
    app: &axum::Router,
    token: &str,
    ws_id: uuid::Uuid,
    query: &str,
) -> serde_json::Value {
    let (status, body) = send(
        app,
        "GET",
        &format!("/api/workspaces/{ws_id}/unreads{query}"),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    body
}

fn group_ids(body: &serde_json::Value) -> Vec<String> {
    body["data"]
        .as_array()
        .expect("data")
        .iter()
        .map(|g| g["channel_id"].as_str().expect("channel id").to_string())
        .collect()
}

#[test_macros::db_test(migrations = "../migrations")]
async fn unreads_group_by_channel_newest_first_and_muted_last(pool: PgPool) {
    let (app, state, owner, token, ws_id, _ch) = setup_channel(pool).await;
    let (colleague, _email, colleague_token) =
        seed_and_login(&app, &state, "colleague", false).await;
    add_ws_member(&state, ws_id, colleague, "member").await;
    let mut channels = Vec::new();
    for name in ["alpha", "beta", "gamma", "delta"] {
        let ch = seed_channel(&state, ws_id, owner, name, false).await;
        join_channel(&state, ch, colleague).await;
        channels.push(ch);
    }
    let (alpha, beta, gamma, delta) = (channels[0], channels[1], channels[2], channels[3]);

    create_message(&app, &colleague_token, alpha, "a1").await;
    create_message(&app, &colleague_token, beta, "b1").await;
    create_message(&app, &colleague_token, beta, "b2").await;
    create_message(&app, &token, beta, "mine").await;
    let read = create_message(&app, &colleague_token, delta, "d1").await;
    create_message(&app, &colleague_token, gamma, "c1").await;
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{delta}/read"),
        Some(&token),
        Some(json!({ "message_id": read })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    sqlx::query("UPDATE channel_members SET muted = true WHERE channel_id = $1 AND user_id = $2")
        .bind(beta)
        .bind(owner)
        .execute(&state.pool)
        .await
        .expect("mute");

    let body = unreads(&app, &token, ws_id, "").await;
    assert_eq!(
        group_ids(&body),
        vec![gamma.to_string(), alpha.to_string(), beta.to_string()],
        "newest activity first, muted last, read channels absent: {body:?}"
    );
    let beta_group = &body["data"][2];
    assert_eq!(beta_group["muted"], json!(true));
    assert_eq!(beta_group["unread_count"], json!(2));
    let contents: Vec<&str> = beta_group["messages"]
        .as_array()
        .expect("messages")
        .iter()
        .map(|m| m["content"].as_str().expect("content"))
        .collect();
    assert_eq!(
        contents,
        vec!["b1", "b2"],
        "oldest first, the reader's own message left out"
    );
    assert_eq!(body["next_offset"], json!(null));
}

#[test_macros::db_test(migrations = "../migrations")]
async fn unreads_page_by_channel(pool: PgPool) {
    let (app, state, owner, token, ws_id, _ch) = setup_channel(pool).await;
    let (colleague, _email, colleague_token) =
        seed_and_login(&app, &state, "colleague", false).await;
    add_ws_member(&state, ws_id, colleague, "member").await;
    for name in ["one", "two", "three"] {
        let ch = seed_channel(&state, ws_id, owner, name, false).await;
        join_channel(&state, ch, colleague).await;
        create_message(&app, &colleague_token, ch, name).await;
    }

    let first = unreads(&app, &token, ws_id, "?limit=2").await;
    assert_eq!(group_ids(&first).len(), 2);
    assert_eq!(first["next_offset"], json!(2));
    let second = unreads(&app, &token, ws_id, "?limit=2&offset=2").await;
    assert_eq!(group_ids(&second).len(), 1);
    assert_eq!(second["next_offset"], json!(null));
}

#[test_macros::db_test(migrations = "../migrations")]
async fn unreads_hold_a_guest_to_their_channels(pool: PgPool) {
    let (app, state, owner, token, ws_id, _ch) = setup_channel(pool).await;
    let (guest, _email, guest_token) = seed_and_login(&app, &state, "guest", false).await;
    add_ws_member(&state, ws_id, guest, "guest").await;
    let shared = seed_channel(&state, ws_id, owner, "shared", false).await;
    let other = seed_channel(&state, ws_id, owner, "other", false).await;
    join_channel(&state, shared, guest).await;
    create_message(&app, &token, shared, "for the guest").await;
    create_message(&app, &token, other, "not for the guest").await;

    let body = unreads(&app, &guest_token, ws_id, "").await;
    assert_eq!(group_ids(&body), vec![shared.to_string()], "{body:?}");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn unreads_outsider_forbidden(pool: PgPool) {
    let (app, state, _owner, _token, ws_id, _ch) = setup_channel(pool).await;
    let (_outsider, _email, outsider_token) = seed_and_login(&app, &state, "outsider", false).await;
    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/unreads"),
        Some(&outsider_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

async fn mark_online(state: &crate::state::AppState, ws_id: uuid::Uuid, user_id: uuid::Uuid) {
    let mut conn = state.redis.clone();
    let expires_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
        + 60;
    let _: redis::RedisResult<()> = redis::cmd("ZADD")
        .arg(format!("presence:ws:{ws_id}"))
        .arg(expires_at)
        .arg(user_id.to_string())
        .query_async(&mut conn)
        .await;
}

#[test_macros::db_test(migrations = "../migrations")]
async fn channel_broadcast_notifies_every_member_except_the_author(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (author_id, _, _) = seed_and_login(&app, &state, "mention-author", false).await;
    let ws_id = seed_workspace(&state, author_id, "Broadcast WS").await;
    let ch_id = seed_channel(&state, ws_id, author_id, "main", false).await;

    let (first, _) = seed(&state, "mention-first", false).await;
    let (second, _) = seed(&state, "mention-second", false).await;
    for member in [first, second] {
        add_ws_member(&state, ws_id, member, "member").await;
        state
            .workspace_service
            .repo
            .add_channel_member(ch_id, member, &ChannelRole::Member)
            .await
            .expect("add channel member");
    }

    let mentioned = expand_mentions(
        &state,
        ch_id,
        author_id,
        "@[channel](channel) deploying now",
    )
    .await;

    let mut expected = vec![first, second];
    expected.sort();
    assert_eq!(
        mentioned, expected,
        "every other channel member is notified, the author is not"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn here_only_reaches_members_with_a_live_connection(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (author_id, _, _) = seed_and_login(&app, &state, "mention-author", false).await;
    let ws_id = seed_workspace(&state, author_id, "Here WS").await;
    let ch_id = seed_channel(&state, ws_id, author_id, "main", false).await;

    let (online_member, _) = seed(&state, "mention-online", false).await;
    let (offline_member, _) = seed(&state, "mention-offline", false).await;
    for member in [online_member, offline_member] {
        add_ws_member(&state, ws_id, member, "member").await;
        state
            .workspace_service
            .repo
            .add_channel_member(ch_id, member, &ChannelRole::Member)
            .await
            .expect("add channel member");
    }
    mark_online(&state, ws_id, online_member).await;

    let mentioned = expand_mentions(&state, ch_id, author_id, "@[here](here) anyone around?").await;

    assert_eq!(
        mentioned,
        vec![online_member],
        "@here skips members who are not connected"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_named_mention_alongside_a_broadcast_is_not_delivered_twice(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (author_id, _, _) = seed_and_login(&app, &state, "mention-author", false).await;
    let ws_id = seed_workspace(&state, author_id, "Dedupe WS").await;
    let ch_id = seed_channel(&state, ws_id, author_id, "main", false).await;

    let (member_id, _) = seed(&state, "mention-member", false).await;
    add_ws_member(&state, ws_id, member_id, "member").await;
    state
        .workspace_service
        .repo
        .add_channel_member(ch_id, member_id, &ChannelRole::Member)
        .await
        .expect("add channel member");

    let content = format!("@[Member]({member_id}) and @[channel](channel)");
    let mentioned = expand_mentions(&state, ch_id, author_id, &content).await;

    assert_eq!(mentioned, vec![member_id]);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_message_without_a_broadcast_only_notifies_the_named_user(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (author_id, _, _) = seed_and_login(&app, &state, "mention-author", false).await;
    let ws_id = seed_workspace(&state, author_id, "Named WS").await;
    let ch_id = seed_channel(&state, ws_id, author_id, "main", false).await;

    let (named, _) = seed(&state, "mention-named", false).await;
    let (bystander, _) = seed(&state, "mention-bystander", false).await;
    for member in [named, bystander] {
        add_ws_member(&state, ws_id, member, "member").await;
        state
            .workspace_service
            .repo
            .add_channel_member(ch_id, member, &ChannelRole::Member)
            .await
            .expect("add channel member");
    }

    let content = format!("just you @[Named]({named})");
    let mentioned = expand_mentions(&state, ch_id, author_id, &content).await;

    assert_eq!(mentioned, vec![named], "bystanders stay unnotified");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn search_holds_guests_to_the_channels_they_belong_to(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) =
        seed_and_login(&app, &state, "search-guest-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Guest Search WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "public-room", false).await;

    let (guest_id, _, guest_token) = seed_and_login(&app, &state, "search-guest", false).await;
    add_ws_member(&state, ws_id, guest_id, "guest").await;
    let (member_id, _, member_token) = seed_and_login(&app, &state, "search-member", false).await;
    add_ws_member(&state, ws_id, member_id, "member").await;

    let needle = format!("pineapple{}", uuid::Uuid::new_v4().simple());
    let (status, msg) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/messages"),
        Some(&owner_token),
        Some(json!({ "content": needle.clone() })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "seed message: {msg:?}");

    let uri = format!("/api/search?workspace_id={ws_id}&q={needle}");

    let (status, body) = send(&app, "GET", &uri, Some(&member_token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["data"].as_array().map(Vec::len),
        Some(1),
        "a plain member finds public-channel messages without joining: {body:?}"
    );

    let (status, body) = send(&app, "GET", &uri, Some(&guest_token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["data"].as_array().map(Vec::len),
        Some(0),
        "search must not be the one door that opens public channels to a guest: {body:?}"
    );

    let (status, _b) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/members"),
        Some(&owner_token),
        Some(json!({ "user_id": guest_id })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "owner adds the guest to the channel"
    );

    let (status, body) = send(&app, "GET", &uri, Some(&guest_token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["data"].as_array().map(Vec::len),
        Some(1),
        "once a member of the channel the guest finds it: {body:?}"
    );
}

/// The reason this ticket exists: `to_tsvector('english', …)` neither stems
/// Serbian correctly nor folds diacritics, so a team writing in it could not
/// find its own messages.
#[test_macros::db_test(migrations = "../migrations")]
async fn diacritics_match_in_both_directions(pool: PgPool) {
    let (app, _state, _owner, token, ws_id, ch_id) = setup_channel(pool).await;
    create_message(&app, &token, ch_id, "sastanak u četvrtak").await;
    create_message(&app, &token, ch_id, "deployment on cetvrtak too").await;

    for query in ["četvrtak", "cetvrtak", "CETVRTAK"] {
        let (status, body) = send(
            &app,
            "GET",
            &format!("/api/search?q={}&workspace_id={ws_id}", urlencode(query)),
            Some(&token),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body:?}");
        let hits = body["data"].as_array().expect("data array").len();
        assert_eq!(hits, 2, "'{query}' should reach both spellings: {body:?}");
    }
}

/// What people actually type into a chat search is half a word, which
/// `to_tsvector` cannot answer at all.
#[test_macros::db_test(migrations = "../migrations")]
async fn a_substring_and_a_near_miss_still_find_the_message(pool: PgPool) {
    let (app, _state, _owner, token, ws_id, ch_id) = setup_channel(pool).await;
    create_message(&app, &token, ch_id, "the deployment pipeline is red").await;

    for query in ["deploym", "deploymnet"] {
        let (status, body) = send(
            &app,
            "GET",
            &format!("/api/search?q={query}&workspace_id={ws_id}"),
            Some(&token),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body:?}");
        assert_eq!(
            body["data"].as_array().expect("data array").len(),
            1,
            "'{query}' should still find it: {body:?}"
        );
    }
}

/// A fuzzy hit is useful but must never outrank the message that actually
/// contains the word.
#[test_macros::db_test(migrations = "../migrations")]
async fn an_exact_match_outranks_a_fuzzy_one(pool: PgPool) {
    let (app, _state, _owner, token, ws_id, ch_id) = setup_channel(pool).await;
    create_message(&app, &token, ch_id, "something about deploymen and more").await;
    let exact = create_message(&app, &token, ch_id, "deployment").await;

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/search?q=deployment&workspace_id={ws_id}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let hits = body["data"].as_array().expect("data array");
    assert!(hits.len() >= 2, "both should match: {body:?}");
    assert_eq!(
        hits[0]["id"].as_str().expect("id"),
        exact.to_string(),
        "the message containing the word comes first"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn exact_matches_that_fill_the_page_leave_no_room_for_fuzzy_ones(pool: PgPool) {
    let (app, _state, _owner, token, ws_id, ch_id) = setup_channel(pool).await;
    create_message(&app, &token, ch_id, "something about deploymen and more").await;
    let first = create_message(&app, &token, ch_id, "deployment one").await;
    let second = create_message(&app, &token, ch_id, "deployment two").await;

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/search?q=deployment&workspace_id={ws_id}&limit=2"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let mut ids: Vec<String> = body["data"]
        .as_array()
        .expect("data array")
        .iter()
        .map(|hit| hit["id"].as_str().expect("id").to_string())
        .collect();
    ids.sort();
    let mut expected = vec![first.to_string(), second.to_string()];
    expected.sort();
    assert_eq!(ids, expected, "a full page of exact matches: {body:?}");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_term_with_no_exact_match_still_finds_its_fuzzy_one(pool: PgPool) {
    let (app, _state, _owner, token, ws_id, ch_id) = setup_channel(pool).await;
    let message = create_message(&app, &token, ch_id, "the deployment pipeline is green").await;

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/search?q=deploymen&workspace_id={ws_id}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let hits = body["data"].as_array().expect("data array");
    assert_eq!(hits.len(), 1, "{body:?}");
    assert_eq!(hits[0]["id"].as_str().expect("id"), message.to_string());
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_dm_is_searchable_by_its_participants_and_nobody_else(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "dm-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "DM search").await;
    let (partner_id, _, partner_token) = seed_and_login(&app, &state, "dm-partner", false).await;
    add_ws_member(&state, ws_id, partner_id, "member").await;
    let (outsider_id, _, outsider_token) = seed_and_login(&app, &state, "dm-outsider", false).await;
    add_ws_member(&state, ws_id, outsider_id, "member").await;

    let (status, conversation) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&owner_token),
        Some(json!({ "participant_ids": [partner_id] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{conversation:?}");
    let conversation_id = conversation["id"].as_str().expect("conversation id");

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{conversation_id}/messages"),
        Some(&owner_token),
        Some(json!({ "content": "the séance is at six" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    for token in [&owner_token, &partner_token] {
        let (status, body) = send(
            &app,
            "GET",
            &format!("/api/search?q=seance&workspace_id={ws_id}"),
            Some(token),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body:?}");
        assert_eq!(
            body["data"].as_array().expect("data").len(),
            1,
            "a participant finds their own DM: {body:?}"
        );
        assert_eq!(body["data"][0]["channel_id"], conversation_id);
    }

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/search?q=seance&workspace_id={ws_id}"),
        Some(&outsider_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert!(
        body["data"].as_array().expect("data").is_empty(),
        "a DM belongs to the people in it: {body:?}"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn scope_decides_which_half_of_the_search_runs(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "scope-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Scope").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "scoped", false).await;
    let (partner_id, _, _) = seed_and_login(&app, &state, "scope-partner", false).await;
    add_ws_member(&state, ws_id, partner_id, "member").await;

    create_message(&app, &owner_token, ch_id, "shared word here").await;
    let (_, conversation) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&owner_token),
        Some(json!({ "participant_ids": [partner_id] })),
    )
    .await;
    let conversation_id = conversation["id"].as_str().expect("conversation id");
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{conversation_id}/messages"),
        Some(&owner_token),
        Some(json!({ "content": "shared word there" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let cases = [
        ("", 1, 1),
        ("&scope=channels", 1, 0),
        ("&scope=conversations", 0, 1),
        ("&scope=all", 1, 1),
    ];
    for (suffix, channels, conversations) in cases {
        let (status, body) = send(
            &app,
            "GET",
            &format!("/api/search?q=shared&workspace_id={ws_id}{suffix}"),
            Some(&owner_token),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body:?}");
        let hits = body["data"].as_array().expect("data");
        let in_conversation = hits
            .iter()
            .filter(|hit| hit["channel_id"] == conversation_id)
            .count();
        assert_eq!(
            hits.len() - in_conversation,
            channels,
            "channels for scope '{suffix}': {body:?}"
        );
        assert_eq!(
            in_conversation, conversations,
            "conversations for scope '{suffix}': {body:?}"
        );
    }
}

async fn search_contents(
    app: &axum::Router,
    token: &str,
    ws_id: uuid::Uuid,
    query: &str,
) -> Vec<String> {
    let (status, body) = send(
        app,
        "GET",
        &format!("/api/search?workspace_id={ws_id}{query}"),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{query}: {body:?}");
    body["data"]
        .as_array()
        .expect("data")
        .iter()
        .map(|hit| hit["content"].as_str().expect("content").to_string())
        .collect()
}

async fn attach(state: &crate::state::AppState, message_id: uuid::Uuid, mime_type: &str) {
    sqlx::query(
        "UPDATE messages SET metadata = metadata || jsonb_build_object('attachments', jsonb_build_array(jsonb_build_object('id', gen_random_uuid(), 'filename', 'f', 'mime_type', $2::text, 'size_bytes', 1, 'url', '/f'))) WHERE id = $1",
    )
    .bind(message_id)
    .bind(mime_type)
    .execute(&state.pool)
    .await
    .expect("attach");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn search_narrows_by_what_a_message_has(pool: PgPool) {
    let (app, state, _owner, token, ws_id, ch_id) = setup_channel(pool).await;
    create_message(&app, &token, ch_id, "quarterly plain").await;
    create_message(&app, &token, ch_id, "quarterly see https://example.com/q").await;
    let file = create_message(&app, &token, ch_id, "quarterly report attached").await;
    attach(&state, file, "application/pdf").await;
    let photo = create_message(&app, &token, ch_id, "quarterly photo attached").await;
    attach(&state, photo, "image/png").await;

    let mut files = search_contents(&app, &token, ws_id, "&q=quarterly&has=file").await;
    files.sort();
    assert_eq!(
        files,
        vec!["quarterly photo attached", "quarterly report attached"]
    );
    assert_eq!(
        search_contents(&app, &token, ws_id, "&q=quarterly&has=image").await,
        vec!["quarterly photo attached"]
    );
    assert_eq!(
        search_contents(&app, &token, ws_id, "&q=quarterly&has=link").await,
        vec!["quarterly see https://example.com/q"]
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn search_narrows_by_date(pool: PgPool) {
    let (app, state, _owner, token, ws_id, ch_id) = setup_channel(pool).await;
    for (content, at) in [
        ("budget january", "2026-01-10T12:00:00Z"),
        ("budget february", "2026-02-10T12:00:00Z"),
        ("budget march", "2026-03-10T12:00:00Z"),
    ] {
        let id = create_message(&app, &token, ch_id, content).await;
        sqlx::query("UPDATE messages SET created_at = $2::timestamptz WHERE id = $1")
            .bind(id)
            .bind(at)
            .execute(&state.pool)
            .await
            .expect("backdate");
    }

    let after = "&after=2026-02-01T00:00:00Z";
    let before = "&before=2026-03-01T00:00:00Z";
    assert_eq!(
        search_contents(&app, &token, ws_id, &format!("&q=budget{after}")).await,
        vec!["budget march", "budget february"]
    );
    assert_eq!(
        search_contents(&app, &token, ws_id, &format!("&q=budget{before}")).await,
        vec!["budget february", "budget january"]
    );
    assert_eq!(
        search_contents(&app, &token, ws_id, &format!("&q=budget{after}{before}")).await,
        vec!["budget february"]
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn search_runs_on_filters_alone(pool: PgPool) {
    let (app, state, _owner, token, ws_id, ch_id) = setup_channel(pool).await;
    let (colleague, colleague_token) = colleague_in_channel(&app, &state, ws_id, ch_id).await;
    create_message(&app, &token, ch_id, "from the owner").await;
    create_message(&app, &colleague_token, ch_id, "first from the colleague").await;
    create_message(&app, &colleague_token, ch_id, "second from the colleague").await;

    assert_eq!(
        search_contents(&app, &token, ws_id, &format!("&user_id={colleague}")).await,
        vec!["second from the colleague", "first from the colleague"]
    );
    assert_eq!(
        search_contents(&app, &token, ws_id, &format!("&q=%20&user_id={colleague}")).await,
        vec!["second from the colleague", "first from the colleague"]
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn search_in_one_conversation(pool: PgPool) {
    let (app, state, _owner, token, ws_id, ch_id) = setup_channel(pool).await;
    let (partner, _, _) = seed_and_login(&app, &state, "search-partner", false).await;
    add_ws_member(&state, ws_id, partner, "member").await;
    let (outsider, _, outsider_token) =
        seed_and_login(&app, &state, "search-outsider", false).await;
    add_ws_member(&state, ws_id, outsider, "member").await;
    let (_, conversation) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&token),
        Some(json!({ "participant_ids": [partner] })),
    )
    .await;
    let conversation_id = conversation["id"].as_str().expect("conversation id");
    create_message(&app, &token, ch_id, "lunch in the channel").await;
    create_message(
        &app,
        &token,
        conversation_id.parse().expect("uuid"),
        "lunch in the DM",
    )
    .await;

    assert_eq!(
        search_contents(
            &app,
            &token,
            ws_id,
            &format!("&q=lunch&channel_id={conversation_id}")
        )
        .await,
        vec!["lunch in the DM"]
    );
    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/search?workspace_id={ws_id}&q=lunch&channel_id={conversation_id}"),
        Some(&outsider_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn opening_a_channel_says_where_the_reader_stopped(pool: PgPool) {
    let (app, state, _owner_id, token, ws_id, ch_id) = setup_channel(pool).await;
    let (_colleague_id, colleague_token) = colleague_in_channel(&app, &state, ws_id, ch_id).await;
    let open = format!("/api/channels/{ch_id}/messages?mark_read=true");

    let first = create_message(&app, &token, ch_id, "first").await;
    let (status, page) = send(&app, "GET", &open, Some(&colleague_token), None).await;
    assert_eq!(status, StatusCode::OK, "{page:?}");
    assert!(
        page.get("last_read_before").is_none(),
        "nothing was read before, so there is no line to draw: {page:?}"
    );

    create_message(&app, &token, ch_id, "second").await;
    create_message(&app, &token, ch_id, "third").await;
    let (_, page) = send(&app, "GET", &open, Some(&colleague_token), None).await;
    assert_eq!(
        page["last_read_before"].as_str(),
        Some(first.to_string().as_str()),
        "the line goes after the last message read: {page:?}"
    );

    let (_, page) = send(&app, "GET", &open, Some(&colleague_token), None).await;
    assert!(
        page.get("last_read_before").is_none(),
        "once read, opening again draws no line: {page:?}"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_thread_reply_counts_in_the_channel_only_when_it_is_shown_there(pool: PgPool) {
    let (app, state, _owner_id, token, ws_id, ch_id) = setup_channel(pool).await;
    let (colleague_id, colleague_token) = colleague_in_channel(&app, &state, ws_id, ch_id).await;
    let root = create_message(&app, &token, ch_id, "root").await;
    send(
        &app,
        "GET",
        &format!("/api/channels/{ch_id}/messages?mark_read=true"),
        Some(&colleague_token),
        None,
    )
    .await;
    let counters = || async {
        sqlx::query_as::<_, (i32, i32)>(
            "SELECT unread_count, mention_count FROM channel_members WHERE channel_id = $1 AND user_id = $2",
        )
        .bind(ch_id)
        .bind(colleague_id)
        .fetch_one(&state.pool)
        .await
        .expect("counters")
    };
    let reply = |also: bool| {
        json!({
            "content": "<@colleague> in the thread",
            "thread_parent_id": root,
            "also_to_channel": also,
        })
    };

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/messages"),
        Some(&token),
        Some(reply(false)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(
        counters().await,
        (0, 0),
        "a reply kept in its thread leaves the channel read"
    );

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/messages"),
        Some(&token),
        Some(reply(true)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(
        counters().await.0,
        1,
        "a reply also sent to the channel is unread there"
    );
}
