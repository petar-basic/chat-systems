use axum::http::StatusCode;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;
use crate::notifications::consumer::notify_message;
use crate::push::sender::{PushSender, VapidKeys};
use crate::state::AppState;
use crate::workspace::models::ChannelRole;

struct Room {
    app: axum::Router,
    state: std::sync::Arc<AppState>,
    ws_id: Uuid,
    ch_id: Uuid,
    owner: (Uuid, String),
    colleague: (Uuid, String),
    third: (Uuid, String),
}

async fn room(pool: PgPool) -> Room {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "thread-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, &format!("ws-{}", Uuid::new_v4())).await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "threads", false).await;
    let mut members = Vec::new();
    for name in ["thread-colleague", "thread-third"] {
        let (id, _, token) = seed_and_login(&app, &state, name, false).await;
        add_ws_member(&state, ws_id, id, "member").await;
        state
            .workspace_service
            .repo
            .add_channel_member(ch_id, id, &ChannelRole::Member)
            .await
            .expect("add channel member");
        members.push((id, token));
    }
    let third = members.pop().expect("third");
    let colleague = members.pop().expect("colleague");
    Room {
        app,
        state,
        ws_id,
        ch_id,
        owner: (owner_id, owner_token),
        colleague,
        third,
    }
}

async fn post(room: &Room, token: &str, content: &str, parent: Option<Uuid>) -> Value {
    let (status, body) = send(
        &room.app,
        "POST",
        &format!("/api/channels/{}/messages", room.ch_id),
        Some(token),
        Some(json!({ "content": content, "thread_parent_id": parent })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    body
}

fn id_of(message: &Value) -> Uuid {
    message["id"].as_str().expect("id").parse().expect("uuid")
}

async fn threads(room: &Room, token: &str) -> Vec<Value> {
    let (status, body) = send(
        &room.app,
        "GET",
        &format!("/api/workspaces/{}/threads", room.ws_id),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    body["data"].as_array().expect("data").clone()
}

async fn unread_threads(room: &Room, token: &str) -> i64 {
    let (status, body) = send(
        &room.app,
        "GET",
        &format!("/api/workspaces/{}/threads/unread", room.ws_id),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    body["count"].as_i64().expect("count")
}

async fn thread_call(room: &Room, method: &str, token: &str, path: &str) -> (StatusCode, Value) {
    send(
        &room.app,
        method,
        &format!("/api/threads/{path}"),
        Some(token),
        None,
    )
    .await
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_reply_puts_the_thread_in_front_of_its_author(pool: PgPool) {
    let room = room(pool).await;
    let root = id_of(&post(&room, &room.owner.1, "plan for friday", None).await);
    post(&room, &room.colleague.1, "count me in", Some(root)).await;

    let mine = threads(&room, &room.owner.1).await;
    assert_eq!(mine.len(), 1);
    assert_eq!(mine[0]["root"]["id"], root.to_string());
    assert_eq!(mine[0]["unread_count"], 1);
    assert_eq!(mine[0]["latest_replies"][0]["content"], "count me in");
    assert_eq!(unread_threads(&room, &room.owner.1).await, 1);

    let theirs = threads(&room, &room.colleague.1).await;
    assert_eq!(theirs.len(), 1, "the replier follows too");
    assert_eq!(theirs[0]["unread_count"], 0, "and has read their own reply");
    assert!(threads(&room, &room.third.1).await.is_empty());

    let (status, read) = thread_call(&room, "POST", &room.owner.1, &format!("{root}/read")).await;
    assert_eq!(status, StatusCode::OK, "{read:?}");
    assert_eq!(read["following"], true);
    assert_eq!(read["unread_count"], 0);
    assert_eq!(unread_threads(&room, &room.owner.1).await, 0);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn replying_reads_the_thread_and_a_reply_to_a_reply_joins_its_root(pool: PgPool) {
    let room = room(pool).await;
    let root = id_of(&post(&room, &room.owner.1, "lunch?", None).await);
    let first = id_of(&post(&room, &room.colleague.1, "yes", Some(root)).await);
    post(&room, &room.colleague.1, "noon?", Some(root)).await;
    post(&room, &room.owner.1, "noon works", Some(first)).await;

    let mine = threads(&room, &room.owner.1).await;
    assert_eq!(mine[0]["root"]["id"], root.to_string());
    assert_eq!(mine[0]["unread_count"], 0);
    assert_eq!(
        threads(&room, &room.colleague.1).await[0]["unread_count"],
        1
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_latest_three_replies_come_oldest_first(pool: PgPool) {
    let room = room(pool).await;
    let root = id_of(&post(&room, &room.owner.1, "count", None).await);
    for n in 1..=5 {
        post(&room, &room.colleague.1, &format!("reply {n}"), Some(root)).await;
    }
    let mine = threads(&room, &room.owner.1).await;
    let latest: Vec<&str> = mine[0]["latest_replies"]
        .as_array()
        .expect("replies")
        .iter()
        .map(|r| r["content"].as_str().expect("content"))
        .collect();
    assert_eq!(latest, vec!["reply 3", "reply 4", "reply 5"]);
    assert_eq!(mine[0]["unread_count"], 5);
    assert_eq!(mine[0]["root"]["reply_count"], 5);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_mention_in_a_reply_follows_the_thread(pool: PgPool) {
    let room = room(pool).await;
    let root = id_of(&post(&room, &room.owner.1, "release", None).await);
    post(&room, &room.colleague.1, "shipped", Some(root)).await;
    post(
        &room,
        &room.colleague.1,
        &format!("@[Third]({}) can you check?", room.third.0),
        Some(root),
    )
    .await;

    let theirs = threads(&room, &room.third.1).await;
    assert_eq!(theirs.len(), 1);
    assert_eq!(
        theirs[0]["unread_count"], 1,
        "replies before the mention count as read"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn unfollowing_stops_the_thread_until_followed_again(pool: PgPool) {
    let room = room(pool).await;
    let root = id_of(&post(&room, &room.owner.1, "status", None).await);
    post(&room, &room.colleague.1, "green", Some(root)).await;

    let (status, body) =
        thread_call(&room, "DELETE", &room.owner.1, &format!("{root}/follow")).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["following"], false);
    post(&room, &room.colleague.1, "still green", Some(root)).await;
    assert!(threads(&room, &room.owner.1).await.is_empty());
    assert_eq!(unread_threads(&room, &room.owner.1).await, 0);
    assert!(!room
        .state
        .thread_repo
        .reply_recipients(root, Some(room.colleague.0))
        .await
        .expect("recipients")
        .contains(&room.owner.0));

    let (status, body) = thread_call(&room, "PUT", &room.owner.1, &format!("{root}/follow")).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["following"], true);
    assert_eq!(body["unread_count"], 2);

    let (_, body) = thread_call(&room, "PUT", &room.third.1, &format!("{root}/follow")).await;
    assert_eq!(body["following"], true);
    assert_eq!(
        body["unread_count"], 0,
        "a new follower starts at the present"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn threads_are_for_the_people_who_can_see_them(pool: PgPool) {
    let room = room(pool).await;
    let root = id_of(&post(&room, &room.owner.1, "internal", None).await);
    let (_, _, outsider_token) =
        seed_and_login(&room.app, &room.state, "thread-outsider", false).await;

    for (method, path) in [
        ("POST", format!("{root}/read")),
        ("PUT", format!("{root}/follow")),
        ("DELETE", format!("{root}/follow")),
    ] {
        let (status, _) = thread_call(&room, method, &outsider_token, &path).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {path}");
        let (status, _) = send(
            &room.app,
            method,
            &format!("/api/threads/{path}"),
            None,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {path}");
    }
    for path in ["threads", "threads/unread"] {
        let (status, _) = send(
            &room.app,
            "GET",
            &format!("/api/workspaces/{}/{path}", room.ws_id),
            Some(&outsider_token),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{path}");
    }
    let (status, _) = thread_call(
        &room,
        "POST",
        &room.owner.1,
        &format!("{}/read", Uuid::new_v4()),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_thread_in_a_channel_you_left_drops_out(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner, _, owner_token) = seed_and_login(&app, &state, "private-owner", false).await;
    let ws_id = seed_workspace(&state, owner, "Private threads").await;
    let ch_id = seed_channel(&state, ws_id, owner, "secret", true).await;
    let (member, _, member_token) = seed_and_login(&app, &state, "private-member", false).await;
    add_ws_member(&state, ws_id, member, "member").await;
    state
        .workspace_service
        .repo
        .add_channel_member(ch_id, member, &ChannelRole::Member)
        .await
        .expect("add channel member");
    let room = Room {
        app,
        state,
        ws_id,
        ch_id,
        owner: (owner, owner_token),
        colleague: (member, member_token),
        third: (Uuid::nil(), String::new()),
    };
    let root = id_of(&post(&room, &room.colleague.1, "between us", None).await);
    post(&room, &room.owner.1, "agreed", Some(root)).await;
    assert_eq!(threads(&room, &room.colleague.1).await.len(), 1);

    sqlx::query("DELETE FROM channel_members WHERE channel_id = $1 AND user_id = $2")
        .bind(ch_id)
        .bind(member)
        .execute(&room.state.pool)
        .await
        .expect("leave");
    assert!(threads(&room, &room.colleague.1).await.is_empty());
    assert_eq!(unread_threads(&room, &room.colleague.1).await, 0);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn followers_hear_about_a_reply_and_the_mentioned_hear_it_once(pool: PgPool) {
    let room = room(pool).await;
    let root = id_of(&post(&room, &room.owner.1, "design review", None).await);
    let reply = post(
        &room,
        &room.colleague.1,
        &format!("@[Third]({}) thoughts?", room.third.0),
        Some(root),
    )
    .await;

    let sender = PushSender::new(
        room.state.push_repo.clone(),
        VapidKeys {
            public_key: "test-public".into(),
            private_key: "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8".into(),
            subject: "mailto:ops@test.local".into(),
        },
    );
    let mut payload = reply.clone();
    payload["workspace_id"] = json!(room.ws_id);
    payload["mentioned_user_ids"] = json!([room.third.0]);
    let mut conn = room.state.redis.clone();
    notify_message(
        &room.state,
        &room.state.notification_repo,
        &sender,
        &mut conn,
        &payload,
    )
    .await;

    let kinds = |user: Uuid| {
        let pool = room.state.pool.clone();
        async move {
            sqlx::query_as::<_, (String, Option<String>)>(
                "SELECT notification_type::text, data->>'message_id' FROM notifications WHERE user_id = $1",
            )
            .bind(user)
            .fetch_all(&pool)
            .await
            .expect("notifications")
        }
    };
    assert_eq!(
        kinds(room.owner.0).await,
        vec![("reply".to_string(), Some(root.to_string()))],
        "the root's author hears about the reply, pointed at the thread"
    );
    assert_eq!(
        kinds(room.third.0).await,
        vec![("mention".to_string(), Some(id_of(&reply).to_string()))],
        "the mentioned person hears about it as a mention, not twice"
    );
    assert!(
        kinds(room.colleague.0).await.is_empty(),
        "nobody hears about their own reply"
    );
}
