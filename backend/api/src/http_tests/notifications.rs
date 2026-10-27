use super::common::*;
use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

use crate::notifications::models::NotificationType;

async fn seed_notification(
    state: &crate::state::AppState,
    user_id: uuid::Uuid,
    ws_id: uuid::Uuid,
) -> uuid::Uuid {
    state
        .notification_repo
        .create(
            user_id,
            ws_id,
            &NotificationType::Mention,
            "You were mentioned",
            Some("hello"),
            &json!({ "message_id": uuid::Uuid::new_v4() }),
        )
        .await
        .expect("seed notification")
        .expect("each seeded notification carries a distinct message_id")
        .id
}

#[test_macros::db_test(migrations = "../migrations")]
async fn list_notifications_returns_seeded_rows(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (uid, _email, token) = seed_and_login(&app, &state, "notif-list", false).await;
    let ws = seed_workspace(&state, uid, "Notif WS").await;
    let n1 = seed_notification(&state, uid, ws).await;
    let n2 = seed_notification(&state, uid, ws).await;

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/notifications"),
        Some(&token),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::OK, "list should be 200: {body:?}");
    let data = body["data"].as_array().expect("data array");
    assert_eq!(
        data.len(),
        2,
        "both seeded notifications must be listed: {body:?}"
    );
    let ids: Vec<&str> = data.iter().filter_map(|n| n["id"].as_str()).collect();
    assert!(ids.contains(&n1.to_string().as_str()));
    assert!(ids.contains(&n2.to_string().as_str()));
}

