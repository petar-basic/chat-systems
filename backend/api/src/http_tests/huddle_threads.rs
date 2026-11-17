use axum::http::StatusCode;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;
use crate::huddle::consumer;
use crate::state::AppState;
use crate::workspace::models::ChannelRole;

struct Room {
    ws: Uuid,
    channel: Uuid,
    owner_id: Uuid,
    owner: String,
    mia_id: Uuid,
    mia: String,
    olu_id: Uuid,
    olu: String,
    cy_id: Uuid,
}

async fn room(app: &axum::Router, state: &AppState) -> Room {
    let (owner_id, _, owner) = seed_and_login(app, state, "ht-owner", false).await;
    let ws = seed_workspace(state, owner_id, "Huddle Threads").await;
    let channel = seed_channel(state, ws, owner_id, "war-room", true).await;
    let (mia_id, _, mia) = seed_and_login(app, state, "ht-mia", false).await;
    let (olu_id, _, olu) = seed_and_login(app, state, "ht-olu", false).await;
    let (cy_id, _, _) = seed_and_login(app, state, "ht-cy", false).await;
    for user in [mia_id, olu_id, cy_id] {
        add_ws_member(state, ws, user, "member").await;
    }
    for user in [mia_id, cy_id] {
        state
            .workspace_service
            .repo
            .add_channel_member(channel, user, &ChannelRole::Member)
            .await
            .expect("join");
    }
    Room {
        ws,
        channel,
        owner_id,
        owner,
        mia_id,
        mia,
        olu_id,
        olu,
        cy_id,
    }
}

