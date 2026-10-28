use axum::http::StatusCode;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;
use crate::state::AppState;
use crate::workspace::models::ChannelRole;

struct Room {
    app: axum::Router,
    state: std::sync::Arc<AppState>,
    ws_id: Uuid,
    ch_id: Uuid,
    owner: (Uuid, String),
    colleague: (Uuid, String),
}

async fn room(pool: PgPool) -> Room {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "keep-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, &format!("ws-{}", Uuid::new_v4())).await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "keep-up", false).await;
    let (colleague_id, _, colleague_token) =
        seed_and_login(&app, &state, "keep-colleague", false).await;
    add_ws_member(&state, ws_id, colleague_id, "member").await;
    state
        .workspace_service
        .repo
        .add_channel_member(ch_id, colleague_id, &ChannelRole::Member)
        .await
        .expect("add channel member");
    Room {
        app,
        state,
        ws_id,
        ch_id,
        owner: (owner_id, owner_token),
        colleague: (colleague_id, colleague_token),
    }
}

async fn post(room: &Room, token: &str, content: &str, parent: Option<Uuid>, also: bool) -> Uuid {
    let (status, body) = send(
        &room.app,
        "POST",
        &format!("/api/channels/{}/messages", room.ch_id),
        Some(token),
        Some(json!({ "content": content, "thread_parent_id": parent, "also_to_channel": also })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    body["id"].as_str().expect("id").parse().expect("uuid")
}

async fn get(room: &Room, path: &str) -> Vec<Value> {
    let (status, body) = send(&room.app, "GET", path, Some(&room.owner.1), None).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    body["data"].as_array().expect("data").clone()
}

async fn channel(room: &Room) -> Vec<Value> {
    get(room, &format!("/api/channels/{}/messages", room.ch_id)).await
}

async fn thread(room: &Room, root: Uuid) -> Vec<Value> {
    get(room, &format!("/api/messages/{root}/thread")).await
}

fn find(messages: &[Value], id: Uuid) -> Option<&Value> {
    messages.iter().find(|m| m["id"] == id.to_string())
}

async fn delete(room: &Room, token: &str, id: Uuid) {
    let (status, body) = send(
        &room.app,
        "DELETE",
        &format!("/api/messages/{id}"),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
}

async fn age(room: &Room, id: Uuid, days: i32) {
    sqlx::query("UPDATE messages SET created_at = NOW() - make_interval(days => $2) WHERE id = $1")
        .bind(id)
        .bind(days)
        .execute(&room.state.pool)
        .await
        .expect("age message");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_long_thread_reads_back_to_its_first_reply(pool: PgPool) {
    let room = room(pool).await;
    let root = post(&room, &room.owner.1, "long one", None, false).await;
    for n in 0..60 {
        post(
            &room,
            &room.colleague.1,
            &format!("reply {n}"),
            Some(root),
            false,
        )
        .await;
    }

    let newest = thread(&room, root).await;
    assert_eq!(newest.len(), 50);
    assert_eq!(
        newest[0]["content"], "reply 59",
        "the first page is the newest"
    );
    let cursor = newest.last().expect("last")["id"].as_str().expect("id");

    let earlier = get(
        &room,
        &format!("/api/messages/{root}/thread?cursor={cursor}"),
    )
    .await;
    assert_eq!(earlier.len(), 10);
    assert_eq!(earlier.last().expect("last")["content"], "reply 0");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn deleting_a_reply_takes_it_off_the_count(pool: PgPool) {
    let room = room(pool).await;
    let root = post(&room, &room.owner.1, "count me", None, false).await;
    let first = post(&room, &room.colleague.1, "one", Some(root), false).await;
    post(&room, &room.colleague.1, "two", Some(root), false).await;

    delete(&room, &room.colleague.1, first).await;

    let listed = channel(&room).await;
    assert_eq!(find(&listed, root).expect("root")["reply_count"], 1);
    let replies = thread(&room, root).await;
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0]["content"], "two");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_deleted_parent_keeps_its_thread(pool: PgPool) {
    let room = room(pool).await;
    let root = post(&room, &room.owner.1, "secret plan", None, false).await;
    let reply = post(&room, &room.colleague.1, "agreed", Some(root), false).await;
    let lone = post(&room, &room.owner.1, "nobody answered", None, false).await;

    delete(&room, &room.owner.1, root).await;
    delete(&room, &room.owner.1, lone).await;

    let listed = channel(&room).await;
    let tombstone = find(&listed, root).expect("the parent stays as a tombstone");
    assert!(tombstone["deleted_at"].is_string());
    assert_eq!(tombstone["content"], "", "what was deleted is not served");
    assert!(
        find(&listed, reply).is_none(),
        "the reply stays in its thread"
    );
    assert!(
        find(&listed, lone).is_none(),
        "a deleted message with no thread goes"
    );

    assert_eq!(thread(&room, root).await.len(), 1);
    post(&room, &room.colleague.1, "still here", Some(root), false).await;
    assert_eq!(thread(&room, root).await.len(), 2);

    let (status, body) = send(
        &room.app,
        "GET",
        &format!("/api/workspaces/{}/threads", room.ws_id),
        Some(&room.colleague.1),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let followed = body["data"].as_array().expect("data");
    let listed = followed
        .iter()
        .find(|t| t["root"]["id"] == root.to_string())
        .expect("a followed thread outlives its parent");
    assert_eq!(listed["root"]["content"], "");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn retention_removes_a_thread_once_its_last_reply_is_past(pool: PgPool) {
    let room = room(pool).await;
    let alive = post(&room, &room.owner.1, "old question", None, false).await;
    let fresh = post(&room, &room.colleague.1, "new answer", Some(alive), false).await;
    let done = post(&room, &room.owner.1, "older question", None, false).await;
    let stale = post(&room, &room.colleague.1, "old answer", Some(done), false).await;
    for id in [alive, done, stale] {
        age(&room, id, 40).await;
    }

    let retention = &room.state.retention_repo;
    assert_eq!(
        retention
            .count_messages_past(room.ws_id, Some(30), false)
            .await
            .expect("count"),
        2
    );
    retention
        .purge_messages(room.ws_id, Some(30), false)
        .await
        .expect("purge");

    let listed = channel(&room).await;
    assert!(
        find(&listed, alive).is_some(),
        "a thread still talked in stays"
    );
    assert!(find(&listed, done).is_none());
    assert!(
        find(&listed, fresh).is_none(),
        "no reply surfaces in the channel"
    );
    assert_eq!(thread(&room, alive).await.len(), 1);
    let left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE id = ANY($1)")
        .bind(vec![done, stale])
        .fetch_one(&room.state.pool)
        .await
        .expect("count");
    assert_eq!(left, 0, "a thread goes as a unit");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn erasing_a_parent_keeps_the_answers_in_their_thread(pool: PgPool) {
    let room = room(pool).await;
    let root = post(&room, &room.owner.1, "my question", None, false).await;
    let answer = post(&room, &room.colleague.1, "my answer", Some(root), false).await;
    let theirs = post(&room, &room.colleague.1, "their question", None, false).await;
    post(&room, &room.owner.1, "my reply", Some(theirs), false).await;
    let lone = post(&room, &room.owner.1, "just me", None, false).await;

    room.state
        .export_repo
        .erase_writing(room.owner.0)
        .await
        .expect("erase");

    let listed = channel(&room).await;
    let tombstone = find(&listed, root).expect("tombstone");
    assert!(tombstone["deleted_at"].is_string());
    assert_eq!(tombstone["content"], "");
    assert!(find(&listed, answer).is_none());
    assert!(find(&listed, lone).is_none());
    assert_eq!(find(&listed, theirs).expect("theirs")["reply_count"], 0);
    let kept: String = sqlx::query_scalar("SELECT content FROM messages WHERE id = $1")
        .bind(root)
        .fetch_one(&room.state.pool)
        .await
        .expect("tombstone row");
    assert_eq!(kept, "", "the erased words are gone from the row too");
    assert_eq!(thread(&room, root).await.len(), 1);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_reply_also_sent_to_the_channel_shows_in_it(pool: PgPool) {
    let room = room(pool).await;
    let root = post(&room, &room.owner.1, "release?", None, false).await;
    let quiet = post(&room, &room.colleague.1, "just here", Some(root), false).await;
    let loud = post(&room, &room.colleague.1, "shipped", Some(root), true).await;
    let ignored = post(&room, &room.colleague.1, "top level", None, true).await;

    let listed = channel(&room).await;
    let shown = find(&listed, loud).expect("in the channel");
    assert_eq!(shown["thread_parent_id"], root.to_string());
    assert_eq!(shown["metadata"]["also_in_channel"], true);
    assert!(find(&listed, quiet).is_none());
    assert_eq!(
        find(&listed, ignored).expect("top level")["metadata"]["also_in_channel"],
        Value::Null,
        "a top-level message has no thread to come from"
    );
    assert_eq!(find(&listed, root).expect("root")["reply_count"], 2);
    assert_eq!(thread(&room, root).await.len(), 2);
}