#[test_macros::db_test(migrations = "../migrations")]
async fn list_notifications_without_token_is_401(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (uid, _email, _token) = seed_and_login(&app, &state, "notif-list-noauth", false).await;
    let ws = seed_workspace(&state, uid, "Notif WS").await;

    let (status, _body) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/notifications"),
        None,
        None,
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED, "no token must be 401");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn list_notifications_non_member_forbidden(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner, _e1, _t1) = seed_and_login(&app, &state, "notif-owner", false).await;
    let ws = seed_workspace(&state, owner, "Notif WS").await;
    let _n = seed_notification(&state, owner, ws).await;

    let (_other, _e2, other_token) = seed_and_login(&app, &state, "notif-other", false).await;

    let (status, _body) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/notifications"),
        Some(&other_token),
        None,
    )
    .await;

    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a non-member of the workspace must be rejected"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn unread_count_reflects_seeded_unread(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (uid, _email, token) = seed_and_login(&app, &state, "notif-count", false).await;
    let ws = seed_workspace(&state, uid, "Notif WS").await;
    seed_notification(&state, uid, ws).await;
    seed_notification(&state, uid, ws).await;
    seed_notification(&state, uid, ws).await;

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/notifications/unread-count"),
        Some(&token),
        None,
    )
    .await;

    assert_eq!(
        status,
        StatusCode::OK,
        "unread-count should be 200: {body:?}"
    );
    assert_eq!(
        body["unread_count"].as_i64(),
        Some(3),
        "three unread notifications expected: {body:?}"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn unread_count_without_token_is_401(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (uid, _email, _token) = seed_and_login(&app, &state, "notif-count-noauth", false).await;
    let ws = seed_workspace(&state, uid, "Notif WS").await;

    let (status, _body) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/notifications/unread-count"),
        None,
        None,
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED, "no token must be 401");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn mark_read_clears_specified_notifications(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (uid, _email, token) = seed_and_login(&app, &state, "notif-mark", false).await;
    let ws = seed_workspace(&state, uid, "Notif WS").await;
    let n1 = seed_notification(&state, uid, ws).await;
    let n2 = seed_notification(&state, uid, ws).await;

    let (_s, before) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/notifications/unread-count"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(before["unread_count"].as_i64(), Some(2));

    let (status, body) = send(
        &app,
        "POST",
        "/api/notifications/read",
        Some(&token),
        Some(json!({ "notification_ids": [n1.to_string()] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "mark-read should be 200: {body:?}");

    let (_s, after) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/notifications/unread-count"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(
        after["unread_count"].as_i64(),
        Some(1),
        "only n1 was marked read; n2 ({n2}) must remain unread: {after:?}"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn mark_read_with_empty_ids_is_ok(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (uid, _email, token) = seed_and_login(&app, &state, "notif-mark-empty", false).await;
    let ws = seed_workspace(&state, uid, "Notif WS").await;
    seed_notification(&state, uid, ws).await;

    let (status, body) = send(
        &app,
        "POST",
        "/api/notifications/read",
        Some(&token),
        Some(json!({ "notification_ids": [] })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "empty mark-read should be 200: {body:?}"
    );

    let (_s, after) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/notifications/unread-count"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(
        after["unread_count"].as_i64(),
        Some(1),
        "empty list must not change anything"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn mark_read_without_token_is_401(pool: PgPool) {
    let (app, _state) = app_and_state(pool).await;
    let (status, _body) = send(
        &app,
        "POST",
        "/api/notifications/read",
        None,
        Some(json!({ "notification_ids": [] })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "no token must be 401");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn mark_read_missing_field_is_422(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_uid, _email, token) = seed_and_login(&app, &state, "notif-mark-bad", false).await;

    let (status, _body) = send(
        &app,
        "POST",
        "/api/notifications/read",
        Some(&token),
        Some(json!({})),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "missing notification_ids must be 422"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn mark_read_cannot_affect_other_users_notification(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner, _e1, owner_token) = seed_and_login(&app, &state, "notif-victim", false).await;
    let ws = seed_workspace(&state, owner, "Notif WS").await;
    let owners_notif = seed_notification(&state, owner, ws).await;

    let (_attacker, _e2, attacker_token) =
        seed_and_login(&app, &state, "notif-attacker", false).await;

    let (status, _body) = send(
        &app,
        "POST",
        "/api/notifications/read",
        Some(&attacker_token),
        Some(json!({ "notification_ids": [owners_notif.to_string()] })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "request itself is well-formed -> 200"
    );

    let (_s, after) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/notifications/unread-count"),
        Some(&owner_token),
        None,
    )
    .await;
    assert_eq!(
        after["unread_count"].as_i64(),
        Some(1),
        "another user must not be able to mark the owner's notification read"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn mark_all_read_clears_unread_count(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (uid, _email, token) = seed_and_login(&app, &state, "notif-markall", false).await;
    let ws = seed_workspace(&state, uid, "Notif WS").await;
    seed_notification(&state, uid, ws).await;
    seed_notification(&state, uid, ws).await;

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/notifications/read-all"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "read-all should be 200: {body:?}");

    let (_s, after) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/notifications/unread-count"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(
        after["unread_count"].as_i64(),
        Some(0),
        "after read-all there must be zero unread: {after:?}"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn mark_all_read_is_scoped_to_workspace(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (uid, _email, token) = seed_and_login(&app, &state, "notif-markall-scope", false).await;
    let ws_a = seed_workspace(&state, uid, "WS A").await;
    let ws_b = seed_workspace(&state, uid, "WS B").await;
    seed_notification(&state, uid, ws_a).await;
    seed_notification(&state, uid, ws_b).await;

    let (status, _body) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_a}/notifications/read-all"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (_sa, count_a) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_a}/notifications/unread-count"),
        Some(&token),
        None,
    )
    .await;
    let (_sb, count_b) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_b}/notifications/unread-count"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(count_a["unread_count"].as_i64(), Some(0), "WS A cleared");
    assert_eq!(count_b["unread_count"].as_i64(), Some(1), "WS B untouched");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn mark_all_read_without_token_is_401(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (uid, _email, _token) = seed_and_login(&app, &state, "notif-markall-noauth", false).await;
    let ws = seed_workspace(&state, uid, "Notif WS").await;

    let (status, _body) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws}/notifications/read-all"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "no token must be 401");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_member_sets_and_clears_a_channel_notify_level(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (uid, _email, token) = seed_and_login(&app, &state, "level-owner", false).await;
    let ws = seed_workspace(&state, uid, "Levels WS").await;
    let ch = seed_channel(&state, ws, uid, "incidents", false).await;
    let route = format!("/api/channels/{ch}/notify-level");

    let (status, body) = send(
        &app,
        "PUT",
        &route,
        Some(&token),
        Some(json!({ "level": "all" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(
        body,
        json!({ "notify_level": "all", "effective_notify_level": "all" })
    );

    let (status, listed) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/channels"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let row = listed["data"]
        .as_array()
        .expect("data")
        .iter()
        .find(|c| c["id"] == json!(ch))
        .expect("the channel is listed");
    assert_eq!(row["notify_level"], json!("all"));
    assert_eq!(row["effective_notify_level"], json!("all"));

    let (status, body) = send(
        &app,
        "PUT",
        &route,
        Some(&token),
        Some(json!({ "level": null })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({ "notify_level": null, "effective_notify_level": "mentions" }),
        "back to the default"
    );

    let (status, _) = send(
        &app,
        "PUT",
        &route,
        Some(&token),
        Some(json!({ "level": "loud" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn only_a_member_sets_a_channel_notify_level(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner, _email, _token) = seed_and_login(&app, &state, "level-owner", false).await;
    let (outsider, _email, outsider_token) =
        seed_and_login(&app, &state, "level-outsider", false).await;
    let ws = seed_workspace(&state, owner, "Levels WS").await;
    add_ws_member(&state, ws, outsider, "member").await;
    let ch = seed_channel(&state, ws, owner, "private-ish", false).await;
    let body = Some(json!({ "level": "nothing" }));

    let (status, _) = send(
        &app,
        "PUT",
        &format!("/api/channels/{ch}/notify-level"),
        Some(&outsider_token),
        body.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = send(
        &app,
        "PUT",
        &format!("/api/channels/{ch}/notify-level"),
        None,
        body.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _) = send(
        &app,
        "PUT",
        &format!("/api/channels/{}/notify-level", uuid::Uuid::new_v4()),
        Some(&outsider_token),
        body,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = send(
        &app,
        "PATCH",
        &format!("/api/channels/{ch}/notifications"),
        Some(&outsider_token),
        Some(json!({ "muted": true })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "nor mutes a channel they are not in"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_workspace_default_is_per_member_and_drives_the_effective_level(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (uid, _email, token) = seed_and_login(&app, &state, "default-owner", false).await;
    let (outsider, _email, outsider_token) =
        seed_and_login(&app, &state, "default-outsider", false).await;
    let ws = seed_workspace(&state, uid, "Default WS").await;
    let ch = seed_channel(&state, ws, uid, "general-ish", false).await;
    let route = format!("/api/workspaces/{ws}/notify-default");
    let _ = outsider;

    let (status, body) = send(&app, "GET", &route, Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({ "notify_default": "mentions" }));

    let (status, body) = send(
        &app,
        "PUT",
        &route,
        Some(&token),
        Some(json!({ "level": "nothing" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({ "notify_default": "nothing" }));

    let (_, listed) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/channels"),
        Some(&token),
        None,
    )
    .await;
    let row = listed["data"]
        .as_array()
        .expect("data")
        .iter()
        .find(|c| c["id"] == json!(ch))
        .expect("listed");
    assert_eq!(row["notify_level"], json!(null));
    assert_eq!(row["effective_notify_level"], json!("nothing"));

    let (status, _) = send(&app, "GET", &route, Some(&outsider_token), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = send(&app, "PUT", &route, None, Some(json!({ "level": "all" }))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn keywords_are_one_list_replaced_whole(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (uid, _email, token) = seed_and_login(&app, &state, "keyword-owner", false).await;
    let ws = seed_workspace(&state, uid, "Keyword WS").await;
    let route = format!("/api/workspaces/{ws}/notification-keywords");

    let (status, body) = send(&app, "GET", &route, Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({ "keywords": [] }));

    let (status, body) = send(
        &app,
        "PUT",
        &route,
        Some(&token),
        Some(json!({ "keywords": ["deploy", " outage ", "Deploy", "Čačak", "cacak", "on call"] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(
        body,
        json!({ "keywords": ["deploy", "outage", "Čačak", "on call"] }),
        "a keyword that normalizes to one already listed is the same keyword"
    );

    let (_, body) = send(&app, "GET", &route, Some(&token), None).await;
    assert_eq!(
        body,
        json!({ "keywords": ["deploy", "outage", "Čačak", "on call"] })
    );

    let (status, body) = send(
        &app,
        "PUT",
        &route,
        Some(&token),
        Some(json!({ "keywords": ["incident"] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({ "keywords": ["incident"] }));
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_keyword_list_is_refused_whole_when_one_keyword_is_wrong(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (uid, _email, token) = seed_and_login(&app, &state, "keyword-strict", false).await;
    let ws = seed_workspace(&state, uid, "Strict WS").await;
    let route = format!("/api/workspaces/{ws}/notification-keywords");
    send(
        &app,
        "PUT",
        &route,
        Some(&token),
        Some(json!({ "keywords": ["deploy"] })),
    )
    .await;

    let too_many: Vec<String> = (0..51).map(|i| format!("word{i}")).collect();
    for keywords in [
        json!(["deploy", "a"]),
        json!(["@ana"]),
        json!(["#general"]),
        json!(["x".repeat(65)]),
        json!(too_many),
    ] {
        let (status, _) = send(
            &app,
            "PUT",
            &route,
            Some(&token),
            Some(json!({ "keywords": keywords })),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{keywords}");
    }

    let (_, body) = send(&app, "GET", &route, Some(&token), None).await;
    assert_eq!(
        body,
        json!({ "keywords": ["deploy"] }),
        "nothing was replaced"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn keywords_belong_to_a_member_of_the_workspace(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (uid, _email, token) = seed_and_login(&app, &state, "keyword-member", false).await;
    let (guest, _email, guest_token) = seed_and_login(&app, &state, "keyword-guest", false).await;
    let (_outsider, _email, outsider_token) =
        seed_and_login(&app, &state, "keyword-outsider", false).await;
    let ws = seed_workspace(&state, uid, "Members WS").await;
    add_ws_member(&state, ws, guest, "guest").await;
    let route = format!("/api/workspaces/{ws}/notification-keywords");
    let body = json!({ "keywords": ["deploy"] });

    let (status, _) = send(&app, "PUT", &route, Some(&token), Some(body.clone())).await;
    assert_eq!(status, StatusCode::OK);
    let (status, mine) = send(
        &app,
        "PUT",
        &route,
        Some(&guest_token),
        Some(json!({ "keywords": ["outage"] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "a guest keeps their own list");
    assert_eq!(mine, json!({ "keywords": ["outage"] }));
    let (_, theirs) = send(&app, "GET", &route, Some(&token), None).await;
    assert_eq!(
        theirs,
        json!({ "keywords": ["deploy"] }),
        "and it is theirs alone"
    );

    let (status, _) = send(&app, "GET", &route, Some(&outsider_token), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = send(
        &app,
        "PUT",
        &route,
        Some(&outsider_token),
        Some(body.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = send(&app, "PUT", &route, None, Some(body)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

async fn schedule_for(
    state: &crate::state::AppState,
    ws: uuid::Uuid,
    user_id: uuid::Uuid,
    timezone: &str,
    days: &[i16],
    start_minute: i16,
    end_minute: i16,
) {
    sqlx::query("UPDATE users SET timezone = $2 WHERE id = $1")
        .bind(user_id)
        .bind(timezone)
        .execute(&state.pool)
        .await
        .expect("timezone");
    state
        .notification_repo
        .set_schedule(ws, user_id, true, days, start_minute, end_minute)
        .await
        .expect("schedule");
}

fn at(instant: &str) -> chrono::DateTime<chrono::Utc> {
    instant.parse().expect("instant")
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_schedule_is_stored_and_refused_when_it_cannot_mean_anything(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (uid, _email, token) = seed_and_login(&app, &state, "schedule-owner", false).await;
    let ws = seed_workspace(&state, uid, "Schedule WS").await;
    let route = &format!("/api/workspaces/{ws}/notifications/schedule");

    let (status, body) = send(&app, "GET", route, Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({ "enabled": false, "days": [1, 2, 3, 4, 5], "start_minute": 540, "end_minute": 1080, "timezone": "UTC" })
    );

    let (status, body) = send(
        &app,
        "PUT",
        route,
        Some(&token),
        Some(json!({ "enabled": true, "days": [5, 1, 1, 3], "start_minute": 1320, "end_minute": 420 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["days"], json!([1, 3, 5]));
    assert_eq!(
        body["start_minute"],
        json!(1320),
        "a window may cross midnight"
    );

    for refused in [
        json!({ "enabled": true, "days": [1], "start_minute": 540, "end_minute": 540 }),
        json!({ "enabled": true, "days": [0], "start_minute": 540, "end_minute": 1080 }),
        json!({ "enabled": true, "days": [8], "start_minute": 540, "end_minute": 1080 }),
        json!({ "enabled": true, "days": [], "start_minute": 540, "end_minute": 1080 }),
        json!({ "enabled": true, "days": [1], "start_minute": 540, "end_minute": 1440 }),
    ] {
        let (status, _) = send(&app, "PUT", route, Some(&token), Some(refused.clone())).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{refused}");
    }
    let (_, body) = send(&app, "GET", route, Some(&token), None).await;
    assert_eq!(body["days"], json!([1, 3, 5]), "a refusal changes nothing");

    let (status, _) = send(&app, "GET", route, None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (_, _, outsider) = seed_and_login(&app, &state, "schedule-outsider", false).await;
    let (status, _) = send(&app, "GET", route, Some(&outsider), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_schedule_pauses_outside_its_window_in_the_persons_timezone(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (uid, _email, _token) = seed_and_login(&app, &state, "schedule-belgrade", false).await;
    let ws = seed_workspace(&state, uid, "Schedule WS").await;
    let repo = &state.notification_repo;
    schedule_for(
        &state,
        ws,
        uid,
        "Europe/Belgrade",
        &[1, 2, 3, 4, 5],
        540,
        1080,
    )
    .await;

    let cases = [
        ("2026-10-21T08:00:00Z", None, "Wednesday 10:00 in Belgrade"),
        (
            "2026-10-21T06:30:00Z",
            Some("2026-10-21T07:00:00Z"),
            "Wednesday 08:30, before the window",
        ),
        (
            "2026-10-21T18:00:00Z",
            Some("2026-10-22T07:00:00Z"),
            "Wednesday 20:00, after it",
        ),
        (
            "2026-10-23T17:00:00Z",
            Some("2026-10-26T08:00:00Z"),
            "Friday evening waits for Monday, after the clocks go back",
        ),
    ];
    for (now, expected, case) in cases {
        assert_eq!(
            repo.paused_until(ws, uid, at(now)).await.expect("paused"),
            expected.map(at),
            "{case}"
        );
    }

    schedule_for(
        &state,
        ws,
        uid,
        "Europe/Belgrade",
        &[1, 2, 3, 4, 5, 6, 7],
        540,
        1080,
    )
    .await;
    assert_eq!(
        repo.paused_until(ws, uid, at("2026-03-29T07:30:00Z"))
            .await
            .expect("paused"),
        None,
        "09:30 on the morning the clocks went forward is inside the window"
    );
    assert_eq!(
        repo.paused_until(ws, uid, at("2026-03-29T06:30:00Z"))
            .await
            .expect("paused"),
        Some(at("2026-03-29T07:00:00Z")),
        "and 08:30 that morning waits for 09:00 summer time"
    );

    schedule_for(
        &state,
        ws,
        uid,
        "America/New_York",
        &[1, 2, 3, 4, 5, 6, 7],
        1320,
        420,
    )
    .await;
    for (now, expected, case) in [
        (
            "2026-10-22T03:30:00Z",
            None,
            "23:30 in New York, inside a night window",
        ),
        (
            "2026-10-22T10:30:00Z",
            None,
            "06:30 the next morning, still inside",
        ),
        (
            "2026-10-22T16:00:00Z",
            Some("2026-10-23T02:00:00Z"),
            "noon waits for 22:00",
        ),
    ] {
        assert_eq!(
            repo.paused_until(ws, uid, at(now)).await.expect("paused"),
            expected.map(at),
            "{case}"
        );
    }
}

#[test_macros::db_test(migrations = "../migrations")]
async fn resuming_during_a_scheduled_pause_lasts_until_the_window_opens(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (uid, _email, _token) = seed_and_login(&app, &state, "schedule-resume", false).await;
    let ws = seed_workspace(&state, uid, "Schedule WS").await;
    let repo = &state.notification_repo;
    schedule_for(
        &state,
        ws,
        uid,
        "Europe/Belgrade",
        &[1, 2, 3, 4, 5],
        540,
        1080,
    )
    .await;
    let evening = at("2026-10-21T18:00:00Z");

    repo.set_dnd_at(ws, uid, None, evening)
        .await
        .expect("resume");
    assert_eq!(
        repo.paused_until(ws, uid, evening).await.expect("paused"),
        None
    );
    assert_eq!(
        repo.paused_until(ws, uid, at("2026-10-22T06:59:00Z"))
            .await
            .expect("paused"),
        None,
        "on through the night"
    );
    assert_eq!(
        repo.paused_until(ws, uid, at("2026-10-22T17:00:00Z"))
            .await
            .expect("paused"),
        Some(at("2026-10-23T07:00:00Z")),
        "and the schedule is back after the next window"
    );

    let timer = at("2026-10-21T19:00:00Z");
    repo.set_dnd_at(ws, uid, Some(timer), evening)
        .await
        .expect("pause");
    assert_eq!(
        repo.paused_until(ws, uid, evening).await.expect("paused"),
        Some(at("2026-10-22T07:00:00Z")),
        "a timer ends the resume, and the schedule outlasts a short timer"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_scheduled_pause_is_reported_to_its_owner_and_to_the_workspace(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (uid, _email, token) = seed_and_login(&app, &state, "schedule-report", false).await;
    let ws = seed_workspace(&state, uid, "Schedule WS").await;
    let now = chrono::Utc::now();
    let minute = |offset: i64| (((now.timestamp() / 60 + offset) % 1440 + 1440) % 1440) as i16;
    schedule_for(
        &state,
        ws,
        uid,
        "UTC",
        &[1, 2, 3, 4, 5, 6, 7],
        minute(120),
        minute(180),
    )
    .await;

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/notifications/dnd"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["dnd_until"], json!(null));
    let resumes: chrono::DateTime<chrono::Utc> = body["schedule_paused_until"]
        .as_str()
        .expect("paused by the schedule")
        .parse()
        .expect("instant");
    assert!(
        resumes > now + chrono::Duration::minutes(110)
            && resumes < now + chrono::Duration::minutes(130)
    );

    let (_, members) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/members"),
        Some(&token),
        None,
    )
    .await;
    let me = members["data"]
        .as_array()
        .expect("data")
        .iter()
        .find(|m| m["user_id"] == json!(uid))
        .expect("listed");
    assert!(me["notifications_paused_until"].is_string(), "{me}");

    let (status, body) = send(
        &app,
        "PATCH",
        &format!("/api/workspaces/{ws}/notifications/dnd"),
        Some(&token),
        Some(json!({ "dnd_until": null })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({ "dnd_until": null, "schedule_paused_until": null })
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_pause_a_schedule_and_mention_emails_belong_to_one_workspace(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (uid, _email, token) = seed_and_login(&app, &state, "per-ws", false).await;
    let here = seed_workspace(&state, uid, "Here").await;
    let there = seed_workspace(&state, uid, "There").await;
    let until = chrono::Utc::now() + chrono::Duration::hours(1);

    let (status, _) = send(
        &app,
        "PATCH",
        &format!("/api/workspaces/{here}/notifications/dnd"),
        Some(&token),
        Some(json!({ "dnd_until": until })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let paused = |ws: uuid::Uuid| {
        let (app, token) = (&app, &token);
        async move {
            let (_, members) = send(
                app,
                "GET",
                &format!("/api/workspaces/{ws}/members"),
                Some(token),
                None,
            )
            .await;
            members["data"][0]["notifications_paused_until"].clone()
        }
    };
    assert!(paused(here).await.is_string());
    assert_eq!(paused(there).await, json!(null), "the other one notifies");

    let (status, _) = send(
        &app,
        "PUT",
        &format!("/api/workspaces/{here}/notifications/schedule"),
        Some(&token),
        Some(json!({ "enabled": true, "days": [1], "start_minute": 540, "end_minute": 1080 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, body) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{there}/notifications/schedule"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(body["enabled"], json!(false));

    let (status, _) = send(
        &app,
        "PATCH",
        &format!("/api/workspaces/{here}/notifications/email"),
        Some(&token),
        Some(json!({ "mention_emails": false })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, body) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{there}/notifications/email"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(body["mention_emails"], json!(true));

    let (_, _, outsider) = seed_and_login(&app, &state, "per-ws-outsider", false).await;
    for (method, path) in [("GET", "notifications/dnd"), ("GET", "notifications/email")] {
        let (status, _) = send(
            &app,
            method,
            &format!("/api/workspaces/{here}/{path}"),
            Some(&outsider),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{path}");
    }
}
