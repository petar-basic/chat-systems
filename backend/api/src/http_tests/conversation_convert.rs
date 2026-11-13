use axum::http::StatusCode;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;
use crate::state::AppState;
use crate::workspace::models::WorkspaceRole;

async fn open(app: &axum::Router, token: &str, ws: Uuid, with: &[Uuid]) -> Uuid {
    let (status, body) = send(
        app,
        "POST",
        &format!("/api/workspaces/{ws}/conversations"),
        Some(token),
        Some(json!({ "participant_ids": with })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    body["id"].as_str().expect("id").parse().expect("uuid")
}

async fn convert(app: &axum::Router, token: &str, id: Uuid, name: &str) -> (StatusCode, Value) {
    send(
        app,
        "POST",
        &format!("/api/conversations/{id}/convert"),
        Some(token),
        Some(json!({ "name": name })),
    )
    .await
}

async fn say(app: &axum::Router, token: &str, ch: Uuid, body: Value) -> Value {
    let (status, message) = send(
        app,
        "POST",
        &format!("/api/channels/{ch}/messages"),
        Some(token),
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{message:?}");
    message
}

async fn get(app: &axum::Router, token: &str, path: &str) -> Value {
    let (status, body) = send(app, "GET", path, Some(token), None).await;
    assert_eq!(status, StatusCode::OK, "{path}: {body:?}");
    body
}

async fn listed(app: &axum::Router, token: &str, ws: Uuid, id: Uuid) -> (bool, Option<Value>) {
    let conversations = get(app, token, &format!("/api/workspaces/{ws}/conversations")).await;
    let in_conversations = conversations["data"]
        .as_array()
        .expect("conversations")
        .iter()
        .any(|c| c["id"] == json!(id));
    let channels = get(app, token, &format!("/api/workspaces/{ws}/channels")).await;
    let channel = channels["data"]
        .as_array()
        .expect("channels")
        .iter()
        .find(|c| c["id"] == json!(id))
        .cloned();
    (in_conversations, channel)
}

async fn read_state(app: &axum::Router, token: &str, ws: Uuid, id: Uuid) -> (Value, Value) {
    let unread = get(app, token, &format!("/api/workspaces/{ws}/channels/unread")).await;
    let row = unread["counts"]
        .as_array()
        .expect("counts")
        .iter()
        .find(|c| c["channel_id"] == json!(id))
        .cloned()
        .unwrap_or(Value::Null);
    (row["unread_count"].clone(), row["last_read_msg"].clone())
}

async fn kind(state: &AppState, id: Uuid) -> String {
    sqlx::query_scalar("SELECT channel_type::text FROM channels WHERE id = $1")
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .expect("channel type")
}

async fn notices(state: &AppState, id: Uuid) -> Vec<Value> {
    sqlx::query_scalar(
        "SELECT metadata FROM messages WHERE channel_id = $1 AND metadata->>'kind' = 'converted_to_channel'",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await
    .expect("system messages")
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_group_dm_becomes_a_private_channel_that_keeps_its_history_and_people(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "conv-to-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Convert WS").await;
    let (ana_id, _, ana) = seed_and_login(&app, &state, "conv-to-ana", false).await;
    let (cy_id, _, cy) = seed_and_login(&app, &state, "conv-to-cy", false).await;
    let (dee_id, _, dee) = seed_and_login(&app, &state, "conv-to-dee", false).await;
    for id in [ana_id, cy_id, dee_id] {
        add_ws_member(&state, ws, id, "member").await;
    }

    let group = open(&app, &ana, ws, &[owner_id, cy_id]).await;
    let kickoff = say(&app, &ana, group, json!({ "content": "kickoff" })).await;
    say(
        &app,
        &owner,
        group,
        json!({ "content": "agenda", "thread_parent_id": kickoff["id"] }),
    )
    .await;
    let (status, _) = send(
        &app,
        "PUT",
        &format!("/api/channels/{group}/notify-level"),
        Some(&cy),
        Some(json!({ "level": "mentions" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{group}/read"),
        Some(&cy),
        Some(json!({ "message_id": kickoff["id"] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    say(&app, &ana, group, json!({ "content": "dates" })).await;
    let cy_before = read_state(&app, &cy, ws, group).await;
    let owner_before = read_state(&app, &owner, ws, group).await;

    let (status, channel) = convert(&app, &ana, group, "launch").await;
    assert_eq!(status, StatusCode::OK, "{channel:?}");
    assert_eq!(channel["id"], json!(group), "it keeps its id");
    assert_eq!(channel["name"], "launch");
    assert_eq!(channel["channel_type"], "private");

    for (token, level) in [(&ana, "all"), (&owner, "all"), (&cy, "mentions")] {
        let (in_conversations, listing) = listed(&app, token, ws, group).await;
        assert!(!in_conversations, "the old row is gone");
        let listing = listing.expect("it is a channel now");
        assert_eq!(listing["name"], "launch");
        assert_eq!(
            listing["notify_level"], level,
            "a group DM notified on every message, and a choice of one's own stays"
        );
    }
    assert_eq!(read_state(&app, &cy, ws, group).await, cy_before);
    assert_eq!(read_state(&app, &owner, ws, group).await, owner_before);

    let roles: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT user_id, role::text FROM channel_members WHERE channel_id = $1 AND role = 'admin'",
    )
    .bind(group)
    .fetch_all(&state.pool)
    .await
    .expect("roles");
    assert_eq!(roles, vec![(ana_id, "admin".into())]);

    let history = get(&app, &owner, &format!("/api/channels/{group}/messages")).await;
    let said = history["data"].to_string();
    assert!(said.contains("kickoff") && said.contains("dates"), "{said}");
    let thread = get(
        &app,
        &cy,
        &format!(
            "/api/messages/{}/thread",
            kickoff["id"].as_str().expect("id")
        ),
    )
    .await;
    assert!(thread["data"].to_string().contains("agenda"));

    assert_eq!(
        notices(&state, group).await,
        vec![json!({ "kind": "converted_to_channel", "name": "launch" })]
    );
    let audit: Vec<(Uuid, Value)> = sqlx::query_as(
        "SELECT user_id, details FROM audit_log WHERE action = 'conversation.converted' AND resource_id = $1",
    )
    .bind(group)
    .fetch_all(&state.pool)
    .await
    .expect("audit");
    assert_eq!(
        audit,
        vec![(ana_id, json!({ "name": "launch", "participants": 3 }))]
    );
    let frame: Value = sqlx::query_scalar(
        "SELECT payload FROM event_outbox WHERE event_type = 'conversation.converted' AND payload->>'conversation_id' = $1",
    )
    .bind(group.to_string())
    .fetch_one(&state.pool)
    .await
    .expect("the participants hear of it");
    let mut told: Vec<Uuid> =
        serde_json::from_value(frame["participant_ids"].clone()).expect("ids");
    told.sort();
    let mut everyone = vec![owner_id, ana_id, cy_id];
    everyone.sort();
    assert_eq!(told, everyone);
    assert_eq!(frame["channel"]["name"], "launch");

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/channels/{group}/messages"),
        Some(&dee),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "it is private");
    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/channels/{group}/members"),
        Some(&ana),
        Some(json!({ "user_id": dee_id })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a channel takes new people: {body:?}"
    );
    let history = get(&app, &dee, &format!("/api/channels/{group}/messages")).await;
    assert!(
        history["data"].to_string().contains("kickoff"),
        "and they read what was said before"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn only_someone_in_a_group_of_three_who_may_make_private_channels_converts_it(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "conv-rule-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Rules WS").await;
    let (ana_id, _, ana) = seed_and_login(&app, &state, "conv-rule-ana", false).await;
    let (cy_id, _, cy) = seed_and_login(&app, &state, "conv-rule-cy", false).await;
    let (dee_id, _, dee) = seed_and_login(&app, &state, "conv-rule-dee", false).await;
    let (_, _, outsider) = seed_and_login(&app, &state, "conv-rule-out", false).await;
    for id in [ana_id, cy_id, dee_id] {
        add_ws_member(&state, ws, id, "member").await;
    }
    let direct = open(&app, &ana, ws, &[cy_id]).await;
    let group = open(&app, &ana, ws, &[owner_id, cy_id]).await;
    let public = seed_channel(&state, ws, owner_id, "general-talk", false).await;
    seed_channel(&state, ws, owner_id, "launch", true).await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/conversations/{group}/convert"),
        None,
        Some(json!({ "name": "plans" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    for (token, id, name, want, what) in [
        (&ana, direct, "plans", StatusCode::BAD_REQUEST, "a 1:1 DM"),
        (&ana, public, "plans", StatusCode::BAD_REQUEST, "a channel"),
        (
            &dee,
            group,
            "plans",
            StatusCode::FORBIDDEN,
            "someone outside the group",
        ),
        (
            &outsider,
            group,
            "plans",
            StatusCode::FORBIDDEN,
            "someone from another workspace",
        ),
        (
            &ana,
            Uuid::new_v4(),
            "plans",
            StatusCode::NOT_FOUND,
            "a conversation that is not there",
        ),
        (
            &ana,
            group,
            "  ",
            StatusCode::UNPROCESSABLE_ENTITY,
            "no name",
        ),
        (
            &ana,
            group,
            "LAUNCH",
            StatusCode::CONFLICT,
            "a name that is taken",
        ),
    ] {
        let (status, body) = convert(&app, token, id, name).await;
        assert_eq!(status, want, "{what}: {body:?}");
    }
    assert_eq!(kind(&state, direct).await, "dm");
    assert_eq!(kind(&state, group).await, "group_dm", "nothing changed");
    assert!(notices(&state, group).await.is_empty());

    let (status, body) = send(
        &app,
        "PATCH",
        &format!("/api/workspaces/{ws}"),
        Some(&owner),
        Some(json!({ "private_channel_creators": "admins" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let (status, body) = convert(&app, &cy, group, "plans").await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body:?}");
    assert_eq!(kind(&state, group).await, "group_dm");

    let (status, body) = convert(&app, &owner, group, "plans").await;
    assert_eq!(status, StatusCode::OK, "an admin in the group: {body:?}");
    let (status, _) = convert(&app, &owner, group, "plans-again").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "it is a channel now");
    assert_eq!(notices(&state, group).await.len(), 1);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn somebody_who_had_left_the_group_does_not_come_back_into_the_channel(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "conv-gone-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Departures WS").await;
    let (ana_id, _, _) = seed_and_login(&app, &state, "conv-gone-ana", false).await;
    let (cy_id, _, _) = seed_and_login(&app, &state, "conv-gone-cy", false).await;
    let (dee_id, _, dee) = seed_and_login(&app, &state, "conv-gone-dee", false).await;
    for id in [ana_id, cy_id, dee_id] {
        add_ws_member(&state, ws, id, "member").await;
    }
    let group = open(&app, &owner, ws, &[ana_id, cy_id, dee_id]).await;
    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/workspaces/{ws}/members/{dee_id}"),
        Some(&dee),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = convert(&app, &owner, group, "crew").await;
    assert_eq!(status, StatusCode::OK, "{body:?}");

    let staged = state
        .workspace_service
        .join_workspace(ws, dee_id, &WorkspaceRole::Member)
        .await
        .expect("come back");
    state.publisher.dispatch_all(staged).await;
    let member: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM channel_members WHERE channel_id = $1 AND user_id = $2)",
    )
    .bind(group)
    .bind(dee_id)
    .fetch_one(&state.pool)
    .await
    .expect("membership");
    assert!(!member, "a private channel is joined by invitation");
    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/channels/{group}/messages"),
        Some(&dee),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
