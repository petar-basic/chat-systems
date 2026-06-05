use super::common::*;
use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

#[sqlx::test(migrations = "../migrations")]
async fn create_public_channel_succeeds_for_workspace_member(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Create Public WS").await;

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/channels"),
        Some(&token),
        Some(json!({ "name": "general-public", "channel_type": "public" })),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "create public channel: {body:?}");
    assert!(
        body["id"].is_string(),
        "response should carry channel id: {body:?}"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn create_private_channel_succeeds_for_workspace_member(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Create Private WS").await;

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/channels"),
        Some(&token),
        Some(json!({ "name": "secret-room", "channel_type": "private" })),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "create private channel: {body:?}");
    assert!(
        body["id"].is_string(),
        "response should carry channel id: {body:?}"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn create_channel_defaults_to_public_when_type_omitted(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Default Type WS").await;

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/channels"),
        Some(&token),
        Some(json!({ "name": "no-type-specified" })),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::OK,
        "create channel without type: {body:?}"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn create_channel_with_empty_name_is_rejected(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Empty Name WS").await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/channels"),
        Some(&token),
        Some(json!({ "name": "   " })),
    )
    .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrations = "../migrations")]
async fn create_channel_requires_authentication(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "No Auth Create WS").await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/channels"),
        None,
        Some(json!({ "name": "should-fail" })),
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn create_channel_forbidden_for_non_workspace_member(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Outsider Create WS").await;
    let (_, _, outsider_token) = seed_and_login(&app, &state, "ch-outsider", false).await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/channels"),
        Some(&outsider_token),
        Some(json!({ "name": "intruder-channel" })),
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../migrations")]
async fn list_channels_succeeds_for_workspace_member(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "List Channels WS").await;
    let (member_id, _, member_token) = seed_and_login(&app, &state, "ch-member", false).await;
    add_ws_member(&state, ws_id, member_id, "member").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "visible-channel", false).await;
    let (add_status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/members"),
        Some(&owner_token),
        Some(json!({ "user_id": member_id })),
    )
    .await;
    assert_eq!(add_status, StatusCode::OK, "owner adds member to channel");

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/channels"),
        Some(&member_token),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::OK, "list channels: {body:?}");
    assert!(
        body["data"].is_array(),
        "list should wrap rows in `data`: {body:?}"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn list_channels_requires_authentication(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "List No Auth WS").await;

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/channels"),
        None,
        None,
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn list_channels_forbidden_for_non_workspace_member(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "List Outsider WS").await;
    let (_, _, outsider_token) = seed_and_login(&app, &state, "ch-outsider", false).await;

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/channels"),
        Some(&outsider_token),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../migrations")]
async fn get_channel_succeeds_for_workspace_member(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Get Channel WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "get-me", false).await;

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/channels/{ch_id}"),
        Some(&token),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::OK, "get channel: {body:?}");
    assert_eq!(body["id"].as_str(), Some(ch_id.to_string().as_str()));
}

#[sqlx::test(migrations = "../migrations")]
async fn get_channel_requires_authentication(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Get No Auth WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "get-no-auth", false).await;

    let (status, _) = send(&app, "GET", &format!("/api/channels/{ch_id}"), None, None).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn get_unknown_channel_returns_not_found(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_, _, token) = seed_and_login(&app, &state, "ch-owner", false).await;
    let missing = uuid::Uuid::new_v4();

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/channels/{missing}"),
        Some(&token),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../migrations")]
async fn get_private_channel_forbidden_for_non_workspace_member(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Private Get WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "private-get", true).await;
    let (_, _, outsider_token) = seed_and_login(&app, &state, "ch-outsider", false).await;

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/channels/{ch_id}"),
        Some(&outsider_token),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../migrations")]
async fn update_channel_succeeds_for_workspace_admin(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Update Channel WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "rename-me", false).await;

    let (status, body) = send(
        &app,
        "PATCH",
        &format!("/api/channels/{ch_id}"),
        Some(&token),
        Some(json!({ "name": "renamed", "topic": "new topic", "description": "desc" })),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "update channel: {body:?}");
}

#[sqlx::test(migrations = "../migrations")]
async fn update_channel_with_empty_name_is_rejected(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Update Bad Name WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "valid-name", false).await;

    let (status, _) = send(
        &app,
        "PATCH",
        &format!("/api/channels/{ch_id}"),
        Some(&token),
        Some(json!({ "name": "  " })),
    )
    .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrations = "../migrations")]
