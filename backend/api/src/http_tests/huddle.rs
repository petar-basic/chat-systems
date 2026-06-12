use super::common::*;
use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

#[sqlx::test(migrations = "../migrations")]
async fn ice_servers_returns_stun_for_member(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (user_id, _, token) = seed_and_login(&app, &state, "huddle-ice", false).await;
    let ws_id = seed_workspace(&state, user_id, "huddle-ws").await;

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/ice-servers"),
        Some(&token),
        None,
    )
    .await;

    assert_eq!(
        status,
        StatusCode::OK,
        "ice-servers should succeed: {body:?}"
    );
    let servers = body["ice_servers"]
        .as_array()
        .expect("ice_servers is an array");
    assert!(
        !servers.is_empty(),
        "STUN should always be returned: {body:?}"
    );
    assert!(
        servers
            .iter()
            .any(|s| s["urls"].as_array().is_some_and(|u| u
                .iter()
                .any(|url| url.as_str().is_some_and(|s| s.starts_with("stun:"))))),
        "a STUN entry should be present: {body:?}"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn ice_servers_forbidden_for_non_member(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "huddle-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "huddle-ws").await;
    let (_outsider_id, _, outsider_token) =
        seed_and_login(&app, &state, "huddle-outsider", false).await;

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/ice-servers"),
        Some(&outsider_token),
        None,
    )
    .await;

    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "non-member must be forbidden"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn start_huddle_happy_path(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (initiator_id, _, token) = seed_and_login(&app, &state, "huddle-init", false).await;
    let ws_id = seed_workspace(&state, initiator_id, "huddle-ws").await;
    let (partner_id, _, _) = seed_and_login(&app, &state, "huddle-partner", false).await;
    add_ws_member(&state, ws_id, partner_id, "member").await;

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/huddles"),
        Some(&token),
        Some(json!({ "dm_partner_id": partner_id })),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::OK,
        "start huddle should succeed: {body:?}"
    );
    assert!(
        body["huddle_id"].is_string(),
        "response should carry a huddle_id: {body:?}"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn start_huddle_partner_not_member_forbidden(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (initiator_id, _, token) = seed_and_login(&app, &state, "huddle-init", false).await;
    let ws_id = seed_workspace(&state, initiator_id, "huddle-ws").await;
    let (outsider_id, _, _) = seed_and_login(&app, &state, "huddle-outsider", false).await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/huddles"),
        Some(&token),
        Some(json!({ "dm_partner_id": outsider_id })),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "starting a huddle with a non-member partner must be forbidden"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn start_huddle_channel_happy_path(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "huddle-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "huddle-ws").await;
    let channel_id = seed_channel(&state, ws_id, owner_id, "huddle-room", false).await;

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/huddles"),
        Some(&token),
        Some(json!({ "channel_id": channel_id })),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "channel huddle start: {body:?}");
    assert!(body["huddle_id"].is_string(), "carries huddle_id: {body:?}");
}

#[sqlx::test(migrations = "../migrations")]
async fn start_channel_huddle_posts_system_message(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "huddle-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "huddle-ws").await;
    let channel_id = seed_channel(&state, ws_id, owner_id, "huddle-room", false).await;

    let (s1, start_body) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/huddles"),
        Some(&token),
        Some(json!({ "channel_id": channel_id })),
    )
    .await;
    assert_eq!(s1, StatusCode::OK, "start: {start_body:?}");
    let huddle_id = start_body["huddle_id"].as_str().unwrap();

    let (s2, msgs) = send(
        &app,
        "GET",
        &format!("/api/channels/{channel_id}/messages"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(s2, StatusCode::OK, "list messages: {msgs:?}");
    let data = msgs["data"].as_array().expect("data array");
    let system = data
        .iter()
        .find(|m| m["metadata"]["kind"].as_str() == Some("huddle_started"))
        .expect("a huddle_started system message should be posted to the channel");
    assert_eq!(
        system["metadata"]["huddle_id"].as_str(),
        Some(huddle_id),
        "system message should reference the started huddle"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn start_huddle_channel_non_member_forbidden(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "huddle-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "huddle-ws").await;
    let channel_id = seed_channel(&state, ws_id, owner_id, "huddle-room", false).await;
    let (_outsider_id, _, outsider_token) =
        seed_and_login(&app, &state, "huddle-outsider", false).await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/huddles"),
        Some(&outsider_token),
        Some(json!({ "channel_id": channel_id })),
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN, "non-member cannot start");
}

#[sqlx::test(migrations = "../migrations")]
async fn start_huddle_rejects_ambiguous_or_missing_scope(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "huddle-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "huddle-ws").await;
    let channel_id = seed_channel(&state, ws_id, owner_id, "huddle-room", false).await;
    let (partner_id, _, _) = seed_and_login(&app, &state, "huddle-partner", false).await;
    add_ws_member(&state, ws_id, partner_id, "member").await;

    let uri = format!("/api/workspaces/{ws_id}/huddles");

    let (no_scope, _) = send(&app, "POST", &uri, Some(&token), Some(json!({}))).await;
    assert_eq!(no_scope, StatusCode::BAD_REQUEST, "missing scope must 400");

    let (both, _) = send(
        &app,
        "POST",
        &uri,
        Some(&token),
        Some(json!({ "channel_id": channel_id, "dm_partner_id": partner_id })),
    )
    .await;
    assert_eq!(both, StatusCode::BAD_REQUEST, "ambiguous scope must 400");
}

#[sqlx::test(migrations = "../migrations")]
async fn invite_to_huddle_member_ok(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "huddle-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "huddle-ws").await;
    let (partner_id, _, _) = seed_and_login(&app, &state, "huddle-partner", false).await;
    add_ws_member(&state, ws_id, partner_id, "member").await;
    let huddle_id = uuid::Uuid::new_v4();

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/huddles/{huddle_id}/invite"),
        Some(&token),
        Some(json!({ "user_ids": [partner_id] })),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "invite should succeed: {body:?}");
}

#[sqlx::test(migrations = "../migrations")]
async fn invite_to_huddle_non_member_forbidden(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "huddle-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "huddle-ws").await;
    let (target_id, _, _) = seed_and_login(&app, &state, "huddle-target", false).await;
    let (_outsider_id, _, outsider_token) =
        seed_and_login(&app, &state, "huddle-outsider", false).await;
    let huddle_id = uuid::Uuid::new_v4();

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/huddles/{huddle_id}/invite"),
        Some(&outsider_token),
        Some(json!({ "user_ids": [target_id] })),
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN, "non-member cannot invite");
}

#[sqlx::test(migrations = "../migrations")]
async fn huddle_history_lifecycle_tracks_participants_and_ends(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "huddle-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "huddle-ws").await;
    let (partner_id, _, _) = seed_and_login(&app, &state, "huddle-partner", false).await;
    add_ws_member(&state, ws_id, partner_id, "member").await;

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/huddles"),
        Some(&token),
        Some(json!({ "dm_partner_id": partner_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "start: {body:?}");
    let huddle_id: uuid::Uuid = body["huddle_id"].as_str().unwrap().parse().unwrap();

    state
        .huddle_repo
        .record_join(huddle_id, owner_id)
        .await
        .unwrap();
    state
        .huddle_repo
        .record_join(huddle_id, partner_id)
        .await
        .unwrap();

    let after_owner_left = state
        .huddle_repo
        .record_leave(huddle_id, owner_id)
        .await
        .unwrap();
    assert_eq!(after_owner_left, 1, "one participant should remain active");

    let after_partner_left = state
        .huddle_repo
        .record_leave(huddle_id, partner_id)
        .await
        .unwrap();
    assert_eq!(
        after_partner_left, 0,
        "no participants should remain active"
    );

    let ended = state.huddle_repo.end_session(huddle_id).await.unwrap();
    let ended = ended.expect("end_session returns the session on first end");
    assert!(ended.ended_at.is_some(), "ended_at should be set");
    assert_eq!(ended.dm_partner_id, Some(partner_id), "scope preserved");

    let second_end = state.huddle_repo.end_session(huddle_id).await.unwrap();
    assert!(
        second_end.is_none(),
        "ending an already-ended session is a no-op"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn active_huddles_lists_only_live_channel_huddles(pool: PgPool) {
    use redis::AsyncCommands;

    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "huddle-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "huddle-ws").await;
    let channel_id = seed_channel(&state, ws_id, owner_id, "huddle-room", false).await;

    let (_s0, body) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/huddles"),
        Some(&token),
        Some(json!({ "channel_id": channel_id })),
    )
    .await;
    let huddle_id = body["huddle_id"].as_str().unwrap().to_string();

    let (s1, empty) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/active-huddles"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(s1, StatusCode::OK, "{empty:?}");
    assert_eq!(
        empty["data"].as_array().map(std::vec::Vec::len),
        Some(0),
        "a session with no live members must not be listed: {empty:?}"
    );

    let key = format!("huddle:{huddle_id}:members");
    let mut conn = state.redis.clone();
    let _: i64 = conn.sadd(&key, owner_id.to_string()).await.unwrap();

    let (s2, listed) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/active-huddles"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(s2, StatusCode::OK, "{listed:?}");
    let data = listed["data"].as_array().expect("data array");
    assert!(
        data.iter()
            .any(|h| h["huddle_id"].as_str() == Some(huddle_id.as_str())
                && h["channel_id"].as_str() == Some(channel_id.to_string().as_str())),
        "a channel huddle with a live member must be listed: {listed:?}"
    );

    let _: i64 = conn.del(&key).await.unwrap();
}
