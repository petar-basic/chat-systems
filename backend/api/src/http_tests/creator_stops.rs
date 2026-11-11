use axum::http::StatusCode;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;

async fn hook_state(pool: &PgPool, id: &str) -> (bool, Option<String>) {
    sqlx::query_as("SELECT is_active, disabled_reason FROM hooks WHERE id = $1::uuid")
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("hook state")
}

async fn hook_credential(pool: &PgPool, id: &str, key: &str) -> String {
    sqlx::query_scalar("SELECT config->>$2 FROM hooks WHERE id = $1::uuid")
        .bind(id)
        .bind(key)
        .fetch_one(pool)
        .await
        .expect("hook credential")
}

async fn audited(pool: &PgPool, action: &str, resource: Uuid) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM audit_log WHERE action = $1 AND resource_id = $2")
        .bind(action)
        .bind(resource)
        .fetch_one(pool)
        .await
        .expect("count audit")
}

async fn create(app: &axum::Router, token: &str, ws: Uuid, body: Value) -> Value {
    let (status, made) = send(
        app,
        "POST",
        &format!("/api/workspaces/{ws}/hooks"),
        Some(token),
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{made:?}");
    made
}

async fn turn_on(app: &axum::Router, token: &str, id: &str) -> (StatusCode, Value) {
    send(
        app,
        "PATCH",
        &format!("/api/hooks/{id}"),
        Some(token),
        Some(json!({ "is_active": true })),
    )
    .await
}

async fn invoke(app: &axum::Router, token: &str, ch: Uuid, command: &str) -> (StatusCode, Value) {
    send(
        app,
        "POST",
        &format!("/api/channels/{ch}/commands"),
        Some(token),
        Some(json!({ "command": command, "text": "now" })),
    )
    .await
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_demoted_creators_command_answers_unknown_command(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "cmd-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Commands").await;
    let ch = seed_channel(&state, ws, owner_id, "deploys", false).await;
    let (admin_id, _, admin) = seed_and_login(&app, &state, "cmd-admin", false).await;
    add_ws_member(&state, ws, admin_id, "admin").await;

    let hook = create(
        &app,
        &admin,
        ws,
        json!({
            "hook_type": "slash_command",
            "name": "deploy",
            "config": {
                "command": "deploy",
                "url": "http://127.0.0.1:9/deploy",
                "channel_ids": [ch]
            }
        }),
    )
    .await;
    let id = hook["id"].as_str().expect("id");
    add_ws_member(&state, ws, admin_id, "member").await;

    let (status, body) = invoke(&app, &owner, ch, "deploy").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body:?}");
    assert_eq!(body["error"], "unknown_command");
    assert_eq!(
        hook_state(&state.pool, id).await,
        (
            false,
            Some("Its creator is no longer allowed to make it".to_string())
        )
    );

    let (status, directory) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/app-directory"),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{directory:?}");
    let listed = directory["installed"]
        .as_array()
        .expect("installed")
        .iter()
        .find(|a| a["id"] == id)
        .expect("the command is listed")
        .clone();
    assert_eq!(listed["health"]["state"], "disabled");
    assert_eq!(
        listed["health"]["reason"],
        "Its creator is no longer allowed to make it"
    );

    let (status, revived) = turn_on(&app, &owner, id).await;
    assert_eq!(status, StatusCode::OK, "{revived:?}");
    assert_eq!(revived["hook"]["is_active"], true);
    assert!(
        revived["secrets"].is_null(),
        "a slash command carries no credential to mint"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_incoming_webhook_turned_back_on_answers_only_at_its_new_url(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "hook-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Webhooks").await;
    let ch = seed_channel(&state, ws, owner_id, "deploys", false).await;

    let hook = create(
        &app,
        &owner,
        ws,
        json!({ "hook_type": "incoming_webhook", "name": "CI", "config": { "channel_id": ch } }),
    )
    .await;
    let id = hook["id"].as_str().expect("id");
    let old = hook_credential(&state.pool, id, "token").await;
    state
        .hook_repo
        .disable(id.parse().expect("uuid"), "Its creator left the workspace")
        .await
        .expect("disable");

    let (status, revived) = turn_on(&app, &owner, id).await;
    assert_eq!(status, StatusCode::OK, "{revived:?}");
    assert_eq!(revived["hook"]["config"]["token"], "***");
    let new = hook_credential(&state.pool, id, "token").await;
    assert_ne!(new, old);
    assert!(revived["secrets"]["incoming_url"]
        .as_str()
        .expect("the new URL, shown once")
        .ends_with(&format!("/api/hooks/incoming/{new}")));

    let message = json!({ "text": "build passed" });
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/hooks/incoming/{old}"),
        None,
        Some(message.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "the URL it had before");
    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/hooks/incoming/{new}"),
        None,
        Some(message),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_outgoing_webhook_turned_back_on_signs_with_a_new_secret(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "hook-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Webhooks").await;
    let ch = seed_channel(&state, ws, owner_id, "deploys", false).await;

    let hook = create(
        &app,
        &owner,
        ws,
        json!({
            "hook_type": "outgoing_webhook",
            "name": "Mirror",
            "config": { "url": "https://example.com/out", "channel_ids": [ch] }
        }),
    )
    .await;
    let id = hook["id"].as_str().expect("id");
    let old = hook_credential(&state.pool, id, "secret").await;
    state
        .hook_repo
        .disable(id.parse().expect("uuid"), "Its creator left the workspace")
        .await
        .expect("disable");

    let (status, revived) = turn_on(&app, &owner, id).await;
    assert_eq!(status, StatusCode::OK, "{revived:?}");
    let new = hook_credential(&state.pool, id, "secret").await;
    assert_ne!(new, old);
    assert_eq!(revived["secrets"]["config"]["secret"], new.as_str());
    assert_eq!(revived["hook"]["config"]["secret"], "***");

    let fired = crate::hooks::executor::hooks_to_fire(&state, ws, ch).await;
    assert_eq!(fired.len(), 1);
    assert_eq!(
        fired[0].config["secret"],
        new.as_str(),
        "what it signs with"
    );

    let credential: Option<String> = sqlx::query_scalar(
        "SELECT details->>'credential' FROM audit_log WHERE action = 'hook.enabled' AND resource_id = $1::uuid",
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await
    .expect("audit");
    assert_eq!(credential.as_deref(), Some("secret"));

    let (status, again) = turn_on(&app, &owner, id).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        again["secrets"].is_null(),
        "a hook that is on is not rotated"
    );
    assert_eq!(hook_credential(&state.pool, id, "secret").await, new);
}

