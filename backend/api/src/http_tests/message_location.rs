use axum::http::StatusCode;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;

struct Place {
    app: axum::Router,
    ws_id: Uuid,
    owner: (Uuid, String),
    colleague: (Uuid, String),
    outsider: (Uuid, String),
}

async fn place(pool: PgPool) -> (Place, std::sync::Arc<crate::state::AppState>) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "locate-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, &format!("ws-{}", Uuid::new_v4())).await;
    let mut people = Vec::new();
    for name in ["locate-colleague", "locate-outsider"] {
        let (id, _, token) = seed_and_login(&app, &state, name, false).await;
        add_ws_member(&state, ws_id, id, "member").await;
        people.push((id, token));
    }
    let outsider = people.pop().expect("outsider");
    let colleague = people.pop().expect("colleague");
    (
        Place {
            app,
            ws_id,
            owner: (owner_id, owner_token),
            colleague,
            outsider,
        },
        state,
    )
}

async fn post(place: &Place, token: &str, channel: Uuid, parent: Option<Uuid>) -> Uuid {
    let (status, body) = send(
        &place.app,
        "POST",
        &format!("/api/channels/{channel}/messages"),
        Some(token),
        Some(json!({ "content": "where is this", "thread_parent_id": parent })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    body["id"].as_str().expect("id").parse().expect("uuid")
}

async fn conversation(place: &Place, with: &[Uuid]) -> Uuid {
    let (status, body) = send(
        &place.app,
        "POST",
        &format!("/api/workspaces/{}/conversations", place.ws_id),
        Some(&place.owner.1),
        Some(json!({ "participant_ids": with })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    body["id"].as_str().expect("id").parse().expect("uuid")
}

async fn locate(place: &Place, token: &str, message: Uuid) -> (StatusCode, Value) {
    send(
        &place.app,
        "GET",
        &format!("/api/messages/{message}/location"),
        Some(token),
        None,
    )
    .await
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_channel_message_is_found_where_it_was_posted(pool: PgPool) {
    let (place, state) = place(pool).await;
    let ch_id = seed_channel(&state, place.ws_id, place.owner.0, "located", false).await;
    let message = post(&place, &place.owner.1, ch_id, None).await;

    let (status, body) = locate(&place, &place.owner.1, message).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["workspace_id"], place.ws_id.to_string());
    assert_eq!(body["channel_id"], ch_id.to_string());
    assert_eq!(body["kind"], "channel");
    assert_eq!(body["thread_parent_id"], Value::Null);
    assert_eq!(body["joined"], true);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_reply_names_its_thread(pool: PgPool) {
    let (place, state) = place(pool).await;
    let ch_id = seed_channel(&state, place.ws_id, place.owner.0, "replies", false).await;
    let root = post(&place, &place.owner.1, ch_id, None).await;
    let reply = post(&place, &place.owner.1, ch_id, Some(root)).await;

    let (status, body) = locate(&place, &place.owner.1, reply).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["channel_id"], ch_id.to_string());
    assert_eq!(body["thread_parent_id"], root.to_string());
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_deleted_root_that_still_has_replies_is_found(pool: PgPool) {
    let (place, state) = place(pool).await;
    let ch_id = seed_channel(&state, place.ws_id, place.owner.0, "tombstone", false).await;
    let root = post(&place, &place.owner.1, ch_id, None).await;
    post(&place, &place.owner.1, ch_id, Some(root)).await;
    let (status, _) = send(
        &place.app,
        "DELETE",
        &format!("/api/messages/{root}"),
        Some(&place.owner.1),
        None,
    )
    .await;
    assert!(status.is_success(), "{status}");

    let (status, body) = locate(&place, &place.owner.1, root).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["channel_id"], ch_id.to_string());
    assert_eq!(body["thread_parent_id"], Value::Null);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_public_channel_not_joined_is_found_but_not_joined(pool: PgPool) {
    let (place, state) = place(pool).await;
    let ch_id = seed_channel(&state, place.ws_id, place.owner.0, "open-door", false).await;
    let message = post(&place, &place.owner.1, ch_id, None).await;

    let (status, body) = locate(&place, &place.colleague.1, message).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["kind"], "channel");
    assert_eq!(body["joined"], false);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn direct_and_group_messages_are_found_in_their_conversation(pool: PgPool) {
    let (place, _state) = place(pool).await;
    let dm = conversation(&place, &[place.colleague.0]).await;
    let group = conversation(&place, &[place.colleague.0, place.outsider.0]).await;

    for (conversation, kind) in [(dm, "dm"), (group, "group_dm")] {
        let message = post(&place, &place.owner.1, conversation, None).await;
        let (status, body) = locate(&place, &place.colleague.1, message).await;
        assert_eq!(status, StatusCode::OK, "{body:?}");
        assert_eq!(body["channel_id"], conversation.to_string());
        assert_eq!(body["kind"], kind);
        assert_eq!(body["joined"], true);
    }
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_message_the_caller_cannot_read_is_not_found(pool: PgPool) {
    let (place, state) = place(pool).await;
    let private = seed_channel(&state, place.ws_id, place.owner.0, "back-room", true).await;
    let secret = post(&place, &place.owner.1, private, None).await;
    let dm = conversation(&place, &[place.colleague.0]).await;
    let whisper = post(&place, &place.owner.1, dm, None).await;

    for message in [secret, whisper, Uuid::new_v4()] {
        let (status, body) = locate(&place, &place.outsider.1, message).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{body:?}");
    }
}