async fn update_channel_requires_authentication(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Update No Auth WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "update-no-auth", false).await;

    let (status, _) = send(
        &app,
        "PATCH",
        &format!("/api/channels/{ch_id}"),
        None,
        Some(json!({ "name": "nope" })),
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn update_channel_forbidden_for_plain_member(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Update Member WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "member-cant-edit", false).await;
    let (member_id, _, member_token) = seed_and_login(&app, &state, "ch-member", false).await;
    add_ws_member(&state, ws_id, member_id, "member").await;

    let (status, _) = send(
        &app,
        "PATCH",
        &format!("/api/channels/{ch_id}"),
        Some(&member_token),
        Some(json!({ "name": "hacked" })),
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../migrations")]
async fn archive_channel_succeeds_for_workspace_admin(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Archive Channel WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "archive-me", false).await;

    let (status, body) = send(
        &app,
        "DELETE",
        &format!("/api/channels/{ch_id}"),
        Some(&token),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::OK, "archive channel: {body:?}");
}

#[sqlx::test(migrations = "../migrations")]
async fn archive_channel_requires_authentication(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Archive No Auth WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "archive-no-auth", false).await;

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/channels/{ch_id}"),
        None,
        None,
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn archive_channel_forbidden_for_plain_member(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Archive Member WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "member-cant-archive", false).await;
    let (member_id, _, member_token) = seed_and_login(&app, &state, "ch-member", false).await;
    add_ws_member(&state, ws_id, member_id, "member").await;

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/channels/{ch_id}"),
        Some(&member_token),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../migrations")]
async fn list_channel_members_succeeds_for_workspace_member(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "List Members WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "members-list", false).await;

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/channels/{ch_id}/members"),
        Some(&token),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::OK, "list channel members: {body:?}");
    assert!(
        body["data"].is_array(),
        "members wrapped in `data`: {body:?}"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn list_channel_members_requires_authentication(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "List Members No Auth WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "members-no-auth", false).await;

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/channels/{ch_id}/members"),
        None,
        None,
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn list_channel_members_forbidden_for_non_workspace_member(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "List Members Outsider WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "members-private", true).await;
    let (_, _, outsider_token) = seed_and_login(&app, &state, "ch-outsider", false).await;

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/channels/{ch_id}/members"),
        Some(&outsider_token),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../migrations")]
async fn add_channel_member_succeeds_for_workspace_admin(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Add Member WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "add-target", false).await;
    let (new_member_id, _) = seed(&state, "ch-newmember", false).await;
    add_ws_member(&state, ws_id, new_member_id, "member").await;

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/members"),
        Some(&token),
        Some(json!({ "user_id": new_member_id })),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "add channel member: {body:?}");
}

#[sqlx::test(migrations = "../migrations")]
async fn add_channel_member_requires_authentication(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Add Member No Auth WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "add-no-auth", false).await;
    let (target_id, _) = seed(&state, "ch-target", false).await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/members"),
        None,
        Some(json!({ "user_id": target_id })),
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn add_channel_member_forbidden_for_plain_member(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Add Member Forbidden WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "add-forbidden", false).await;
    let (member_id, _, member_token) = seed_and_login(&app, &state, "ch-member", false).await;
    add_ws_member(&state, ws_id, member_id, "member").await;
    let (target_id, _) = seed(&state, "ch-target", false).await;
    add_ws_member(&state, ws_id, target_id, "member").await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/members"),
        Some(&member_token),
        Some(json!({ "user_id": target_id })),
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../migrations")]
async fn remove_channel_member_succeeds_for_workspace_admin(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Remove Member WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "remove-target", false).await;
    let (member_id, _) = seed(&state, "ch-member", false).await;
    add_ws_member(&state, ws_id, member_id, "member").await;
    let (add_status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/members"),
        Some(&token),
        Some(json!({ "user_id": member_id })),
    )
    .await;
    assert_eq!(add_status, StatusCode::OK, "precondition: add member");

    let (status, body) = send(
        &app,
        "DELETE",
        &format!("/api/channels/{ch_id}/members/{member_id}"),
        Some(&token),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::OK, "admin removes member: {body:?}");
}

#[sqlx::test(migrations = "../migrations")]
async fn remove_self_from_channel_succeeds(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Remove Self WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "self-leave", false).await;
    let (member_id, _, member_token) = seed_and_login(&app, &state, "ch-member", false).await;
    add_ws_member(&state, ws_id, member_id, "member").await;
    let (add_status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/members"),
        Some(&owner_token),
        Some(json!({ "user_id": member_id })),
    )
    .await;
    assert_eq!(add_status, StatusCode::OK, "precondition: add member");

    let (status, body) = send(
        &app,
        "DELETE",
        &format!("/api/channels/{ch_id}/members/{member_id}"),
        Some(&member_token),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::OK, "self-removal: {body:?}");
}

#[sqlx::test(migrations = "../migrations")]
async fn remove_channel_member_requires_authentication(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Remove No Auth WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "remove-no-auth", false).await;
    let (member_id, _) = seed(&state, "ch-member", false).await;

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/channels/{ch_id}/members/{member_id}"),
        None,
        None,
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn remove_other_member_forbidden_for_plain_member(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Remove Other WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "remove-other", false).await;
    let (actor_id, _, actor_token) = seed_and_login(&app, &state, "ch-actor", false).await;
    add_ws_member(&state, ws_id, actor_id, "member").await;
    let (victim_id, _) = seed(&state, "ch-victim", false).await;
    add_ws_member(&state, ws_id, victim_id, "member").await;

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/channels/{ch_id}/members/{victim_id}"),
        Some(&actor_token),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}
