use axum::http::StatusCode;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;
use crate::notifications::models::NotificationType;
use crate::state::AppState;
use crate::workspace::models::ChannelRole;

async fn get(app: &axum::Router, token: &str, path: &str) -> (StatusCode, Value) {
    send(app, "GET", path, Some(token), None).await
}

async fn post(app: &axum::Router, token: &str, path: &str, body: Value) -> Value {
    let (status, created) = send(app, "POST", path, Some(token), Some(body)).await;
    assert_eq!(status, StatusCode::OK, "{path}: {created:?}");
    created
}

async fn join(state: &AppState, channel: Uuid, user: Uuid) {
    state
        .workspace_service
        .repo
        .add_channel_member(channel, user, &ChannelRole::Member)
        .await
        .unwrap();
}

async fn say(app: &axum::Router, token: &str, channel: Uuid, content: &str) -> Uuid {
    let message = post(
        app,
        token,
        &format!("/api/channels/{channel}/messages"),
        json!({ "content": content }),
    )
    .await;
    message["id"].as_str().unwrap().parse().unwrap()
}

async fn context_and_routes(
    app: &axum::Router,
    token: &str,
    ws: Uuid,
    channel: Uuid,
) -> (Value, Value) {
    let (status, context) = get(app, token, &format!("/api/channels/{channel}/context")).await;
    assert_eq!(status, StatusCode::OK, "{context:?}");
    let mut routes = serde_json::Map::new();
    for (key, path) in [
        ("members", format!("/api/channels/{channel}/members")),
        ("pins", format!("/api/channels/{channel}/pins")),
        ("bookmarks", format!("/api/channels/{channel}/bookmarks")),
        (
            "lists",
            format!("/api/workspaces/{ws}/lists?channel_id={channel}"),
        ),
        (
            "workflow_shortcuts",
            format!("/api/channels/{channel}/workflow-shortcuts"),
        ),
    ] {
        let (status, body) = get(app, token, &path).await;
        assert_eq!(status, StatusCode::OK, "{path}: {body:?}");
        routes.insert(key.into(), body["data"].clone());
    }
    (context, Value::Object(routes))
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_context_is_what_each_route_returns(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "ctx-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Context WS").await;
    let (member_id, _, member) = seed_and_login(&app, &state, "ctx-member", false).await;
    add_ws_member(&state, ws, member_id, "member").await;
    let channel = seed_channel(&state, ws, owner_id, "launch", false).await;
    join(&state, channel, member_id).await;

    let pinned = say(&app, &owner, channel, "the plan").await;
    post(
        &app,
        &owner,
        &format!("/api/messages/{pinned}/pin"),
        json!({}),
    )
    .await;
    post(
        &app,
        &owner,
        &format!("/api/channels/{channel}/bookmarks"),
        json!({ "label": "Runbook", "url": "https://example.test/runbook" }),
    )
    .await;
    post(
        &app,
        &owner,
        &format!("/api/workspaces/{ws}/lists"),
        json!({
            "title": "Launch plan",
            "channel_id": channel,
            "columns": [{ "name": "Task", "kind": "text" }],
        }),
    )
    .await;
    post(
        &app,
        &owner,
        &format!("/api/workspaces/{ws}/workflows"),
        json!({
            "name": "Ask",
            "definition": {
                "trigger": { "type": "shortcut", "channel_id": channel },
                "steps": [{ "type": "send_message", "channel_id": channel, "text": "asked" }],
            },
        }),
    )
    .await;

    let (context, routes) = context_and_routes(&app, &member, ws, channel).await;
    for key in [
        "members",
        "pins",
        "bookmarks",
        "lists",
        "workflow_shortcuts",
    ] {
        assert_eq!(context[key], routes[key], "{key}");
        assert_eq!(
            context[key].as_array().map(Vec::len),
            Some(if key == "members" { 2 } else { 1 }),
            "{key}: {context:?}"
        );
    }
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_context_refuses_what_the_routes_refuse(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "ctx-deny-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Context WS").await;
    let (member_id, _, member) = seed_and_login(&app, &state, "ctx-deny-member", false).await;
    add_ws_member(&state, ws, member_id, "member").await;
    let (_, _, outsider) = seed_and_login(&app, &state, "ctx-deny-outsider", false).await;
    let open = seed_channel(&state, ws, owner_id, "open", false).await;
    let secret = seed_channel(&state, ws, owner_id, "secret", true).await;

    for (token, channel, refused) in [
        (&outsider, open, StatusCode::FORBIDDEN),
        (&member, secret, StatusCode::FORBIDDEN),
        (&member, Uuid::new_v4(), StatusCode::NOT_FOUND),
    ] {
        let (status, _) = get(&app, token, &format!("/api/channels/{channel}/context")).await;
        assert_eq!(status, refused, "context of {channel}");
        let (status, _) = get(&app, token, &format!("/api/channels/{channel}/bookmarks")).await;
        assert_eq!(status, refused, "bookmarks of {channel}");
        let (status, _) = get(&app, token, &format!("/api/channels/{channel}/pins")).await;
        assert_eq!(status, refused, "pins of {channel}");
    }

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/channels/{open}/context"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _) = send(
        &app,
        "PATCH",
        &format!("/api/channels/{open}"),
        Some(&owner),
        Some(json!({ "post_policy": "moderators" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    join(&state, open, member_id).await;
    let (status, _) = get(
        &app,
        &member,
        &format!("/api/channels/{open}/workflow-shortcuts"),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, context) = get(&app, &member, &format!("/api/channels/{open}/context")).await;
    assert_eq!(status, StatusCode::OK, "{context:?}");
    assert_eq!(context["workflow_shortcuts"], json!([]));
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_first_page_carries_the_read_mark_when_asked(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "ctx-read-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Context WS").await;
    let (member_id, _, member) = seed_and_login(&app, &state, "ctx-read-member", false).await;
    add_ws_member(&state, ws, member_id, "member").await;
    let channel = seed_channel(&state, ws, owner_id, "launch", false).await;
    let elsewhere = seed_channel(&state, ws, owner_id, "elsewhere", false).await;
    join(&state, channel, member_id).await;

    let oldest = say(&app, &owner, channel, "first").await;
    let newest = say(&app, &owner, channel, "second").await;
    for (in_channel, message) in [(channel, newest), (elsewhere, oldest)] {
        state
            .notification_repo
            .create(
                member_id,
                ws,
                &NotificationType::Mention,
                "You were mentioned",
                None,
                &json!({ "channel_id": in_channel, "message_id": message }),
            )
            .await
            .unwrap();
    }
    let read_mark = || async {
        sqlx::query_scalar::<_, Option<Uuid>>(
            "SELECT last_read_message_id FROM channel_members WHERE channel_id = $1 AND user_id = $2",
        )
        .bind(channel)
        .bind(member_id)
        .fetch_one(&state.pool)
        .await
        .unwrap()
    };

    let messages = format!("/api/channels/{channel}/messages");
    let (status, page) = get(&app, &member, &messages).await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.get("unread_notifications").is_none(), "{page:?}");
    let (_, page) = get(
        &app,
        &member,
        &format!("{messages}?mark_read=true&cursor={newest}"),
    )
    .await;
    assert!(page.get("unread_notifications").is_none(), "{page:?}");
    assert_eq!(read_mark().await, None);

    let (status, page) = get(&app, &member, &format!("{messages}?mark_read=true")).await;
    assert_eq!(status, StatusCode::OK, "{page:?}");
    assert_eq!(page["data"][0]["id"], json!(newest));
    assert_eq!(page["unread_notifications"], json!(1));
    assert_eq!(read_mark().await, Some(newest));
    let (_, count) = get(
        &app,
        &member,
        &format!("/api/workspaces/{ws}/notifications/unread-count"),
    )
    .await;
    assert_eq!(count["unread_count"], json!(1));
}
