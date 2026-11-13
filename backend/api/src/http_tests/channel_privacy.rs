use axum::http::StatusCode;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;
use super::file_search::{found, q, upload_bytes};
use crate::state::AppState;
use crate::workspace::models::ChannelRole;

async fn set_privacy(
    app: &axum::Router,
    token: &str,
    ch: Uuid,
    private: bool,
) -> (StatusCode, Value) {
    send(
        app,
        "PUT",
        &format!("/api/channels/{ch}/privacy"),
        Some(token),
        Some(json!({ "private": private })),
    )
    .await
}

async fn preview(app: &axum::Router, token: Option<&str>, ch: Uuid) -> (StatusCode, Value) {
    send(
        app,
        "GET",
        &format!("/api/channels/{ch}/privacy"),
        token,
        None,
    )
    .await
}

async fn say(app: &axum::Router, token: &str, ch: Uuid, body: Value) -> Uuid {
    let (status, message) = send(
        app,
        "POST",
        &format!("/api/channels/{ch}/messages"),
        Some(token),
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{message:?}");
    message["id"].as_str().expect("id").parse().expect("uuid")
}

async fn join(state: &AppState, ch: Uuid, user: Uuid) {
    state
        .workspace_service
        .repo
        .add_channel_member(ch, user, &ChannelRole::Member)
        .await
        .expect("join");
}

async fn conversation(app: &axum::Router, token: &str, ws: Uuid, with: &[Uuid]) -> Uuid {
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

async fn channel_type(state: &AppState, ch: Uuid) -> String {
    sqlx::query_scalar("SELECT channel_type::text FROM channels WHERE id = $1")
        .bind(ch)
        .fetch_one(&state.pool)
        .await
        .expect("channel type")
}

async fn notices(state: &AppState, ch: Uuid) -> Vec<(String, Value)> {
    sqlx::query_as(
        "SELECT content, metadata FROM messages WHERE channel_id = $1 AND metadata->>'kind' = 'channel_privacy_changed' ORDER BY created_at",
    )
    .bind(ch)
    .fetch_all(&state.pool)
    .await
    .expect("system messages")
}

async fn audited(state: &AppState, ch: Uuid) -> Vec<(Uuid, Value)> {
    sqlx::query_as(
        "SELECT user_id, details FROM audit_log WHERE action = 'channel.privacy_changed' AND resource_id = $1 ORDER BY created_at",
    )
    .bind(ch)
    .fetch_all(&state.pool)
    .await
    .expect("audit rows")
}

async fn reads(app: &axum::Router, token: &str, ch: Uuid, root: Uuid, file: &str) -> Vec<u16> {
    let mut statuses = Vec::new();
    for path in [
        format!("/api/channels/{ch}/messages"),
        format!("/api/messages/{root}/thread"),
        format!("/api/channels/{ch}/pins"),
        format!("/api/channels/{ch}/files"),
        format!("/api/files/{file}"),
        format!("/api/messages/{root}/location"),
    ] {
        let (status, _) = send(app, "GET", &path, Some(token), None).await;
        statuses.push(status.as_u16());
    }
    statuses
}

async fn message_hits(app: &axum::Router, token: &str, ws: Uuid) -> Vec<String> {
    let (status, body) = send(
        app,
        "GET",
        &format!("/api/search?q=shortlist&workspace_id={ws}"),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    body["data"]
        .as_array()
        .expect("data")
        .iter()
        .map(|hit| hit["id"].as_str().expect("id").to_string())
        .collect()
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_channel_made_private_closes_every_read_path_to_whoever_is_not_in_it(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "priv-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Privacy WS").await;
    let (bob_id, _, bob) = seed_and_login(&app, &state, "priv-bob", false).await;
    let (cleo_id, _, cleo) = seed_and_login(&app, &state, "priv-cleo", false).await;
    add_ws_member(&state, ws, bob_id, "member").await;
    add_ws_member(&state, ws, cleo_id, "member").await;
    let ch = seed_channel(&state, ws, owner_id, "hiring", false).await;
    join(&state, ch, bob_id).await;

    let root = say(&app, &owner, ch, json!({ "content": "the shortlist" })).await;
    say(
        &app,
        &bob,
        ch,
        json!({ "content": "two more", "thread_parent_id": root }),
    )
    .await;
    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/messages/{root}/pin"),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let file = upload_bytes(
        &app,
        ws,
        &owner,
        "offer-letter.pdf",
        "application/pdf",
        b"offer",
    )
    .await
    .as_str()
    .expect("file id")
    .to_string();
    say(
        &app,
        &owner,
        ch,
        json!({ "content": "", "attachment_ids": [&file] }),
    )
    .await;

    assert_eq!(
        reads(&app, &cleo, ch, root, &file).await,
        vec![200; 6],
        "a public channel reads for the whole workspace"
    );
    assert_eq!(message_hits(&app, &cleo, ws).await, vec![root.to_string()]);
    assert_eq!(
        found(&app, &cleo, ws, &q("offer")).await,
        vec!["offer-letter.pdf"]
    );

    let (status, body) = set_privacy(&app, &owner, ch, true).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["channel_type"], "private");
    assert_eq!(channel_type(&state, ch).await, "private");

    assert_eq!(
        reads(&app, &cleo, ch, root, &file).await,
        vec![403, 403, 403, 403, 403, 404],
        "messages, thread, pins, files, the file and the link all close"
    );
    assert!(message_hits(&app, &cleo, ws).await.is_empty());
    assert!(found(&app, &cleo, ws, &q("offer")).await.is_empty());

    assert_eq!(
        reads(&app, &bob, ch, root, &file).await,
        vec![200; 6],
        "a member keeps all of it"
    );
    assert_eq!(message_hits(&app, &bob, ws).await, vec![root.to_string()]);

    let said = notices(&state, ch).await;
    assert_eq!(said.len(), 1);
    assert_eq!(said[0].0, "made this channel private");
    assert_eq!(said[0].1["private"], true);
    let audit = audited(&state, ch).await;
    assert_eq!(
        audit,
        vec![(
            owner_id,
            json!({ "name": "hiring", "from": "public", "to": "private" })
        )]
    );
    let frame: Value = sqlx::query_scalar(
        "SELECT payload FROM event_outbox WHERE event_type = 'channel.privacy_changed' AND payload->>'channel_id' = $1",
    )
    .bind(ch.to_string())
    .fetch_one(&state.pool)
    .await
    .expect("the change is announced");
    assert_eq!(
        frame,
        json!({ "channel_id": ch, "workspace_id": ws, "private": true })
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn only_a_workspace_admin_who_can_read_the_channel_changes_its_type(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "priv-matrix-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Matrix WS").await;
    let (ada_id, _, ada) = seed_and_login(&app, &state, "priv-matrix-ada", false).await;
    let (bob_id, _, bob) = seed_and_login(&app, &state, "priv-matrix-bob", false).await;
    let (_, _, outsider) = seed_and_login(&app, &state, "priv-matrix-out", false).await;
    add_ws_member(&state, ws, ada_id, "admin").await;
    add_ws_member(&state, ws, bob_id, "member").await;
    let public = seed_channel(&state, ws, bob_id, "launch", false).await;
    let private = seed_channel(&state, ws, owner_id, "board", true).await;
    join(&state, private, bob_id).await;

    let (status, _) = preview(&app, None, public).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    for (token, ch, want, who) in [
        (
            bob.as_str(),
            public,
            StatusCode::FORBIDDEN,
            "a channel admin who is a member",
        ),
        (
            bob.as_str(),
            private,
            StatusCode::FORBIDDEN,
            "a member inside a private channel",
        ),
        (
            ada.as_str(),
            private,
            StatusCode::FORBIDDEN,
            "an admin outside a private channel",
        ),
        (
            outsider.as_str(),
            public,
            StatusCode::FORBIDDEN,
            "someone from another workspace",
        ),
        (
            ada.as_str(),
            Uuid::new_v4(),
            StatusCode::NOT_FOUND,
            "a channel that is not there",
        ),
    ] {
        let (status, body) = preview(&app, Some(token), ch).await;
        assert_eq!(status, want, "GET by {who}: {body:?}");
        let (status, body) = set_privacy(&app, token, ch, ch != private).await;
        assert_eq!(status, want, "PUT by {who}: {body:?}");
    }
    assert_eq!(channel_type(&state, public).await, "public");
    assert_eq!(channel_type(&state, private).await, "private");

    let (status, body) = preview(&app, Some(&ada), public).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let (status, body) = set_privacy(&app, &ada, public, true).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "an admin who is not a member: {body:?}"
    );
    let (status, body) = set_privacy(&app, &owner, private, false).await;
    assert_eq!(status, StatusCode::OK, "the owner inside it: {body:?}");
    assert_eq!(channel_type(&state, public).await, "private");
    assert_eq!(channel_type(&state, private).await, "public");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_conversation_an_archived_and_a_default_channel_keep_their_type(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "priv-fixed-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Fixed WS").await;
    let (bob_id, _, _) = seed_and_login(&app, &state, "priv-fixed-bob", false).await;
    let (cleo_id, _, _) = seed_and_login(&app, &state, "priv-fixed-cleo", false).await;
    add_ws_member(&state, ws, bob_id, "member").await;
    add_ws_member(&state, ws, cleo_id, "member").await;

    let direct = conversation(&app, &owner, ws, &[bob_id]).await;
    let group = conversation(&app, &owner, ws, &[bob_id, cleo_id]).await;
    let archived = seed_channel(&state, ws, owner_id, "old-launch", false).await;
    sqlx::query("UPDATE channels SET is_archived = true WHERE id = $1")
        .bind(archived)
        .execute(&state.pool)
        .await
        .expect("archive");
    let default = seed_channel(&state, ws, owner_id, "everyone", false).await;
    sqlx::query("UPDATE channels SET is_default = true WHERE id = $1")
        .bind(default)
        .execute(&state.pool)
        .await
        .expect("default");

    for (ch, want, what) in [
        (direct, StatusCode::FORBIDDEN, "a DM"),
        (group, StatusCode::FORBIDDEN, "a group DM"),
        (archived, StatusCode::FORBIDDEN, "an archived channel"),
        (default, StatusCode::BAD_REQUEST, "a default channel"),
    ] {
        let before = channel_type(&state, ch).await;
        let (status, body) = preview(&app, Some(&owner), ch).await;
        assert_eq!(status, want, "GET {what}: {body:?}");
        let (status, body) = set_privacy(&app, &owner, ch, before != "private").await;
        assert_eq!(status, want, "PUT {what}: {body:?}");
        assert_eq!(channel_type(&state, ch).await, before, "{what}");
        assert!(notices(&state, ch).await.is_empty(), "{what}");
    }
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_channel_made_public_opens_its_history_to_members_and_not_to_guests(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "priv-open-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Open WS").await;
    let (cleo_id, _, cleo) = seed_and_login(&app, &state, "priv-open-cleo", false).await;
    let (gus_id, _, gus) = seed_and_login(&app, &state, "priv-open-gus", false).await;
    add_ws_member(&state, ws, cleo_id, "member").await;
    add_ws_member(&state, ws, gus_id, "guest").await;
    let ch = seed_channel(&state, ws, owner_id, "roadmap", true).await;

    let root = say(&app, &owner, ch, json!({ "content": "the shortlist" })).await;
    say(&app, &owner, ch, json!({ "content": "second" })).await;
    let gone = say(&app, &owner, ch, json!({ "content": "deleted later" })).await;
    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/messages/{gone}"),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let file = upload_bytes(&app, ws, &owner, "plan.pdf", "application/pdf", b"plan").await;
    say(
        &app,
        &owner,
        ch,
        json!({ "content": "", "attachment_ids": [file] }),
    )
    .await;

    let (status, body) = preview(&app, Some(&owner), ch).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(
        body,
        json!({ "private": true, "messages": 3, "files": 1 }),
        "what opening it shows, deleted messages left out"
    );

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/channels/{ch}/messages"),
        Some(&cleo),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, body) = set_privacy(&app, &owner, ch, false).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["channel_type"], "public");

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/channels/{ch}/messages"),
        Some(&cleo),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(message_hits(&app, &cleo, ws).await, vec![root.to_string()]);
    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/channels/browse"),
        Some(&cleo),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert!(body["data"]
        .as_array()
        .expect("data")
        .iter()
        .any(|c| c["id"] == json!(ch) && c["is_member"] == false));

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/channels/{ch}/messages"),
        Some(&gus),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a guest sees only their channels"
    );
    assert!(message_hits(&app, &gus, ws).await.is_empty());

    let said = notices(&state, ch).await;
    assert_eq!(said.len(), 1);
    assert_eq!(said[0].0, "made this channel public");
    assert_eq!(
        audited(&state, ch).await,
        vec![(
            owner_id,
            json!({ "name": "roadmap", "from": "private", "to": "public" })
        )]
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn asking_for_the_type_a_channel_already_has_changes_nothing(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "priv-twice-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Twice WS").await;
    let ch = seed_channel(&state, ws, owner_id, "ops", false).await;

    for _ in 0..2 {
        let (status, body) = set_privacy(&app, &owner, ch, true).await;
        assert_eq!(status, StatusCode::OK, "{body:?}");
        assert_eq!(body["channel_type"], "private");
    }
    assert_eq!(notices(&state, ch).await.len(), 1);
    assert_eq!(audited(&state, ch).await.len(), 1);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_bot_and_a_hook_outside_a_channel_stop_posting_when_it_goes_private(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "priv-int-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Integrations WS").await;
    let (ada_id, _, ada) = seed_and_login(&app, &state, "priv-int-ada", false).await;
    add_ws_member(&state, ws, ada_id, "admin").await;
    let ch = seed_channel(&state, ws, owner_id, "deploys", false).await;

    let (status, bot) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/bot-apps"),
        Some(&owner),
        Some(json!({
            "name": "Shipper",
            "description": "Says what shipped",
            "scopes": ["chat:write", "chat:write.public"]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{bot:?}");
    let bot_token = bot["token"].as_str().expect("token").to_string();

    let (status, hook) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/hooks"),
        Some(&ada),
        Some(json!({
            "hook_type": "incoming_webhook",
            "name": "ci",
            "config": { "channel_id": ch }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{hook:?}");
    let hook_token = hook["config"]["token"].as_str().expect("token").to_string();

    let bot_uri = format!("/api/channels/{ch}/messages");
    let hook_uri = format!("/api/hooks/incoming/{hook_token}");
    let bot_post = || {
        send(
            &app,
            "POST",
            &bot_uri,
            Some(&bot_token),
            Some(json!({ "content": "v1 shipped" })),
        )
    };
    let hook_post = || {
        send(
            &app,
            "POST",
            &hook_uri,
            None,
            Some(json!({ "text": "build passed" })),
        )
    };

    let (status, body) = bot_post().await;
    assert_eq!(status, StatusCode::OK, "chat:write.public: {body:?}");
    let (status, body) = hook_post().await;
    assert_eq!(status, StatusCode::OK, "{body:?}");

    let (status, body) = set_privacy(&app, &owner, ch, true).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");

    let (status, _) = bot_post().await;
    assert_eq!(status, StatusCode::FORBIDDEN, "public means public");
    let (status, _) = hook_post().await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "its owner is not in the channel"
    );
    let active: bool = sqlx::query_scalar("SELECT is_active FROM hooks WHERE id = $1")
        .bind(Uuid::parse_str(hook["id"].as_str().expect("id")).expect("uuid"))
        .fetch_one(&state.pool)
        .await
        .expect("hook");
    assert!(!active, "the hook stops");
}