async fn start(app: &axum::Router, token: &str, ws: Uuid, body: Value) -> (Uuid, Uuid) {
    let (status, started) = send(
        app,
        "POST",
        &format!("/api/workspaces/{ws}/huddles"),
        Some(token),
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{started:?}");
    let id = |key: &str| started[key].as_str().expect(key).parse().expect("uuid");
    (id("huddle_id"), id("message_id"))
}

async fn joins(state: &AppState, huddle: Uuid, user: Uuid) {
    consumer::handle(
        state,
        "huddle.member_joined",
        &json!({ "huddle_id": huddle, "user_id": user }),
    )
    .await
    .expect("join");
}

async fn leaves(state: &AppState, huddle: Uuid, user: Uuid) {
    consumer::handle(
        state,
        "huddle.member_left",
        &json!({ "huddle_id": huddle, "user_id": user }),
    )
    .await
    .expect("leave");
}

async fn post(
    app: &axum::Router,
    token: &str,
    channel: Uuid,
    thread: Option<Uuid>,
    content: &str,
) -> (StatusCode, Value) {
    send(
        app,
        "POST",
        &format!("/api/channels/{channel}/messages"),
        Some(token),
        Some(json!({ "content": content, "thread_parent_id": thread })),
    )
    .await
}

async fn get(app: &axum::Router, token: &str, path: &str) -> StatusCode {
    send(app, "GET", path, Some(token), None).await.0
}

async fn following(state: &AppState, user: Uuid, root: Uuid) -> Option<bool> {
    sqlx::query_scalar(
        "SELECT following FROM thread_follows WHERE user_id = $1 AND root_message_id = $2",
    )
    .bind(user)
    .bind(root)
    .fetch_optional(&state.pool)
    .await
    .expect("follow row")
}

#[test_macros::db_test(migrations = "../migrations")]
async fn someone_invited_from_outside_has_the_thread_only_while_in_the_huddle(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let r = room(&app, &state).await;
    let (status, other) = post(&app, &r.owner, r.channel, None, "the plan").await;
    assert_eq!(status, StatusCode::OK);
    let other = other["id"].as_str().expect("id").to_string();
    let (huddle, root) = start(&app, &r.owner, r.ws, json!({ "channel_id": r.channel })).await;
    state
        .huddle_repo
        .invite(huddle, r.olu_id, r.owner_id)
        .await
        .expect("invite");
    joins(&state, huddle, r.owner_id).await;

    let thread = format!("/api/messages/{root}/thread");
    let huddle_thread = format!("/api/huddles/{huddle}/thread");
    assert_eq!(get(&app, &r.olu, &thread).await, StatusCode::FORBIDDEN);
    assert_eq!(
        post(&app, &r.olu, r.channel, Some(root), "hi").await.0,
        StatusCode::FORBIDDEN
    );

    joins(&state, huddle, r.olu_id).await;
    assert_eq!(get(&app, &r.olu, &thread).await, StatusCode::OK);
    let (status, mine) = send(&app, "GET", &huddle_thread, Some(&r.olu), None).await;
    assert_eq!(status, StatusCode::OK, "{mine:?}");
    assert_eq!(mine["in_channel"], false);
    assert_eq!(mine["message"]["id"], root.to_string());
    let (_, theirs) = send(&app, "GET", &huddle_thread, Some(&r.owner), None).await;
    assert_eq!(theirs["in_channel"], true);

    let (status, reply) = post(
        &app,
        &r.olu,
        r.channel,
        Some(root),
        "https://example.com/doc",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reply:?}");
    let reply = reply["id"].as_str().expect("id").to_string();
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/messages/{reply}/reactions"),
        Some(&r.olu),
        Some(json!({ "emoji": "👍" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        send(
            &app,
            "POST",
            &format!("/api/threads/{root}/read"),
            Some(&r.olu),
            None
        )
        .await
        .0,
        StatusCode::OK
    );

    assert_eq!(
        post(&app, &r.olu, r.channel, None, "hello channel").await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post(
            &app,
            &r.olu,
            r.channel,
            Some(other.parse().expect("uuid")),
            "hi"
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        get(
            &app,
            &r.olu,
            &format!("/api/channels/{}/messages", r.channel)
        )
        .await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        get(&app, &r.olu, &format!("/api/messages/{other}/thread")).await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(
            &app,
            "POST",
            &format!("/api/messages/{other}/reactions"),
            Some(&r.olu),
            Some(json!({ "emoji": "👍" })),
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );

    let (status, channel) = send(
        &app,
        "GET",
        &format!("/api/channels/{}/messages", r.channel),
        Some(&r.mia),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let shown = channel["data"]
        .as_array()
        .expect("data")
        .iter()
        .find(|m| m["id"] == root.to_string())
        .expect("the huddle's message");
    assert_eq!(shown["reply_count"], 1, "members see the reply under it");

    leaves(&state, huddle, r.olu_id).await;
    assert_eq!(get(&app, &r.olu, &thread).await, StatusCode::FORBIDDEN);
    assert_eq!(
        get(&app, &r.olu, &huddle_thread).await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post(&app, &r.olu, r.channel, Some(root), "still here?")
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(
            &app,
            "DELETE",
            &format!("/api/messages/{reply}/reactions/%F0%9F%91%8D"),
            Some(&r.olu),
            None
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_ended_huddle_says_how_long_it_lasted_and_who_was_in_it(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let r = room(&app, &state).await;
    let (huddle, root) = start(&app, &r.owner, r.ws, json!({ "channel_id": r.channel })).await;
    joins(&state, huddle, r.owner_id).await;
    joins(&state, huddle, r.mia_id).await;
    leaves(&state, huddle, r.mia_id).await;
    joins(&state, huddle, r.mia_id).await;
    leaves(&state, huddle, r.mia_id).await;

    let metadata: Value = sqlx::query_scalar("SELECT metadata FROM messages WHERE id = $1")
        .bind(root)
        .fetch_one(&state.pool)
        .await
        .expect("root");
    assert!(
        metadata.get("ended_at").is_none(),
        "still going: {metadata}"
    );

    leaves(&state, huddle, r.owner_id).await;
    let (started_at, ended_at): (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>) =
        sqlx::query_as("SELECT started_at, ended_at FROM huddle_sessions WHERE id = $1")
            .bind(huddle)
            .fetch_one(&state.pool)
            .await
            .expect("session");
    assert!(ended_at >= started_at);
    let metadata: Value = sqlx::query_scalar("SELECT metadata FROM messages WHERE id = $1")
        .bind(root)
        .fetch_one(&state.pool)
        .await
        .expect("root");
    assert_eq!(metadata["kind"], "huddle_started");
    assert_eq!(metadata["huddle_id"], huddle.to_string());
    assert_eq!(
        metadata["participant_ids"],
        json!([r.owner_id, r.mia_id]),
        "each person once, in the order they came"
    );
    let ended: chrono::DateTime<chrono::Utc> = metadata["ended_at"]
        .as_str()
        .expect("ended_at")
        .parse()
        .expect("timestamp");
    assert_eq!(ended.timestamp_micros(), ended_at.timestamp_micros());

    let updated: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM event_outbox
          WHERE event_type = 'message.updated' AND payload->>'id' = $1",
    )
    .bind(root.to_string())
    .fetch_one(&state.pool)
    .await
    .expect("outbox");
    assert_eq!(updated, 1, "clients hear about the summary");

    leaves(&state, huddle, r.owner_id).await;
    let updated_again: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM event_outbox
          WHERE event_type = 'message.updated' AND payload->>'id' = $1",
    )
    .bind(root.to_string())
    .fetch_one(&state.pool)
    .await
    .expect("outbox");
    assert_eq!(updated_again, 1, "a replayed leave ends nothing twice");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_direct_huddle_starts_its_thread_in_the_conversation(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let r = room(&app, &state).await;
    let (huddle, root) = start(&app, &r.owner, r.ws, json!({ "dm_partner_id": r.mia_id })).await;

    let channel: Uuid = sqlx::query_scalar("SELECT channel_id FROM messages WHERE id = $1")
        .bind(root)
        .fetch_one(&state.pool)
        .await
        .expect("root");
    let kind: String = sqlx::query_scalar("SELECT channel_type::text FROM channels WHERE id = $1")
        .bind(channel)
        .fetch_one(&state.pool)
        .await
        .expect("channel");
    assert_eq!(kind, "dm");
    let (status, page) = send(
        &app,
        "GET",
        &format!("/api/channels/{channel}/messages"),
        Some(&r.mia),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(page["data"]
        .as_array()
        .expect("data")
        .iter()
        .any(|m| m["id"] == root.to_string() && m["metadata"]["huddle_id"] == huddle.to_string()));

    let (_, again) = start(&app, &r.mia, r.ws, json!({ "dm_partner_id": r.owner_id })).await;
    let same: Uuid = sqlx::query_scalar("SELECT channel_id FROM messages WHERE id = $1")
        .bind(again)
        .fetch_one(&state.pool)
        .await
        .expect("root");
    assert_eq!(same, channel, "the conversation the two already have");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_huddle_follows_its_thread_while_there_and_writers_keep_it(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let r = room(&app, &state).await;
    let (huddle, root) = start(&app, &r.owner, r.ws, json!({ "channel_id": r.channel })).await;
    state
        .huddle_repo
        .invite(huddle, r.olu_id, r.owner_id)
        .await
        .expect("invite");
    for user in [r.owner_id, r.mia_id, r.olu_id] {
        joins(&state, huddle, user).await;
        assert_eq!(following(&state, user, root).await, Some(true));
    }
    assert_eq!(following(&state, r.cy_id, root).await, None);

    let (status, _) = post(&app, &r.olu, r.channel, Some(root), "notes in the doc").await;
    assert_eq!(status, StatusCode::OK);
    let mut heard = state
        .thread_repo
        .reply_recipients(root, Some(r.olu_id))
        .await
        .expect("recipients");
    heard.sort();
    let mut expected = vec![r.owner_id, r.mia_id];
    expected.sort();
    assert_eq!(heard, expected, "nobody outside the huddle is told");

    leaves(&state, huddle, r.mia_id).await;
    assert_eq!(following(&state, r.mia_id, root).await, Some(false));
    let heard = state
        .thread_repo
        .reply_recipients(root, Some(r.owner_id))
        .await
        .expect("recipients");
    assert!(heard.contains(&r.olu_id), "olu is still in the huddle");
    assert!(!heard.contains(&r.mia_id));

    leaves(&state, huddle, r.olu_id).await;
    assert_eq!(
        following(&state, r.olu_id, root).await,
        Some(true),
        "olu wrote in it"
    );
    let heard = state
        .thread_repo
        .reply_recipients(root, Some(r.owner_id))
        .await
        .expect("recipients");
    assert!(
        !heard.contains(&r.olu_id),
        "but cannot read the channel once out of the huddle"
    );

    leaves(&state, huddle, r.owner_id).await;
    assert_eq!(following(&state, r.owner_id, root).await, Some(false));
    let (status, _) = post(&app, &r.mia, r.channel, Some(root), "after the call").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        following(&state, r.owner_id, root).await,
        Some(false),
        "a later reply does not bring the starter back"
    );

    joins_again_after_writing(&app, &state, &r).await;
}

async fn joins_again_after_writing(app: &axum::Router, state: &AppState, r: &Room) {
    let (huddle, root) = start(app, &r.owner, r.ws, json!({ "channel_id": r.channel })).await;
    joins(state, huddle, r.owner_id).await;
    joins(state, huddle, r.mia_id).await;
    let (status, _) = post(app, &r.mia, r.channel, Some(root), "me again").await;
    assert_eq!(status, StatusCode::OK);
    leaves(state, huddle, r.mia_id).await;
    joins(state, huddle, r.mia_id).await;
    leaves(state, huddle, r.mia_id).await;
    assert_eq!(following(state, r.mia_id, root).await, Some(true));
}
