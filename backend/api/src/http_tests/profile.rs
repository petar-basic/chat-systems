use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
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
async fn a_photo_set_in_a_workspace_is_shown_to_it(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner) = seed_and_login(&app, &state, "avatar-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Avatar WS").await;
    let (member_id, _, member_token) = seed_and_login(&app, &state, "avatar-member", false).await;
    add_ws_member(&state, ws_id, member_id, "member").await;

    let boundary = "----avatartest";
    let mut body = Vec::new();
    body.extend_from_slice(
        format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"me.txt\"\r\nContent-Type: text/plain\r\n\r\n").as_bytes(),
    );
    body.extend_from_slice(b"not really a png");
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let request = Request::builder()
        .method("POST")
        .uri(format!("/api/files/upload/{ws_id}"))
        .header(header::AUTHORIZATION, format!("Bearer {owner}"))
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .expect("build request");
    let response = app.clone().oneshot(request).await.expect("upload");
    assert_eq!(response.status(), StatusCode::OK);
    let uploaded: Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body"),
    )
    .expect("json");
    let url = uploaded[0]["url"].as_str().expect("url").to_string();
    let path = url[url.find("/api/").expect("api path")..].to_string();

    let (status, _) = send(&app, "GET", &path, Some(&member_token), None).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "an unposted upload is its owner's"
    );

    let (status, _) = send(
        &app,
        "PATCH",
        &format!("/api/workspaces/{ws_id}/profile"),
        Some(&owner),
        Some(json!({ "avatar_url": url })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send(&app, "GET", &path, Some(&member_token), None).await;
    assert_eq!(status, StatusCode::OK, "a photo is there to be seen");
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
