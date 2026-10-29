use axum::http::StatusCode;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;

async fn member(app: &axum::Router, token: &str, ws_id: Uuid, user_id: Uuid) -> Value {
    let (status, members) = send(
        app,
        "GET",
        &format!("/api/workspaces/{ws_id}/members"),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{members:?}");
    members["data"]
        .as_array()
        .expect("array")
        .iter()
        .find(|m| m["user_id"] == user_id.to_string())
        .expect("a member")
        .clone()
}

async fn set_status(
    app: &axum::Router,
    token: &str,
    ws_id: Uuid,
    body: Value,
) -> (StatusCode, Value) {
    send(
        app,
        "PUT",
        &format!("/api/workspaces/{ws_id}/profile/status"),
        Some(token),
        Some(body),
    )
    .await
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_status_belongs_to_the_workspace_it_was_set_in(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "status-owner", false).await;
    let work = seed_workspace(&state, owner_id, "Work").await;
    let hobby = seed_workspace(&state, owner_id, "Hobby").await;

    let (status, profile) = set_status(
        &app,
        &token,
        work,
        json!({ "emoji": "🍕", "text": "at lunch" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{profile:?}");
    assert_eq!(profile["status_emoji"], "🍕");
    assert_eq!(
        member(&app, &token, work, owner_id).await["status_text"],
        "at lunch"
    );
    assert!(member(&app, &token, hobby, owner_id).await["status_text"].is_null());

    let (status, _) = set_status(
        &app,
        &token,
        hobby,
        json!({ "emoji": "🎸", "text": "practising" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        member(&app, &token, hobby, owner_id).await["status_text"],
        "practising"
    );
    assert_eq!(
        member(&app, &token, work, owner_id).await["status_text"],
        "at lunch"
    );

    let (status, cleared) = send(
        &app,
        "DELETE",
        &format!("/api/workspaces/{work}/profile/status"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(cleared["status_text"].is_null());
    assert_eq!(
        member(&app, &token, hobby, owner_id).await["status_text"],
        "practising"
    );

    let (_, me) = send(&app, "GET", "/api/users/me", Some(&token), None).await;
    assert!(
        me.get("status_text").is_none(),
        "the account has no status: {me:?}"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_expired_status_stops_being_shown(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "status-expiry", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Status WS").await;

    let (status, _) = set_status(
        &app,
        &token,
        ws_id,
        json!({
            "emoji": "🏝️",
            "text": "on holiday",
            "expires_at": (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339(),
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    sqlx::query(
        "UPDATE workspace_members SET status_expires_at = NOW() - interval '1 minute' WHERE user_id = $1",
    )
    .bind(owner_id)
    .execute(&state.pool)
    .await
    .expect("expire the status");

    let (_, profile) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/profile"),
        Some(&token),
        None,
    )
    .await;
    assert!(profile["status_text"].is_null(), "{profile:?}");
    assert!(member(&app, &token, ws_id, owner_id).await["status_text"].is_null());
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_status_has_to_say_something_this_workspace_can_show(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "status-empty", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Status WS").await;

    for (body, why) in [
        (json!({ "text": "   " }), "nothing"),
        (json!({ "text": "x".repeat(200) }), "too long"),
        (
            json!({
                "text": "back soon",
                "expires_at": (chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339(),
            }),
            "an expiry in the past",
        ),
    ] {
        let (status, _) = set_status(&app, &token, ws_id, body).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{why}");
    }

    let (status, body) = set_status(&app, &token, ws_id, json!({ "emoji": ":shipit:" })).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(body
        .to_string()
        .contains(":shipit: is not an emoji in this workspace"));

    let emoji_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO workspace_emojis (id, workspace_id, name, storage_key, created_by) VALUES ($1, $2, 'shipit', 'emoji/x.png', $3)",
    )
    .bind(emoji_id)
    .bind(ws_id)
    .bind(owner_id)
    .execute(&state.pool)
    .await
    .expect("an emoji");
    let (status, profile) = set_status(&app, &token, ws_id, json!({ "emoji": ":shipit:" })).await;
    assert_eq!(status, StatusCode::OK, "{profile:?}");
    assert_eq!(profile["status_emoji"], ":shipit:");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_name_photo_and_bio_of_its_own_and_the_accounts_otherwise(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "profile-owner", false).await;
    let work = seed_workspace(&state, owner_id, "Work").await;
    let hobby = seed_workspace(&state, owner_id, "Hobby").await;
    let (guest_id, _, guest) = seed_and_login(&app, &state, "profile-guest", false).await;
    add_ws_member(&state, work, guest_id, "guest").await;
    let channel = seed_channel(&state, work, owner_id, "shared", false).await;
    for user in [owner_id, guest_id] {
        sqlx::query("INSERT INTO channel_members (channel_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
            .bind(channel)
            .bind(user)
            .execute(&state.pool)
            .await
            .expect("join the channel");
    }

    let (status, account) = send(
        &app,
        "PATCH",
        "/api/users/me",
        Some(&owner),
        Some(json!({ "display_name": "Alice Account", "bio": "everywhere" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{account:?}");

    let (status, profile) = send(
        &app,
        "PATCH",
        &format!("/api/workspaces/{work}/profile"),
        Some(&owner),
        Some(json!({ "display_name": "Alice at Work", "avatar_url": "/api/files/download/a.png", "bio": "ships things" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{profile:?}");
    assert_eq!(profile["display_name"], "Alice at Work");

    let at_work = member(&app, &owner, work, owner_id).await;
    assert_eq!(at_work["display_name"], "Alice at Work");
    assert_eq!(at_work["avatar_url"], "/api/files/download/a.png");
    assert_eq!(at_work["bio"], "ships things");
    assert_eq!(
        member(&app, &guest, work, owner_id).await["display_name"],
        "Alice at Work",
        "a guest sees the same name"
    );
    let at_hobby = member(&app, &owner, hobby, owner_id).await;
    assert_eq!(at_hobby["display_name"], "Alice Account");
    assert_eq!(at_hobby["bio"], "everywhere");

    let (status, profile) = send(
        &app,
        "PATCH",
        &format!("/api/workspaces/{work}/profile"),
        Some(&owner),
        Some(json!({ "display_name": "" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(profile["display_name"].is_null());
    assert_eq!(profile["bio"], "ships things", "a field left out stays");
    assert_eq!(
        member(&app, &owner, work, owner_id).await["display_name"],
        "Alice Account"
    );

    let (status, _) = send(
        &app,
        "PATCH",
        &format!("/api/workspaces/{work}/profile"),
        Some(&owner),
        Some(json!({ "bio": "x".repeat(501) })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (_, _, outsider) = seed_and_login(&app, &state, "profile-out", false).await;
    let (status, _) = send(
        &app,
        "PATCH",
        &format!("/api/workspaces/{work}/profile"),
        Some(&outsider),
        Some(json!({ "display_name": "Intruder" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn erasing_a_person_clears_their_profile_in_every_workspace(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (subject_id, _, subject) = seed_and_login(&app, &state, "erase-subject", false).await;
    let ws_id = seed_workspace(&state, subject_id, "Erase WS").await;
    send(
        &app,
        "PATCH",
        &format!("/api/workspaces/{ws_id}/profile"),
        Some(&subject),
        Some(json!({ "display_name": "Findable", "bio": "about me" })),
    )
    .await;
    set_status(&app, &subject, ws_id, json!({ "text": "here" })).await;

    let (_, _, instance_admin) = seed_and_login(&app, &state, "erase-admin", true).await;
    let (status, body) = send(
        &app,
        "DELETE",
        &format!("/api/admin/users/{subject_id}/data"),
        Some(&instance_admin),
        Some(json!({ "hard_delete": false })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");

    let left: (Option<String>, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT display_name, bio, status_text FROM workspace_members WHERE user_id = $1",
    )
    .bind(subject_id)
    .fetch_one(&state.pool)
    .await
    .expect("the membership");
    assert_eq!(left, (None, None, None));
}

async fn manual_away(state: &crate::state::AppState, user_id: Uuid) -> bool {
    sqlx::query_scalar!("SELECT manual_away FROM users WHERE id = $1", user_id)
        .fetch_one(&state.pool)
        .await
        .expect("read manual_away")
}

async fn presence_event_for(
    stream: &mut (impl futures_util::Stream<Item = redis::Msg> + Unpin),
    user_id: Uuid,
) -> Option<Value> {
    use futures_util::StreamExt;
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        let message = tokio::time::timeout_at(deadline, stream.next())
            .await
            .ok()??;
        let Ok(payload) = message.get_payload::<String>() else {
            continue;
        };
        let Ok(event) = serde_json::from_str::<Value>(&payload) else {
            continue;
        };
        if event["payload"]["user_id"] == user_id.to_string() {
            return Some(event);
        }
    }
}

#[test_macros::db_test(migrations = "../migrations")]
async fn setting_yourself_away_is_kept_and_tells_the_gateway(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (user_id, _, token) = seed_and_login(&app, &state, "away-user", false).await;

    let url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into());
    let mut pubsub = redis::Client::open(url)
        .expect("redis client")
        .get_async_pubsub()
        .await
        .expect("pubsub");
    pubsub
        .subscribe("events:presence")
        .await
        .expect("subscribe");
    let mut events = pubsub.into_on_message();

    let (status, body) = send(
        &app,
        "PUT",
        "/api/users/me/presence",
        Some(&token),
        Some(json!({ "away": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["away"], true);
    assert!(
        manual_away(&state, user_id).await,
        "kept across devices and restarts"
    );

    let event = presence_event_for(&mut events, user_id)
        .await
        .expect("the gateway is asked to work the status out again");
    assert_eq!(event["event_type"], "presence.recompute");

    let (status, body) = send(
        &app,
        "PUT",
        "/api/users/me/presence",
        Some(&token),
        Some(json!({ "away": false })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert!(!manual_away(&state, user_id).await);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_away_command_switches_between_away_and_active(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (user_id, _, token) = seed_and_login(&app, &state, "away-cmd", false).await;
    let ws_id = seed_workspace(&state, user_id, "Away").await;
    let ch_id = seed_channel(&state, ws_id, user_id, "main", false).await;

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/commands"),
        Some(&token),
        Some(json!({ "command": "/away", "text": "" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["response_type"], "ephemeral", "nobody else is told");
    assert!(manual_away(&state, user_id).await);

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/commands"),
        Some(&token),
        Some(json!({ "command": "away", "text": "" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        !manual_away(&state, user_id).await,
        "a second /away sets you active"
    );
}
