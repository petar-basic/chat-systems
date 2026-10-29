use axum::http::StatusCode;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;

const EVERY_SCOPE: [&str; 10] = [
    "chat:write",
    "chat:write.public",
    "channels:read",
    "channels:history",
    "groups:history",
    "im:write",
    "reactions:write",
    "files:write",
    "users:read",
    "commands",
];

struct Bot {
    app_id: Uuid,
    user_id: Uuid,
    token: String,
}

async fn make_app(
    app: &axum::Router,
    admin_token: &str,
    ws_id: Uuid,
    name: &str,
    scopes: &[&str],
) -> Bot {
    let (status, body) = send(
        app,
        "POST",
        &format!("/api/workspaces/{ws_id}/bot-apps"),
        Some(admin_token),
        Some(json!({ "name": name, "description": "Says it back", "scopes": scopes })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    Bot {
        app_id: body["id"].as_str().expect("id").parse().expect("uuid"),
        user_id: body["bot_user_id"]
            .as_str()
            .expect("bot_user_id")
            .parse()
            .expect("uuid"),
        token: body["token"].as_str().expect("token").to_string(),
    }
}

async fn add_to_channel(app: &axum::Router, token: &str, ch_id: Uuid, user_id: Uuid) {
    let (status, body) = send(
        app,
        "POST",
        &format!("/api/channels/{ch_id}/members"),
        Some(token),
        Some(json!({ "user_id": user_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
}

async fn post(app: &axum::Router, token: &str, ch_id: Uuid, body: Value) -> (StatusCode, Value) {
    send(
        app,
        "POST",
        &format!("/api/channels/{ch_id}/messages"),
        Some(token),
        Some(body),
    )
    .await
}

async fn history(app: &axum::Router, token: &str, ch_id: Uuid) -> (StatusCode, Value) {
    send(
        app,
        "GET",
        &format!("/api/channels/{ch_id}/messages"),
        Some(token),
        None,
    )
    .await
}

async fn upload(app: &axum::Router, token: &str, ws_id: Uuid) -> StatusCode {
    let boundary = "BOT-BOUNDARY";
    let body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"report.txt\"\r\nContent-Type: text/plain\r\n\r\nall green\r\n--{boundary}--\r\n"
    );
    let request = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/api/files/upload/{ws_id}?draft=true"))
        .header(axum::http::header::AUTHORIZATION, format!("Bearer {token}"))
        .header(
            axum::http::header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(axum::body::Body::from(body))
        .expect("request");
    tower::ServiceExt::oneshot(app.clone(), request)
        .await
        .expect("response")
        .status()
}

fn refusal(body: &Value) -> String {
    body["error"].as_str().unwrap_or_default().to_string()
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_admin_makes_an_app_whose_token_is_shown_once_and_stored_hashed(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner, _, token) = seed_and_login(&app, &state, "bot-owner", false).await;
    let ws_id = seed_workspace(&state, owner, "Bots").await;
    let bot = make_app(&app, &token, ws_id, "Echo", &["chat:write"]).await;

    assert!(bot.token.starts_with("cbot-"));
    let stored: String = sqlx::query_scalar("SELECT token_hash FROM bot_apps WHERE id = $1")
        .bind(bot.app_id)
        .fetch_one(&state.pool)
        .await
        .expect("stored");
    assert_eq!(stored, crate::bots::hash_token(&bot.token));
    assert!(!stored.contains(&bot.token));

    let (status, shown) = send(
        &app,
        "GET",
        &format!("/api/bot-apps/{}", bot.app_id),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(shown.get("token").is_none(), "shown once, at creation");
    assert_eq!(shown["scopes"], json!(["chat:write"]));

    let (status, members) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/members"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let listed = members["data"]
        .as_array()
        .expect("members")
        .iter()
        .find(|m| m["user_id"] == bot.user_id.to_string())
        .expect("the bot is a member")
        .clone();
    assert_eq!(listed["is_bot"], true);
    assert_eq!(listed["display_name"], "Echo");
    assert_eq!(listed["email"], "", "a bot has no address to show");

    let email: String = sqlx::query_scalar("SELECT email FROM users WHERE id = $1")
        .bind(bot.user_id)
        .fetch_one(&state.pool)
        .await
        .expect("email");
    let (status, _) = send(
        &app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({ "email": email, "password": PASSWORD })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "a bot cannot sign in");

    let (member, _, member_token) = seed_and_login(&app, &state, "bot-member", false).await;
    add_ws_member(&state, ws_id, member, "member").await;
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/bot-apps"),
        Some(&member_token),
        Some(json!({ "name": "Mine", "scopes": [] })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_bot_token_works_only_on_the_routes_that_take_one(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner, _, token) = seed_and_login(&app, &state, "bot-routes", false).await;
    let ws_id = seed_workspace(&state, owner, "Bot routes").await;
    let bot = make_app(&app, &token, ws_id, "Echo", &EVERY_SCOPE).await;

    let (status, me) = send(&app, "GET", "/api/bot-apps/me", Some(&bot.token), None).await;
    assert_eq!(status, StatusCode::OK, "{me:?}");
    assert_eq!(me["id"], bot.app_id.to_string());
    assert_eq!(me["bot_user_id"], bot.user_id.to_string());

    let (status, body) = send(&app, "GET", "/api/users/me", Some(&bot.token), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body:?}");
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/bot-apps"),
        Some(&bot.token),
        Some(json!({ "name": "Child", "scopes": [] })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "a bot makes no apps");

    let (status, _) = send(&app, "GET", "/api/bot-apps/me", Some("cbot-forged"), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = send(&app, "GET", "/api/bot-apps/me", Some(&token), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "a person has no app");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn each_scope_opens_its_routes_and_nothing_else(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner, _, token) = seed_and_login(&app, &state, "bot-scopes", false).await;
    let ws_id = seed_workspace(&state, owner, "Bot scopes").await;
    let ch_id = seed_channel(&state, ws_id, owner, "ops", false).await;
    let bot = make_app(&app, &token, ws_id, "Echo", &["chat:write"]).await;
    add_to_channel(&app, &token, ch_id, bot.user_id).await;

    let (status, posted) = post(&app, &bot.token, ch_id, json!({ "content": "hello" })).await;
    assert_eq!(status, StatusCode::OK, "{posted:?}");
    assert_eq!(posted["user_id"], bot.user_id.to_string());
    let msg_id = posted["id"].as_str().expect("id").to_string();

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/messages/{msg_id}/reactions"),
        Some(&bot.token),
        Some(json!({ "emoji": "👍" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(refusal(&body).contains("reactions:write"), "{body:?}");
    let (status, body) = history(&app, &bot.token, ch_id).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(refusal(&body).contains("channels:history"), "{body:?}");
    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/members"),
        Some(&bot.token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(refusal(&body).contains("users:read"), "{body:?}");
    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/channels"),
        Some(&bot.token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(upload(&app, &bot.token, ws_id).await, StatusCode::FORBIDDEN);

    let (status, body) = send(
        &app,
        "PATCH",
        &format!("/api/bot-apps/{}", bot.app_id),
        Some(&token),
        Some(json!({ "scopes": EVERY_SCOPE })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/messages/{msg_id}/reactions"),
        Some(&bot.token),
        Some(json!({ "emoji": "👍" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, reply) = post(
        &app,
        &bot.token,
        ch_id,
        json!({ "content": "in the thread", "thread_parent_id": msg_id }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reply:?}");
    assert_eq!(reply["thread_parent_id"], msg_id.as_str());
    let (status, thread) = send(
        &app,
        "GET",
        &format!("/api/messages/{msg_id}/thread"),
        Some(&bot.token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{thread:?}");
    let (status, page) = history(&app, &bot.token, ch_id).await;
    assert_eq!(status, StatusCode::OK);
    assert!(page["data"]
        .as_array()
        .expect("messages")
        .iter()
        .any(|m| m["id"] == msg_id.as_str()));
    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/members"),
        Some(&bot.token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, channels) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/channels"),
        Some(&bot.token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(channels["data"].as_array().expect("channels").len(), 1);
    assert_eq!(upload(&app, &bot.token, ws_id).await, StatusCode::OK);
    let (status, _) = send(
        &app,
        "PATCH",
        &format!("/api/messages/{msg_id}"),
        Some(&bot.token),
        Some(json!({ "content": "hello again" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_bot_reads_and_writes_only_where_it_was_added(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner, _, token) = seed_and_login(&app, &state, "bot-member", false).await;
    let ws_id = seed_workspace(&state, owner, "Bot membership").await;
    let public = seed_channel(&state, ws_id, owner, "general-talk", false).await;
    let private = seed_channel(&state, ws_id, owner, "secret", true).await;
    let bot = make_app(
        &app,
        &token,
        ws_id,
        "Echo",
        &["chat:write", "channels:history", "groups:history"],
    )
    .await;

    let (status, _) = post(&app, &bot.token, public, json!({ "content": "hi" })).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a public channel is not open to a bot"
    );
    let (status, _) = history(&app, &bot.token, public).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = history(&app, &bot.token, private).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    send(
        &app,
        "PATCH",
        &format!("/api/bot-apps/{}", bot.app_id),
        Some(&token),
        Some(json!({ "scopes": ["chat:write", "chat:write.public", "channels:history", "groups:history"] })),
    )
    .await;
    let (status, body) = post(&app, &bot.token, public, json!({ "content": "hi" })).await;
    assert_eq!(status, StatusCode::OK, "chat:write.public: {body:?}");
    let (status, _) = history(&app, &bot.token, public).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "posting is not reading");
    let (status, _) = post(&app, &bot.token, private, json!({ "content": "hi" })).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "public means public");

    add_to_channel(&app, &token, private, bot.user_id).await;
    let (status, _) = history(&app, &bot.token, private).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = post(&app, &bot.token, private, json!({ "content": "inside" })).await;
    assert_eq!(status, StatusCode::OK);

    sqlx::query("UPDATE channels SET is_archived = true WHERE id = $1")
        .bind(private)
        .execute(&state.pool)
        .await
        .expect("archive");
    let (status, _) = post(&app, &bot.token, private, json!({ "content": "late" })).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "an archived channel takes no posts"
    );
    let (status, _) = history(&app, &bot.token, private).await;
    assert_eq!(status, StatusCode::OK, "and is still read");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_bot_talks_to_a_guest_in_a_dm(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner, _, token) = seed_and_login(&app, &state, "bot-dm-owner", false).await;
    let ws_id = seed_workspace(&state, owner, "Bot DMs").await;
    let (guest, _, guest_token) = seed_and_login(&app, &state, "bot-dm-guest", false).await;
    add_ws_member(&state, ws_id, guest, "guest").await;
    let bot = make_app(
        &app,
        &token,
        ws_id,
        "Echo",
        &["chat:write", "groups:history"],
    )
    .await;

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&bot.token),
        Some(json!({ "participant_ids": [guest] })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(refusal(&body).contains("im:write"), "{body:?}");

    send(
        &app,
        "PATCH",
        &format!("/api/bot-apps/{}", bot.app_id),
        Some(&token),
        Some(json!({ "scopes": ["chat:write", "groups:history", "im:write"] })),
    )
    .await;
    let (status, dm) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/conversations"),
        Some(&bot.token),
        Some(json!({ "participant_ids": [guest] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{dm:?}");
    let dm_id: Uuid = dm["id"].as_str().expect("id").parse().expect("uuid");

    let (status, _) = post(
        &app,
        &bot.token,
        dm_id,
        json!({ "content": "your build passed" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, page) = history(&app, &guest_token, dm_id).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["data"][0]["content"], "your build passed");
    let (status, _) = post(&app, &guest_token, dm_id, json!({ "content": "thanks" })).await;
    assert_eq!(status, StatusCode::OK);
    let (status, page) = history(&app, &bot.token, dm_id).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["data"][0]["content"], "thanks");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn removing_an_app_revokes_its_token_and_its_commands(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner, _, token) = seed_and_login(&app, &state, "bot-remove", false).await;
    let ws_id = seed_workspace(&state, owner, "Bot removal").await;
    let ch_id = seed_channel(&state, ws_id, owner, "deploys", false).await;
    let bot = make_app(&app, &token, ws_id, "Deployer", &["chat:write"]).await;

    let command = |name: &str| {
        json!({
            "hook_type": "slash_command",
            "name": name,
            "app_id": bot.app_id,
            "config": { "command": name, "url": "https://deployer.example/command", "channel_ids": [ch_id] }
        })
    };
    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/hooks"),
        Some(&token),
        Some(command("deploy")),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body:?}");
    assert!(refusal(&body).contains("commands"));

    send(
        &app,
        "PATCH",
        &format!("/api/bot-apps/{}", bot.app_id),
        Some(&token),
        Some(json!({ "scopes": ["chat:write", "commands"] })),
    )
    .await;
    let (status, hook) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/hooks"),
        Some(&token),
        Some(command("deploy")),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{hook:?}");
    assert_eq!(hook["app_id"], bot.app_id.to_string());
    let hook_id = hook["id"].as_str().expect("id").to_string();

    let (status, rotated) = send(
        &app,
        "POST",
        &format!("/api/bot-apps/{}/rotate", bot.app_id),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let fresh = rotated["token"].as_str().expect("token").to_string();
    let (status, _) = send(&app, "GET", "/api/bot-apps/me", Some(&bot.token), None).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "the old token stops at once"
    );
    let (status, _) = send(&app, "GET", "/api/bot-apps/me", Some(&fresh), None).await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/bot-apps/{}", bot.app_id),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = send(&app, "GET", "/api/bot-apps/me", Some(&fresh), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/hooks/{hook_id}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "its command went with it");
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/commands"),
        Some(&token),
        Some(json!({ "command": "deploy", "text": "" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (suspended, member): (bool, bool) = sqlx::query_as(
        "SELECT status = 'suspended', EXISTS (SELECT 1 FROM workspace_members WHERE user_id = $1) \
           FROM users WHERE id = $1",
    )
    .bind(bot.user_id)
    .fetch_one(&state.pool)
    .await
    .expect("bot user");
    assert!(suspended);
    assert!(!member);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn what_a_bot_does_is_audited_under_its_app(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner, _, token) = seed_and_login(&app, &state, "bot-audit", false).await;
    let ws_id = seed_workspace(&state, owner, "Bot audit").await;
    let ch_id = seed_channel(&state, ws_id, owner, "ops", false).await;
    let bot = make_app(
        &app,
        &token,
        ws_id,
        "Echo",
        &["chat:write", "channels:history"],
    )
    .await;
    add_to_channel(&app, &token, ch_id, bot.user_id).await;

    post(&app, &bot.token, ch_id, json!({ "content": "done" })).await;
    history(&app, &bot.token, ch_id).await;
    post(
        &app,
        &bot.token,
        Uuid::new_v4(),
        json!({ "content": "nowhere" }),
    )
    .await;

    let rows: Vec<(String, Option<Uuid>, Option<Uuid>, Value)> = sqlx::query_as(
        "SELECT action, user_id, resource_id, details FROM audit_log \
          WHERE workspace_id = $1 AND action LIKE 'bot_app.%' ORDER BY created_at",
    )
    .bind(ws_id)
    .fetch_all(&state.pool)
    .await
    .expect("audit");
    let actions: Vec<&str> = rows.iter().map(|r| r.0.as_str()).collect();
    assert_eq!(actions, vec!["bot_app.created", "bot_app.called"]);
    let called = &rows[1];
    assert_eq!(called.1, Some(bot.user_id));
    assert_eq!(called.2, Some(bot.app_id));
    assert_eq!(called.3["method"], "POST");
    assert!(called.3["route"]
        .as_str()
        .unwrap_or_default()
        .ends_with("/channels/{ch_id}/messages"));
}

#[test_macros::db_test(migrations = "../migrations")]
async fn each_bot_token_has_a_read_budget_of_its_own(pool: PgPool) {
    use redis::AsyncCommands;

    let (app, state) = app_and_state(pool).await;
    let (owner, _, token) = seed_and_login(&app, &state, "bot-budget", false).await;
    let ws_id = seed_workspace(&state, owner, "Bot budget").await;
    let ch_id = seed_channel(&state, ws_id, owner, "ops", false).await;
    let busy = make_app(&app, &token, ws_id, "Busy", &["channels:history"]).await;
    let quiet = make_app(&app, &token, ws_id, "Quiet", &["channels:history"]).await;
    add_to_channel(&app, &token, ch_id, busy.user_id).await;
    add_to_channel(&app, &token, ch_id, quiet.user_id).await;

    let mut redis = state.redis.clone();
    let _: () = redis
        .set_ex(
            format!(
                "rate_limit:write:{}:{}",
                crate::rate_limit::BOT_READ.name,
                busy.user_id
            ),
            crate::rate_limit::BOT_READ.max,
            60,
        )
        .await
        .expect("spend the budget");

    let (status, _) = history(&app, &busy.token, ch_id).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    let (status, _) = history(&app, &quiet.token, ch_id).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = history(&app, &token, ch_id).await;
    assert_eq!(status, StatusCode::OK, "people's reads are not limited");
}
