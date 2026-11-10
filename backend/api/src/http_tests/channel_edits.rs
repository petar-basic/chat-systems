use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;
use crate::state::AppState;
use crate::workspace::models::ChannelRole;

async fn conversation(app: &axum::Router, token: &str, ws_id: Uuid, with: &[Uuid]) -> Uuid {
    let (status, body) = send(
        app,
        "POST",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(token),
        Some(json!({ "participant_ids": with })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    body["id"].as_str().expect("id").parse().expect("uuid")
}

async fn command(
    app: &axum::Router,
    token: &str,
    ch: Uuid,
    command: &str,
    text: &str,
) -> (StatusCode, serde_json::Value) {
    send(
        app,
        "POST",
        &format!("/api/channels/{ch}/commands"),
        Some(token),
        Some(json!({ "command": command, "text": text })),
    )
    .await
}

async fn patch(
    app: &axum::Router,
    token: &str,
    ch: Uuid,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    send(
        app,
        "PATCH",
        &format!("/api/channels/{ch}"),
        Some(token),
        Some(body),
    )
    .await
}

async fn join(app: &axum::Router, token: &str, ch: Uuid) {
    let (status, body) = send(
        app,
        "POST",
        &format!("/api/channels/{ch}/join"),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
}

async fn audited(state: &AppState, action: &str, by: Uuid, ch: Uuid) -> i64 {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE action = $1 AND user_id = $2 AND resource_id = $3",
    )
    .bind(action)
    .bind(by)
    .bind(ch)
    .fetch_one(&state.pool)
    .await
    .expect("count audit rows")
}

async fn channel(state: &AppState, ch: Uuid) -> crate::workspace::models::Channel {
    state
        .workspace_service
        .repo
        .find_channel_by_id(ch)
        .await
        .expect("query")
        .expect("channel")
}

async fn roster(state: &AppState, ch: Uuid) -> Vec<(Uuid, String)> {
    sqlx::query_as(
        "SELECT user_id, role::text FROM channel_members WHERE channel_id = $1 ORDER BY user_id",
    )
    .bind(ch)
    .fetch_all(&state.pool)
    .await
    .expect("roster")
}

async fn bot_messages(state: &AppState, ch: Uuid) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE channel_id = $1 AND metadata ? 'bot'")
        .bind(ch)
        .fetch_one(&state.pool)
        .await
        .expect("count")
}

async fn said(state: &AppState, ch: Uuid) -> Vec<String> {
    let mut said: Vec<String> = sqlx::query_scalar(
        "SELECT content FROM messages WHERE channel_id = $1 AND metadata->>'kind' IN ('channel_renamed', 'channel_topic_changed', 'channel_description_changed')",
    )
    .bind(ch)
    .fetch_all(&state.pool)
    .await
    .expect("system messages");
    said.sort();
    said
}

async fn last_update(state: &AppState, ch: Uuid) -> serde_json::Value {
    sqlx::query_scalar(
        "SELECT payload FROM event_outbox WHERE event_type = 'channel.updated' AND payload->>'channel_id' = $1 ORDER BY id DESC LIMIT 1",
    )
    .bind(ch.to_string())
    .fetch_one(&state.pool)
    .await
    .expect("channel.updated")
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_conversation_keeps_the_people_and_settings_it_started_with(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "lock-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Locked WS").await;
    let (admin_id, _, admin) = seed_and_login(&app, &state, "lock-admin", false).await;
    add_ws_member(&state, ws_id, admin_id, "admin").await;
    let (bob_id, _, bob) = seed_and_login(&app, &state, "lock-bob", false).await;
    let (carol_id, _, _) = seed_and_login(&app, &state, "lock-carol", false).await;
    let (dave_id, _, _) = seed_and_login(&app, &state, "lock-dave", false).await;
    let (erin_id, _, _) = seed_and_login(&app, &state, "lock-erin", false).await;
    for id in [bob_id, carol_id, dave_id, erin_id] {
        add_ws_member(&state, ws_id, id, "member").await;
    }

    let direct = conversation(&app, &bob, ws_id, &[carol_id]).await;
    let group = conversation(&app, &bob, ws_id, &[carol_id, dave_id]).await;

    for ch in [direct, group] {
        let before = roster(&state, ch).await;
        let attempts = [
            (
                "POST",
                format!("/api/channels/{ch}/members"),
                Some(json!({ "user_id": erin_id })),
            ),
            (
                "PATCH",
                format!("/api/channels/{ch}/members/{carol_id}/role"),
                Some(json!({ "role": "admin" })),
            ),
            (
                "DELETE",
                format!("/api/channels/{ch}/members/{carol_id}"),
                None,
            ),
            (
                "DELETE",
                format!("/api/channels/{ch}/members/{bob_id}"),
                None,
            ),
            ("DELETE", format!("/api/channels/{ch}"), None),
            ("POST", format!("/api/channels/{ch}/unarchive"), None),
            (
                "PATCH",
                format!("/api/channels/{ch}"),
                Some(json!({ "topic": "ours now" })),
            ),
            (
                "PATCH",
                format!("/api/channels/{ch}"),
                Some(json!({ "name": "renamed" })),
            ),
        ];
        for token in [&bob, &admin, &owner] {
            for (method, path, body) in &attempts {
                let (status, answer) = send(&app, method, path, Some(token), body.clone()).await;
                assert_eq!(status, StatusCode::FORBIDDEN, "{method} {path}: {answer:?}");
            }
        }
        for (method, path) in [
            ("DELETE", format!("/api/channels/{ch}")),
            ("POST", format!("/api/channels/{ch}/unarchive")),
        ] {
            let (_, answer) = send(&app, method, &path, Some(&bob), None).await;
            assert_eq!(
                answer["error"], "A direct message keeps the people and settings it started with",
                "{method} {path}"
            );
        }
        let (status, answer) =
            command(&app, &bob, ch, "invite", &format!("@[Erin]({erin_id})")).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{answer:?}");
        let (status, answer) = command(&app, &bob, ch, "topic", "ours now").await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{answer:?}");

        assert_eq!(
            roster(&state, ch).await,
            before,
            "nobody joined, left or moved up"
        );
        let after = channel(&state, ch).await;
        assert!(!after.is_archived);
        assert!(after.topic.is_none());
        assert!(after.name.is_none());
    }
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_member_who_posts_here_sets_the_topic_and_only_a_moderator_renames(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "about-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "About WS").await;
    let ch = seed_channel(&state, ws_id, owner_id, "deploys", false).await;
    let (bob_id, _, bob) = seed_and_login(&app, &state, "about-bob", false).await;
    add_ws_member(&state, ws_id, bob_id, "member").await;
    join(&app, &bob, ch).await;
    let (outsider_id, _, outsider) = seed_and_login(&app, &state, "about-out", false).await;
    add_ws_member(&state, ws_id, outsider_id, "member").await;
    let (guest_id, _, guest) = seed_and_login(&app, &state, "about-guest", false).await;
    add_ws_member(&state, ws_id, guest_id, "guest").await;
    state
        .workspace_service
        .repo
        .add_channel_member(ch, guest_id, &ChannelRole::Member)
        .await
        .expect("guest joins");

    let (status, body) = patch(
        &app,
        &bob,
        ch,
        json!({ "topic": "release week", "description": "Where we ship" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let now = channel(&state, ch).await;
    assert_eq!(now.topic.as_deref(), Some("release week"));
    assert_eq!(now.description.as_deref(), Some("Where we ship"));
    assert_eq!(now.name.as_deref(), Some("deploys"));
    assert_eq!(audited(&state, "channel.updated", bob_id, ch).await, 1);

    for body in [
        json!({ "name": "mine" }),
        json!({ "name": "mine", "topic": "and this" }),
        json!({ "post_policy": "moderators" }),
        json!({ "allow_broadcast_mentions": false }),
    ] {
        let (status, answer) = patch(&app, &bob, ch, body.clone()).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}: {answer:?}");
    }
    for token in [&guest, &outsider] {
        let (status, answer) = patch(&app, token, ch, json!({ "topic": "not yours" })).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{answer:?}");
    }
    let now = channel(&state, ch).await;
    assert_eq!(now.topic.as_deref(), Some("release week"));
    assert_eq!(now.name.as_deref(), Some("deploys"));

    let (status, body) = patch(&app, &owner, ch, json!({ "post_policy": "moderators" })).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let (status, answer) = patch(&app, &bob, ch, json!({ "topic": "quiet now" })).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "an announcement channel holds its topic to who may post: {answer:?}"
    );
    let (status, body) = patch(&app, &owner, ch, json!({ "topic": "announcements" })).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_topic_command_follows_the_same_rule(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "topic-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Topic WS").await;
    let ch = seed_channel(&state, ws_id, owner_id, "deploys", false).await;
    let (bob_id, _, bob) = seed_and_login(&app, &state, "topic-bob", false).await;
    add_ws_member(&state, ws_id, bob_id, "member").await;
    join(&app, &bob, ch).await;
    let (guest_id, _, guest) = seed_and_login(&app, &state, "topic-guest", false).await;
    add_ws_member(&state, ws_id, guest_id, "guest").await;
    state
        .workspace_service
        .repo
        .add_channel_member(ch, guest_id, &ChannelRole::Member)
        .await
        .expect("guest joins");

    let (status, body) = command(&app, &bob, ch, "topic", "release week").await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["response_type"], "in_channel");
    assert_eq!(
        channel(&state, ch).await.topic.as_deref(),
        Some("release week")
    );
    assert_eq!(audited(&state, "channel.updated", bob_id, ch).await, 1);

    let (status, body) = command(&app, &guest, ch, "topic", "").await;
    assert_eq!(
        status,
        StatusCode::OK,
        "reading the topic is open: {body:?}"
    );
    assert_eq!(body["text"], "The topic is: release week");
    let (status, body) = command(&app, &guest, ch, "topic", "guest topic").await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body:?}");

    let (status, _) = patch(&app, &owner, ch, json!({ "post_policy": "moderators" })).await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = command(&app, &bob, ch, "topic", "quiet now").await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body:?}");

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/channels/{ch}"),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = command(&app, &owner, ch, "topic", "after the end").await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body:?}");
    assert_eq!(body["error"], "This channel is archived", "{body:?}");

    assert_eq!(
        channel(&state, ch).await.topic.as_deref(),
        Some("release week")
    );
    assert_eq!(
        said(&state, ch).await,
        vec!["set the channel topic: release week".to_string()],
        "only bob's change was announced"
    );
    assert_eq!(bot_messages(&state, ch).await, 0);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn invite_is_the_add_member_route_with_its_checks_and_its_audit(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "inv-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Invite WS").await;
    let ch = seed_channel(&state, ws_id, owner_id, "deploys", false).await;
    let (bob_id, _, bob) = seed_and_login(&app, &state, "inv-bob", false).await;
    add_ws_member(&state, ws_id, bob_id, "member").await;
    join(&app, &bob, ch).await;
    let (erin_id, _, _) = seed_and_login(&app, &state, "inv-erin", false).await;
    add_ws_member(&state, ws_id, erin_id, "member").await;
    let (frank_id, _, _) = seed_and_login(&app, &state, "inv-frank", false).await;
    add_ws_member(&state, ws_id, frank_id, "member").await;
    let (guest_id, _, guest) = seed_and_login(&app, &state, "inv-guest", false).await;
    add_ws_member(&state, ws_id, guest_id, "guest").await;
    state
        .workspace_service
        .repo
        .add_channel_member(ch, guest_id, &ChannelRole::Member)
        .await
        .expect("guest joins");

    let mention = |id: Uuid| format!("@[Someone]({id})");

    let (status, body) = command(&app, &guest, ch, "invite", &mention(erin_id)).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a guest adds nobody: {body:?}"
    );

    let (status, body) = command(&app, &bob, ch, "invite", &mention(erin_id)).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(
        body["text"],
        format!("added @[Test User]({erin_id}) to the channel"),
        "the answer names who was added by their own name, not the label typed"
    );
    let added: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE action = 'channel.member_added' \
         AND user_id = $1 AND resource_id = $2 AND details->>'user_id' = $3",
    )
    .bind(bob_id)
    .bind(ch)
    .bind(erin_id.to_string())
    .fetch_one(&state.pool)
    .await
    .expect("count");
    assert_eq!(
        added, 1,
        "the command leaves the same record the route does"
    );

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/channels/{ch}"),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = command(&app, &owner, ch, "invite", &mention(frank_id)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body:?}");
    let present = state
        .workspace_service
        .repo
        .get_channel_member(ch, frank_id)
        .await
        .expect("query");
    assert!(present.is_none(), "nobody is added to an archived channel");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_command_answer_into_a_dm_with_someone_who_left_stays_private(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "gone-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Gone WS").await;
    let (bob_id, _, _) = seed_and_login(&app, &state, "gone-bob", false).await;
    add_ws_member(&state, ws_id, bob_id, "member").await;
    let direct = conversation(&app, &owner, ws_id, &[bob_id]).await;

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/workspaces/{ws_id}/members/{bob_id}"),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = command(&app, &owner, direct, "shrug", "anyone?").await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["response_type"], "ephemeral", "{body:?}");
    let posted: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE channel_id = $1")
        .bind(direct)
        .fetch_one(&state.pool)
        .await
        .expect("count");
    assert_eq!(posted, 0, "nothing lands where nobody would read it");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_channel_hears_what_changed_and_every_open_client_gets_the_channel(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "said-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Said WS").await;
    let ch = seed_channel(&state, ws_id, owner_id, "deploys", false).await;
    let (bob_id, _, bob) = seed_and_login(&app, &state, "said-bob", false).await;
    add_ws_member(&state, ws_id, bob_id, "member").await;
    join(&app, &bob, ch).await;

    let (status, body) = patch(
        &app,
        &bob,
        ch,
        json!({ "topic": "release week", "description": "Where we ship" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let mut expected = vec![
        "set the channel description: Where we ship".to_string(),
        "set the channel topic: release week".to_string(),
    ];
    assert_eq!(said(&state, ch).await, expected);
    let update = last_update(&state, ch).await;
    assert_eq!(update["workspace_id"], json!(ws_id));
    assert_eq!(update["channel"]["topic"], "release week");
    assert_eq!(update["channel"]["description"], "Where we ship");

    let (status, _) = patch(&app, &bob, ch, json!({ "topic": "release week" })).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        said(&state, ch).await,
        expected,
        "the same topic again says nothing"
    );

    let (status, body) = command(&app, &bob, ch, "topic", "launch day").await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["response_type"], "in_channel");
    assert_eq!(body["text"], "");
    assert_eq!(
        bot_messages(&state, ch).await,
        0,
        "the change speaks for itself, not the command"
    );
    expected.push("set the channel topic: launch day".into());
    expected.sort();
    assert_eq!(said(&state, ch).await, expected);

    let (status, _) = patch(&app, &bob, ch, json!({ "topic": "" })).await;
    assert_eq!(status, StatusCode::OK);
    expected.push("cleared the channel topic".into());
    expected.sort();
    assert_eq!(said(&state, ch).await, expected);
    assert_eq!(
        last_update(&state, ch).await["channel"]["topic"]
            .as_str()
            .unwrap_or_default(),
        ""
    );

    let (status, _) = patch(&app, &owner, ch, json!({ "name": "shipping" })).await;
    assert_eq!(status, StatusCode::OK);
    expected.push("renamed the channel from “deploys” to “shipping”".into());
    expected.sort();
    assert_eq!(said(&state, ch).await, expected);
    assert_eq!(last_update(&state, ch).await["channel"]["name"], "shipping");

    let (status, _) = patch(&app, &owner, ch, json!({ "post_policy": "moderators" })).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        said(&state, ch).await,
        expected,
        "who may post is a setting, not news"
    );
    let update = last_update(&state, ch).await;
    assert_eq!(update["channel"]["settings"]["post_policy"], "moderators");
    assert_eq!(update["channel"]["name"], "shipping");
    assert_eq!(audited(&state, "channel.updated", owner_id, ch).await, 1);
    assert_eq!(
        audited(&state, "channel.post_policy_changed", owner_id, ch).await,
        1
    );

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/channels/{ch}"),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        last_update(&state, ch).await["channel"]["is_archived"],
        true
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn adding_people_to_a_conversation_starts_a_group_or_finds_the_one_they_have(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "add-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Add WS").await;
    let (bob_id, _, bob) = seed_and_login(&app, &state, "add-bob", false).await;
    let (carol_id, _, carol) = seed_and_login(&app, &state, "add-carol", false).await;
    let (dave_id, _, dave) = seed_and_login(&app, &state, "add-dave", false).await;
    let (erin_id, _, _) = seed_and_login(&app, &state, "add-erin", false).await;
    for id in [bob_id, carol_id, dave_id, erin_id] {
        add_ws_member(&state, ws_id, id, "member").await;
    }

    let direct = conversation(&app, &bob, ws_id, &[carol_id]).await;
    let group = conversation(&app, &bob, ws_id, &[carol_id, dave_id]).await;
    assert_ne!(group, direct);
    assert_eq!(
        roster(&state, direct).await.len(),
        2,
        "the DM stays as it was"
    );
    assert_eq!(
        conversation(&app, &carol, ws_id, &[dave_id, bob_id]).await,
        group,
        "the same people land in the group they already have"
    );
    let bigger = conversation(&app, &bob, ws_id, &[carol_id, dave_id, erin_id]).await;
    assert_ne!(bigger, group);

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/workspaces/{ws_id}/members/{dave_id}"),
        Some(&dave),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let fresh = conversation(&app, &bob, ws_id, &[carol_id, erin_id]).await;
    assert_ne!(
        fresh, bigger,
        "a group somebody left is not the one these people started"
    );
}
