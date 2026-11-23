use axum::http::StatusCode;
use axum::Router;
use chrono::{DateTime, Duration, Utc};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;
use crate::state::AppState;
use crate::workspace::models::ChannelRole;

struct Team {
    ws: Uuid,
    owner_id: Uuid,
    owner: String,
    ana_id: Uuid,
    ana: String,
    ben: String,
    general: Uuid,
    secret: Uuid,
    root: Uuid,
}

async fn team(app: &Router, state: &AppState) -> Team {
    let (owner_id, _, owner) = seed_and_login(app, state, "dr-owner", false).await;
    let ws = seed_workspace(state, owner_id, "Drafts").await;
    let (ana_id, _, ana) = seed_and_login(app, state, "dr-ana", false).await;
    add_ws_member(state, ws, ana_id, "member").await;
    let (ben_id, _, ben) = seed_and_login(app, state, "dr-ben", false).await;
    add_ws_member(state, ws, ben_id, "member").await;
    let general = seed_channel(state, ws, owner_id, "dr-general", false).await;
    let secret = seed_channel(state, ws, owner_id, "dr-secret", true).await;
    for (channel, user) in [(general, ana_id), (general, ben_id), (secret, ana_id)] {
        state
            .workspace_service
            .repo
            .add_channel_member(channel, user, &ChannelRole::Member)
            .await
            .expect("join");
    }
    let root = post(app, &owner, general, "a thread to answer").await;
    Team {
        ws,
        owner_id,
        owner,
        ana_id,
        ana,
        ben,
        general,
        secret,
        root,
    }
}

async fn post(app: &Router, token: &str, channel: Uuid, content: &str) -> Uuid {
    post_body(app, token, channel, json!({ "content": content })).await
}