struct Bot {
    id: Uuid,
    token: String,
}

async fn make_bot(app: &axum::Router, token: &str, ws: Uuid, name: &str) -> Bot {
    let (status, made) = send(
        app,
        "POST",
        &format!("/api/workspaces/{ws}/bot-apps"),
        Some(token),
        Some(json!({ "name": name, "scopes": ["chat:write"] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{made:?}");
    let id: Uuid = made["id"].as_str().expect("id").parse().expect("uuid");
    let (status, body) = send(
        app,
        "PATCH",
        &format!("/api/bot-apps/{id}"),
        Some(token),
        Some(json!({ "events": ["message"], "socket_mode": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let (status, body) = send(
        app,
        "POST",
        &format!("/api/bot-apps/{id}/app-token"),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    Bot {
        id,
        token: made["token"].as_str().expect("token").to_string(),
    }
}

async fn bot_status(app: &axum::Router, token: &str) -> StatusCode {
    send(app, "GET", "/api/bot-apps/me", Some(token), None)
        .await
        .0
}

async fn listening(state: &crate::state::AppState, ws: Uuid, ch: Uuid, bot: &Bot) -> bool {
    state
        .app_event_repo
        .subscribers(ws, ch)
        .await
        .expect("subscribers")
        .iter()
        .any(|s| s.id == bot.id)
}

async fn read_app(app: &axum::Router, token: &str, bot: &Bot) -> Value {
    let (status, shown) = send(
        app,
        "GET",
        &format!("/api/bot-apps/{}", bot.id),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{shown:?}");
    shown
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_app_stops_with_its_creator_and_an_admin_turns_it_back_on(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "app-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Apps").await;
    let ch = seed_channel(&state, ws, owner_id, "lobby", false).await;
    let (admin_id, _, admin) = seed_and_login(&app, &state, "app-admin", false).await;
    add_ws_member(&state, ws, admin_id, "admin").await;
    let bot = make_bot(&app, &admin, ws, "Echo").await;
    assert_eq!(bot_status(&app, &bot.token).await, StatusCode::OK);
    assert!(listening(&state, ws, ch, &bot).await);

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/workspaces/{ws}/members/{admin_id}"),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    assert_eq!(bot_status(&app, &bot.token).await, StatusCode::UNAUTHORIZED);
    assert!(!listening(&state, ws, ch, &bot).await, "no events go out");
    let stopped = read_app(&app, &owner, &bot).await;
    assert_eq!(stopped["disabled_reason"], "Its creator left the workspace");
    assert!(stopped["disabled_at"].is_string());
    assert_eq!(stopped["has_app_token"], false, "its app token went too");
    assert_eq!(audited(&state.pool, "bot_app.disabled", bot.id).await, 1);

    let old_secret: Vec<u8> =
        sqlx::query_scalar("SELECT signing_secret_encrypted FROM bot_apps WHERE id = $1")
            .bind(bot.id)
            .fetch_one(&state.pool)
            .await
            .expect("secret");
    let (member_id, _, member) = seed_and_login(&app, &state, "app-member", false).await;
    add_ws_member(&state, ws, member_id, "member").await;
    let route = format!("/api/bot-apps/{}/enable", bot.id);
    let (status, _) = send(&app, "POST", &route, Some(&member), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, on) = send(&app, "POST", &route, Some(&owner), None).await;
    assert_eq!(status, StatusCode::OK, "{on:?}");
    let fresh = on["token"].as_str().expect("a new token, shown once");
    assert_ne!(fresh, bot.token);
    assert!(on["disabled_at"].is_null());
    assert_eq!(on["created_by"], owner_id.to_string());
    assert_eq!(bot_status(&app, fresh).await, StatusCode::OK);
    assert_eq!(
        bot_status(&app, &bot.token).await,
        StatusCode::UNAUTHORIZED,
        "the token it had stays refused"
    );
    assert!(listening(&state, ws, ch, &bot).await);
    let new_secret: Vec<u8> =
        sqlx::query_scalar("SELECT signing_secret_encrypted FROM bot_apps WHERE id = $1")
            .bind(bot.id)
            .fetch_one(&state.pool)
            .await
            .expect("secret");
    assert_ne!(new_secret, old_secret, "it signs with a new secret");
    assert_eq!(audited(&state.pool, "bot_app.enabled", bot.id).await, 1);

    let (status, _) = send(&app, "POST", &route, Some(&owner), None).await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn suspending_or_deprovisioning_an_apps_creator_stops_it(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_, _, instance_admin) = seed_and_login(&app, &state, "app-instance-admin", true).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "app-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Apps").await;
    let ch = seed_channel(&state, ws, owner_id, "lobby", false).await;
    let (suspended_id, _, suspended) = seed_and_login(&app, &state, "app-suspended", false).await;
    let (leaver_id, _, leaver) = seed_and_login(&app, &state, "app-leaver", false).await;
    add_ws_member(&state, ws, suspended_id, "admin").await;
    add_ws_member(&state, ws, leaver_id, "admin").await;
    let first = make_bot(&app, &suspended, ws, "First").await;
    let second = make_bot(&app, &leaver, ws, "Second").await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/admin/users/{suspended_id}/suspend"),
        Some(&instance_admin),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        bot_status(&app, &first.token).await,
        StatusCode::UNAUTHORIZED
    );
    assert!(!listening(&state, ws, ch, &first).await);
    let reason: Option<String> =
        sqlx::query_scalar("SELECT disabled_reason FROM bot_apps WHERE id = $1")
            .bind(first.id)
            .fetch_one(&state.pool)
            .await
            .expect("reason");
    assert_eq!(
        reason.as_deref(),
        Some("Its creator's account is suspended")
    );
    assert_eq!(bot_status(&app, &second.token).await, StatusCode::OK);

    let (status, scim) = send(
        &app,
        "POST",
        "/api/admin/scim/tokens",
        Some(&instance_admin),
        Some(json!({ "description": "Okta" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{scim:?}");
    let (status, body) = send(
        &app,
        "PATCH",
        &format!("/api/scim/v2/Users/{leaver_id}"),
        scim["token"].as_str(),
        Some(json!({
            "schemas": ["urn:ietf:params:scim:api:messages:2.0:PatchOp"],
            "Operations": [{ "op": "replace", "path": "active", "value": false }]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(
        bot_status(&app, &second.token).await,
        StatusCode::UNAUTHORIZED
    );
    assert!(!listening(&state, ws, ch, &second).await);
}

async fn invite(app: &axum::Router, token: &str, ws: Uuid, body: Value) -> (Uuid, String) {
    let (status, made) = send(
        app,
        "POST",
        &format!("/api/workspaces/{ws}/invites"),
        Some(token),
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{made:?}");
    (
        made["id"].as_str().expect("id").parse().expect("uuid"),
        made["token"].as_str().expect("token").to_string(),
    )
}

async fn invite_exists(pool: &PgPool, id: Uuid) -> bool {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM workspace_invites WHERE id = $1)")
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("invite")
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_removed_or_suspended_admins_invite_links_stop_but_email_invites_stay(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_, _, instance_admin) = seed_and_login(&app, &state, "inv-instance-admin", true).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "inv-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Invites").await;
    let (removed_id, _, removed) = seed_and_login(&app, &state, "inv-removed", false).await;
    let (suspended_id, _, suspended) = seed_and_login(&app, &state, "inv-suspended", false).await;
    add_ws_member(&state, ws, removed_id, "admin").await;
    add_ws_member(&state, ws, suspended_id, "admin").await;
    let link = json!({ "role": "member", "max_uses": 5 });
    let (removed_link, removed_token) = invite(&app, &removed, ws, link.clone()).await;
    let (removed_email, _) = invite(
        &app,
        &removed,
        ws,
        json!({ "email": "newcomer-121c@example.com", "role": "member" }),
    )
    .await;
    let (_, suspended_token) = invite(&app, &suspended, ws, link.clone()).await;
    let (_, owners_token) = invite(&app, &owner, ws, link).await;

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/workspaces/{ws}/members/{removed_id}"),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/admin/users/{suspended_id}/suspend"),
        Some(&instance_admin),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    assert!(!invite_exists(&state.pool, removed_link).await);
    assert!(
        invite_exists(&state.pool, removed_email).await,
        "an email invite names who it is for, so it stays"
    );
    assert_eq!(
        audited(&state.pool, "invite.revoked", removed_link).await,
        1
    );

    let (_, _, newcomer) = seed_and_login(&app, &state, "inv-newcomer", false).await;
    for token in [&removed_token, &suspended_token] {
        let (status, body) = send(
            &app,
            "POST",
            &format!("/api/invites/{token}/accept"),
            Some(&newcomer),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{body:?}");
    }
    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/invites/{owners_token}/accept"),
        Some(&newcomer),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn only_an_admin_reads_a_workspaces_apps(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "apps-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Apps").await;
    let bot = make_bot(&app, &owner, ws, "Echo").await;
    let list = format!("/api/workspaces/{ws}/bot-apps");
    let one = format!("/api/bot-apps/{}", bot.id);

    for (role, expected) in [
        ("admin", StatusCode::OK),
        ("member", StatusCode::FORBIDDEN),
        ("guest", StatusCode::FORBIDDEN),
    ] {
        let (user_id, _, token) =
            seed_and_login(&app, &state, &format!("apps-{role}"), false).await;
        add_ws_member(&state, ws, user_id, role).await;
        for route in [&list, &one] {
            let (status, body) = send(&app, "GET", route, Some(&token), None).await;
            assert_eq!(status, expected, "{role} on {route}: {body:?}");
        }
    }
}