async fn post_body(app: &Router, token: &str, channel: Uuid, body: Value) -> Uuid {
    let (status, body) = send(
        app,
        "POST",
        &format!("/api/v1/channels/{channel}/messages"),
        Some(token),
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    body["id"].as_str().expect("id").parse().expect("uuid")
}

async fn db_now(state: &AppState) -> DateTime<Utc> {
    sqlx::query_scalar("SELECT NOW()")
        .fetch_one(&state.pool)
        .await
        .expect("now")
}

fn place(channel: Uuid, thread: Option<Uuid>, content: &str, at: DateTime<Utc>) -> Value {
    json!({
        "channel_id": channel,
        "thread_parent_id": thread,
        "content": content,
        "updated_at": at,
    })
}

async fn save(app: &Router, token: &str, ws: Uuid, body: Value) -> (StatusCode, Value) {
    send(
        app,
        "PUT",
        &format!("/api/v1/workspaces/{ws}/drafts"),
        Some(token),
        Some(body),
    )
    .await
}

async fn saved(app: &Router, token: &str, ws: Uuid, body: Value) -> Value {
    let (status, draft) = save(app, token, ws, body).await;
    assert_eq!(status, StatusCode::OK, "{draft:?}");
    draft
}

async fn drafts(app: &Router, token: &str, ws: Uuid) -> Vec<String> {
    let (status, body) = send(
        app,
        "GET",
        &format!("/api/v1/workspaces/{ws}/drafts"),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    body["data"]
        .as_array()
        .expect("data")
        .iter()
        .map(|d| d["content"].as_str().expect("content").to_string())
        .collect()
}

async fn sent(app: &Router, token: &str, ws: Uuid, query: &str) -> Vec<Value> {
    let (status, body) = send(
        app,
        "GET",
        &format!("/api/v1/workspaces/{ws}/sent-messages{query}"),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    body["data"].as_array().expect("data").clone()
}

fn contents(messages: &[Value]) -> Vec<&str> {
    messages
        .iter()
        .map(|m| m["content"].as_str().expect("content"))
        .collect()
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_draft_is_kept_per_place_and_cleared_by_saving_it_empty(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let t = team(&app, &state).await;
    let now = db_now(&state).await;

    let channel = saved(
        &app,
        &t.ana,
        t.ws,
        place(t.general, None, "hello every", now - Duration::seconds(20)),
    )
    .await;
    assert_eq!(channel["content"], "hello every");
    assert_eq!(channel["thread_parent_id"], Value::Null);
    saved(
        &app,
        &t.ana,
        t.ws,
        place(
            t.general,
            Some(t.root),
            "about that",
            now - Duration::seconds(10),
        ),
    )
    .await;
    assert_eq!(
        drafts(&app, &t.ana, t.ws).await,
        ["about that", "hello every"],
        "newest first, the thread apart from its channel"
    );

    let rewritten = saved(
        &app,
        &t.ana,
        t.ws,
        place(
            t.general,
            None,
            "hello everyone",
            now - Duration::seconds(5),
        ),
    )
    .await;
    assert_eq!(rewritten["id"], channel["id"], "one row per place");

    let cleared = saved(&app, &t.ana, t.ws, place(t.general, None, "   ", now)).await;
    assert_eq!(cleared["content"], "");
    assert_eq!(drafts(&app, &t.ana, t.ws).await, ["about that"]);
    assert!(drafts(&app, &t.ben, t.ws).await.is_empty());
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_newer_write_wins_and_a_clock_from_the_future_does_not(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let t = team(&app, &state).await;
    let now = db_now(&state).await;

    saved(
        &app,
        &t.ana,
        t.ws,
        place(
            t.general,
            None,
            "from the desktop",
            now - Duration::minutes(1),
        ),
    )
    .await;
    let offline = saved(
        &app,
        &t.ana,
        t.ws,
        place(t.general, None, "typed offline", now - Duration::minutes(5)),
    )
    .await;
    assert_eq!(
        offline["content"], "from the desktop",
        "an older write answers with what the server kept"
    );
    let stale_clear = saved(
        &app,
        &t.ana,
        t.ws,
        place(t.general, None, "", now - Duration::minutes(3)),
    )
    .await;
    assert_eq!(stale_clear["content"], "from the desktop");

    let ahead = saved(
        &app,
        &t.ana,
        t.ws,
        place(t.general, None, "a fast clock", now + Duration::hours(1)),
    )
    .await;
    let kept_at: DateTime<Utc> = ahead["updated_at"]
        .as_str()
        .expect("updated_at")
        .parse()
        .expect("time");
    assert!(
        kept_at < now + Duration::minutes(1),
        "a time ahead of the server is cut to now"
    );
    let next = saved(
        &app,
        &t.ana,
        t.ws,
        place(t.general, None, "later still", now + Duration::hours(2)),
    )
    .await;
    assert_eq!(
        next["content"], "later still",
        "the cut time does not lock the place for an hour"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn sending_clears_the_draft_of_that_place_and_no_other(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let t = team(&app, &state).await;
    let typed_at = db_now(&state).await - Duration::seconds(30);

    saved(
        &app,
        &t.ana,
        t.ws,
        place(t.general, None, "ship it", typed_at),
    )
    .await;
    saved(
        &app,
        &t.ana,
        t.ws,
        place(t.general, Some(t.root), "agreed", typed_at),
    )
    .await;

    post(&app, &t.ben, t.general, "someone else talks").await;
    assert_eq!(drafts(&app, &t.ana, t.ws).await.len(), 2);

    post(&app, &t.ana, t.general, "ship it").await;
    assert_eq!(drafts(&app, &t.ana, t.ws).await, ["agreed"]);
    let late = saved(
        &app,
        &t.ana,
        t.ws,
        place(t.general, None, "ship it", typed_at),
    )
    .await;
    assert_eq!(
        late["content"], "",
        "a save from before the send does not bring the draft back"
    );

    post_body(
        &app,
        &t.ana,
        t.general,
        json!({ "content": "agreed", "thread_parent_id": t.root }),
    )
    .await;
    assert!(drafts(&app, &t.ana, t.ws).await.is_empty());
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_place_you_cannot_reach_answers_not_found(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let t = team(&app, &state).await;
    let now = db_now(&state).await;
    let (_, _, stranger) = seed_and_login(&app, &state, "dr-stranger", false).await;
    let other_ws = seed_workspace(&state, t.owner_id, "Elsewhere").await;
    let elsewhere = seed_channel(&state, other_ws, t.owner_id, "dr-elsewhere", false).await;
    let unjoined = seed_channel(&state, t.ws, t.owner_id, "dr-unjoined", false).await;
    let reply = post_body(
        &app,
        &t.owner,
        t.general,
        json!({ "content": "a reply", "thread_parent_id": t.root }),
    )
    .await;
    let secret_root = post(&app, &t.owner, t.secret, "in the secret room").await;

    for (token, channel, thread) in [
        (&t.ben, t.secret, None),
        (&t.ben, unjoined, None),
        (&t.ana, elsewhere, None),
        (&stranger, t.general, None),
        (&t.ana, t.general, Some(reply)),
        (&t.ana, t.general, Some(secret_root)),
        (&t.ana, t.general, Some(Uuid::new_v4())),
        (&t.ana, Uuid::new_v4(), None),
    ] {
        let (status, body) = save(&app, token, t.ws, place(channel, thread, "psst", now)).await;
        assert_eq!(
            status,
            StatusCode::NOT_FOUND,
            "{channel} {thread:?}: {body:?}"
        );
    }

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/v1/workspaces/{}/drafts", t.ws),
        Some(&stranger),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = save(
        &app,
        &t.ana,
        t.ws,
        place(t.general, None, &"x".repeat(4001), now),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    saved(
        &app,
        &t.ana,
        t.ws,
        place(t.secret, None, "for the room", now),
    )
    .await;
    sqlx::query("DELETE FROM channel_members WHERE channel_id = $1 AND user_id = $2")
        .bind(t.secret)
        .bind(t.ana_id)
        .execute(&state.pool)
        .await
        .expect("leave");
    assert!(
        drafts(&app, &t.ana, t.ws).await.is_empty(),
        "a channel you left takes its draft out of the list"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn sent_lists_your_messages_newest_first_a_page_at_a_time(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let t = team(&app, &state).await;

    post(&app, &t.ana, t.general, "one").await;
    post_body(
        &app,
        &t.ana,
        t.general,
        json!({ "content": "two", "thread_parent_id": t.root }),
    )
    .await;
    let (status, dm) = send(
        &app,
        "POST",
        &format!("/api/v1/workspaces/{}/conversations", t.ws),
        Some(&t.ana),
        Some(json!({ "participant_ids": [t.owner_id] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{dm:?}");
    let dm: Uuid = dm["id"].as_str().expect("dm").parse().expect("uuid");
    post(&app, &t.ana, dm, "three").await;
    post(&app, &t.ana, t.secret, "in a room I leave").await;
    let deleted = post(&app, &t.ana, t.general, "taken back").await;
    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/v1/messages/{deleted}"),
        Some(&t.ana),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    post(&app, &t.ben, t.general, "not mine").await;
    sqlx::query(
        "INSERT INTO messages (channel_id, user_id, content, metadata) \
         VALUES ($1, $2, 'archived this channel', '{\"kind\": \"channel_archived\"}')",
    )
    .bind(t.general)
    .bind(t.ana_id)
    .execute(&state.pool)
    .await
    .expect("system message");
    let other_ws = seed_workspace(&state, t.owner_id, "Elsewhere").await;
    add_ws_member(&state, other_ws, t.ana_id, "member").await;
    let elsewhere = seed_channel(&state, other_ws, t.owner_id, "dr-elsewhere", false).await;
    state
        .workspace_service
        .repo
        .add_channel_member(elsewhere, t.ana_id, &ChannelRole::Member)
        .await
        .expect("join");
    post(&app, &t.ana, elsewhere, "another workspace").await;
    sqlx::query("DELETE FROM channel_members WHERE channel_id = $1 AND user_id = $2")
        .bind(t.secret)
        .bind(t.ana_id)
        .execute(&state.pool)
        .await
        .expect("leave");

    let first = sent(&app, &t.ana, t.ws, "?limit=2").await;
    assert_eq!(contents(&first), ["three", "two"]);
    assert_eq!(first[1]["thread_parent_id"], t.root.to_string());
    assert_eq!(first[0]["channel_id"], dm.to_string());
    let cursor = first[1]["id"].as_str().expect("id");
    let rest = sent(&app, &t.ana, t.ws, &format!("?limit=2&cursor={cursor}")).await;
    assert_eq!(contents(&rest), ["one"]);
    assert_eq!(contents(&sent(&app, &t.ben, t.ws, "").await), ["not mine"]);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_person_export_carries_drafts_and_erasing_or_leaving_removes_them(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let t = team(&app, &state).await;
    let now = db_now(&state).await;
    saved(&app, &t.ana, t.ws, place(t.general, None, "kept", now)).await;
    saved(
        &app,
        &t.ana,
        t.ws,
        place(t.general, Some(t.root), "also kept", now),
    )
    .await;
    saved(
        &app,
        &t.ana,
        t.ws,
        place(t.secret, None, "gone", now - Duration::seconds(5)),
    )
    .await;
    saved(&app, &t.ana, t.ws, place(t.secret, None, "", now)).await;

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/v1/users/{}/exports", t.ana_id),
        Some(&t.ana),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let job = state
        .export_repo
        .claim_next()
        .await
        .expect("claim")
        .expect("a pending job");
    let job_id = job.id;
    crate::export::job::run_for_test(&state, job)
        .await
        .expect("export runs");
    let manifest = state
        .export_repo
        .find(job_id)
        .await
        .expect("find")
        .expect("job")
        .manifest
        .expect("manifest");
    assert_eq!(manifest["files"]["drafts.jsonl"]["rows"], 2);
    let exported = state.draft_repo.owned_by(t.ana_id).await.expect("owned");
    assert_eq!(exported[0]["channel_name"], "dr-general");

    let (_, _, instance_admin) = seed_and_login(&app, &state, "dr-instance", true).await;
    let (status, body) = send(
        &app,
        "DELETE",
        &format!("/api/v1/admin/users/{}/data", t.ana_id),
        Some(&instance_admin),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(
        body["account"]["drafts"], 2,
        "a cleared place is not a draft"
    );

    let (ben_id, _, ben) = seed_and_login(&app, &state, "dr-leaver", false).await;
    add_ws_member(&state, t.ws, ben_id, "member").await;
    state
        .workspace_service
        .repo
        .add_channel_member(t.general, ben_id, &ChannelRole::Member)
        .await
        .expect("join");
    saved(&app, &ben, t.ws, place(t.general, None, "unfinished", now)).await;
    let (status, body) = send(
        &app,
        "DELETE",
        &format!("/api/v1/workspaces/{}/members/{ben_id}", t.ws),
        Some(&t.owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let kept: i64 = sqlx::query_scalar("SELECT count(*) FROM drafts WHERE user_id = ANY($1)")
        .bind(vec![t.ana_id, ben_id])
        .fetch_one(&state.pool)
        .await
        .expect("count");
    assert_eq!(kept, 0);
}
