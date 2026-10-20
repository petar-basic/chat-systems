use super::common::*;
use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

use uuid::Uuid;

use crate::workspace::models::ChannelRole;

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
async fn get_channel_requires_authentication(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Get No Auth WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "get-no-auth", false).await;

    let (status, _) = send(&app, "GET", &format!("/api/channels/{ch_id}"), None, None).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
async fn add_channel_member_forbidden_for_non_workspace_member(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Add Member Forbidden WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "add-forbidden", false).await;
    let (_, _, outsider_token) = seed_and_login(&app, &state, "ch-outsider", false).await;
    let (target_id, _) = seed(&state, "ch-target", false).await;
    add_ws_member(&state, ws_id, target_id, "member").await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/members"),
        Some(&outsider_token),
        Some(json!({ "user_id": target_id })),
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
async fn browse_lists_public_channels_with_membership_and_counts(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Browse WS").await;
    let joined = seed_channel(&state, ws_id, owner_id, "joined-room", false).await;
    let open = seed_channel(&state, ws_id, owner_id, "open-room", false).await;
    seed_channel(&state, ws_id, owner_id, "secret-room", true).await;

    let (browser_id, _, browser_token) = seed_and_login(&app, &state, "ch-browser", false).await;
    add_ws_member(&state, ws_id, browser_id, "member").await;
    state
        .workspace_service
        .repo
        .add_channel_member(joined, browser_id, &ChannelRole::Member)
        .await
        .expect("precondition: join one channel");

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/channels/browse"),
        Some(&browser_token),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::OK, "browse: {body:?}");
    let channels = body["data"].as_array().expect("data array");
    let names: Vec<&str> = channels
        .iter()
        .map(|c| c["name"].as_str().unwrap_or_default())
        .collect();
    assert!(
        !names.contains(&"secret-room"),
        "private channels must stay hidden: {names:?}"
    );

    let joined_row = channels
        .iter()
        .find(|c| c["id"].as_str() == Some(&joined.to_string()))
        .expect("joined channel listed");
    assert_eq!(joined_row["is_member"], true);
    assert_eq!(joined_row["member_count"], 2);

    let open_row = channels
        .iter()
        .find(|c| c["id"].as_str() == Some(&open.to_string()))
        .expect("unjoined public channel listed");
    assert_eq!(open_row["is_member"], false);
    assert_eq!(open_row["member_count"], 1);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn browse_is_forbidden_for_non_members_and_guests(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Browse Guard WS").await;
    seed_channel(&state, ws_id, owner_id, "open-room", false).await;

    let (_, _, outsider_token) = seed_and_login(&app, &state, "ch-outsider", false).await;
    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/channels/browse"),
        Some(&outsider_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "outsider must not browse");

    let (guest_id, _, guest_token) = seed_and_login(&app, &state, "ch-guest", false).await;
    add_ws_member(&state, ws_id, guest_id, "guest").await;
    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/channels/browse"),
        Some(&guest_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "guest must not browse");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn member_can_join_a_public_channel_and_repeat_joins_are_idempotent(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Join WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "open-room", false).await;
    let (joiner_id, _, joiner_token) = seed_and_login(&app, &state, "ch-joiner", false).await;
    add_ws_member(&state, ws_id, joiner_id, "member").await;

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/join"),
        Some(&joiner_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "join: {body:?}");
    assert_eq!(body["user_id"], joiner_id.to_string());
    assert_eq!(body["role"], "member");

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/join"),
        Some(&joiner_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "joining twice should be a no-op");

    let (status, listing) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/channels"),
        Some(&joiner_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let ids: Vec<&str> = listing["data"]
        .as_array()
        .expect("channels array")
        .iter()
        .map(|c| c["id"].as_str().unwrap_or_default())
        .collect();
    assert!(
        ids.contains(&ch_id.to_string().as_str()),
        "joined channel should appear in the sidebar listing: {ids:?}"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn join_rejects_private_channels_guests_and_outsiders(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Join Guard WS").await;
    let private = seed_channel(&state, ws_id, owner_id, "secret-room", true).await;
    let public = seed_channel(&state, ws_id, owner_id, "open-room", false).await;

    let (member_id, _, member_token) = seed_and_login(&app, &state, "ch-member", false).await;
    add_ws_member(&state, ws_id, member_id, "member").await;
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{private}/join"),
        Some(&member_token),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "private channels are invite-only"
    );

    let (guest_id, _, guest_token) = seed_and_login(&app, &state, "ch-guest", false).await;
    add_ws_member(&state, ws_id, guest_id, "guest").await;
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{public}/join"),
        Some(&guest_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "guests cannot self-join");

    let (_, _, outsider_token) = seed_and_login(&app, &state, "ch-outsider", false).await;
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{public}/join"),
        Some(&outsider_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "outsiders cannot self-join");

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{public}/join"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn join_rejects_archived_channels(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Join Archived WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "old-room", false).await;
    state
        .workspace_service
        .repo
        .archive_channel(ch_id)
        .await
        .expect("precondition: archive");

    let (joiner_id, _, joiner_token) = seed_and_login(&app, &state, "ch-joiner", false).await;
    add_ws_member(&state, ws_id, joiner_id, "member").await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/join"),
        Some(&joiner_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn channel_admin_can_rename_and_archive_their_own_channel(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Channel Admin WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "product", false).await;

    let (manager_id, _, manager_token) = seed_and_login(&app, &state, "ch-manager", false).await;
    add_ws_member(&state, ws_id, manager_id, "member").await;
    state
        .workspace_service
        .repo
        .add_channel_member(ch_id, manager_id, &ChannelRole::Admin)
        .await
        .expect("precondition: channel admin");

    let (status, body) = send(
        &app,
        "PATCH",
        &format!("/api/channels/{ch_id}"),
        Some(&manager_token),
        Some(json!({ "name": "product-renamed", "topic": "roadmap talk" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "channel admin renames: {body:?}");
    assert_eq!(body["name"], "product-renamed");
    assert_eq!(body["topic"], "roadmap talk");

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/channels/{ch_id}"),
        Some(&manager_token),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "channel admin archives their channel"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn plain_member_still_cannot_rename_or_archive_a_channel(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Member Guard WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "product", false).await;

    let (member_id, _, member_token) = seed_and_login(&app, &state, "ch-member", false).await;
    add_ws_member(&state, ws_id, member_id, "member").await;
    state
        .workspace_service
        .repo
        .add_channel_member(ch_id, member_id, &ChannelRole::Member)
        .await
        .expect("precondition: plain channel member");

    let (status, _) = send(
        &app,
        "PATCH",
        &format!("/api/channels/{ch_id}"),
        Some(&member_token),
        Some(json!({ "name": "hijacked" })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "plain member must not rename"
    );

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/channels/{ch_id}"),
        Some(&member_token),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "plain member must not archive"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn channel_admin_can_promote_and_demote_channel_members(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Promote WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "product", false).await;

    let (member_id, _, member_token) = seed_and_login(&app, &state, "ch-member", false).await;
    add_ws_member(&state, ws_id, member_id, "member").await;
    state
        .workspace_service
        .repo
        .add_channel_member(ch_id, member_id, &ChannelRole::Member)
        .await
        .expect("precondition: channel member");

    let (status, _) = send(
        &app,
        "PATCH",
        &format!("/api/channels/{ch_id}/members/{owner_id}/role"),
        Some(&member_token),
        Some(json!({ "role": "admin" })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a plain channel member cannot hand out roles"
    );

    let (status, body) = send(
        &app,
        "PATCH",
        &format!("/api/channels/{ch_id}/members/{member_id}/role"),
        Some(&owner_token),
        Some(json!({ "role": "admin" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "promote: {body:?}");
    assert_eq!(body["role"], "admin");

    let (status, body) = send(
        &app,
        "PATCH",
        &format!("/api/channels/{ch_id}/members/{owner_id}/role"),
        Some(&member_token),
        Some(json!({ "role": "member" })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the promoted admin can demote: {body:?}"
    );
    assert_eq!(body["role"], "member");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn changing_the_role_of_a_non_member_is_not_found(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Role NotFound WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "product", false).await;
    let (outsider_id, _) = seed(&state, "ch-outsider", false).await;
    add_ws_member(&state, ws_id, outsider_id, "member").await;

    let (status, _) = send(
        &app,
        "PATCH",
        &format!("/api/channels/{ch_id}/members/{outsider_id}/role"),
        Some(&owner_token),
        Some(json!({ "role": "admin" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn any_workspace_member_can_add_people_to_a_public_channel(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Public Add WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "open-room", false).await;

    let (actor_id, _, actor_token) = seed_and_login(&app, &state, "ch-actor", false).await;
    add_ws_member(&state, ws_id, actor_id, "member").await;
    let (invitee_id, _) = seed(&state, "ch-invitee", false).await;
    add_ws_member(&state, ws_id, invitee_id, "member").await;

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/members"),
        Some(&actor_token),
        Some(json!({ "user_id": invitee_id })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "member adds to public channel: {body:?}"
    );
    assert_eq!(body["user_id"], invitee_id.to_string());
}

#[test_macros::db_test(migrations = "../migrations")]
async fn adding_to_a_private_channel_requires_belonging_to_it(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Private Add WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "secret-room", true).await;

    let (outsider_id, _, outsider_token) = seed_and_login(&app, &state, "ch-outsider", false).await;
    add_ws_member(&state, ws_id, outsider_id, "member").await;
    let (invitee_id, _) = seed(&state, "ch-invitee", false).await;
    add_ws_member(&state, ws_id, invitee_id, "member").await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/members"),
        Some(&outsider_token),
        Some(json!({ "user_id": invitee_id })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "someone outside a private channel cannot add people to it"
    );

    state
        .workspace_service
        .repo
        .add_channel_member(ch_id, outsider_id, &ChannelRole::Member)
        .await
        .expect("join the private channel");

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/members"),
        Some(&outsider_token),
        Some(json!({ "user_id": invitee_id })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a private-channel member may invite: {body:?}"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn guests_cannot_add_channel_members_and_outsiders_cannot_be_added(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Add Guard WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "open-room", false).await;

    let (guest_id, _, guest_token) = seed_and_login(&app, &state, "ch-guest", false).await;
    add_ws_member(&state, ws_id, guest_id, "guest").await;
    state
        .workspace_service
        .repo
        .add_channel_member(ch_id, guest_id, &ChannelRole::Member)
        .await
        .expect("precondition: guest belongs to the channel");
    let (invitee_id, _) = seed(&state, "ch-invitee", false).await;
    add_ws_member(&state, ws_id, invitee_id, "member").await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/members"),
        Some(&guest_token),
        Some(json!({ "user_id": invitee_id })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "guests cannot add people");

    let (stranger_id, _) = seed(&state, "ch-stranger", false).await;
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/members"),
        Some(&owner_token),
        Some(json!({ "user_id": stranger_id })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "someone outside the workspace cannot be added to a channel"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn channel_admin_can_remove_another_member(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Remove By Channel Admin WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "product", false).await;

    let (manager_id, _, manager_token) = seed_and_login(&app, &state, "ch-manager", false).await;
    add_ws_member(&state, ws_id, manager_id, "member").await;
    state
        .workspace_service
        .repo
        .add_channel_member(ch_id, manager_id, &ChannelRole::Admin)
        .await
        .expect("precondition: channel admin");
    let (victim_id, _) = seed(&state, "ch-victim", false).await;
    add_ws_member(&state, ws_id, victim_id, "member").await;
    state
        .workspace_service
        .repo
        .add_channel_member(ch_id, victim_id, &ChannelRole::Member)
        .await
        .expect("precondition: channel member");

    let (status, body) = send(
        &app,
        "DELETE",
        &format!("/api/channels/{ch_id}/members/{victim_id}"),
        Some(&manager_token),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "channel admin removes a member: {body:?}"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_guest_channel_admin_still_cannot_moderate(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "ch-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Guest Moderate WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "product", false).await;

    let (guest_id, _, guest_token) = seed_and_login(&app, &state, "ch-guest", false).await;
    add_ws_member(&state, ws_id, guest_id, "guest").await;
    state
        .workspace_service
        .repo
        .add_channel_member(ch_id, guest_id, &ChannelRole::Admin)
        .await
        .expect("precondition: guest carries the channel admin role");

    let (status, _) = send(
        &app,
        "PATCH",
        &format!("/api/channels/{ch_id}"),
        Some(&guest_token),
        Some(json!({ "name": "guest-renamed" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = send(
        &app,
        "PATCH",
        &format!("/api/channels/{ch_id}/members/{owner_id}/role"),
        Some(&guest_token),
        Some(json!({ "role": "member" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

async fn counters(state: &crate::state::AppState, channel_id: Uuid, user_id: Uuid) -> (i32, i32) {
    sqlx::query_as(
        "SELECT unread_count, mention_count FROM channel_members \
          WHERE channel_id = $1 AND user_id = $2",
    )
    .bind(channel_id)
    .bind(user_id)
    .fetch_one(&state.pool)
    .await
    .expect("counters")
}

/// The definition the denormalised counter replaced. Kept as a test-only helper
/// so the two can be proven equivalent before the subquery is trusted to be gone.
async fn unread_by_subquery(
    state: &crate::state::AppState,
    channel_id: Uuid,
    user_id: Uuid,
) -> i64 {
    sqlx::query_scalar(
        r"
        SELECT COUNT(*)
          FROM messages m
          JOIN channel_members cm
            ON cm.channel_id = m.channel_id AND cm.user_id = $2
         WHERE m.channel_id = $1
           AND m.deleted_at IS NULL
           AND m.user_id <> $2
           AND (cm.last_read_at IS NULL OR m.created_at > cm.last_read_at)
        ",
    )
    .bind(channel_id)
    .bind(user_id)
    .fetch_one(&state.pool)
    .await
    .expect("subquery count")
}

#[test_macros::db_test(migrations = "../migrations")]
async fn sending_counts_for_everyone_but_the_author(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (author_id, _, author) = seed_and_login(&app, &state, "unread-author", false).await;
    let ws = seed_workspace(&state, author_id, "Unread WS").await;
    let ch = seed_channel(&state, ws, author_id, "main", false).await;

    let (reader_id, _) = seed(&state, "unread-reader", false).await;
    add_ws_member(&state, ws, reader_id, "member").await;
    state
        .workspace_service
        .repo
        .add_channel_member(ch, reader_id, &ChannelRole::Member)
        .await
        .expect("join");

    for i in 0..3 {
        let (status, _) = send(
            &app,
            "POST",
            &format!("/api/channels/{ch}/messages"),
            Some(&author),
            Some(json!({ "content": format!("message {i}") })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }

    assert_eq!(counters(&state, ch, reader_id).await.0, 3);
    assert_eq!(
        counters(&state, ch, author_id).await.0,
        0,
        "your own message is not unread for you"
    );
    assert_eq!(
        unread_by_subquery(&state, ch, reader_id).await,
        3,
        "the denormalised counter agrees with the definition it replaced"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_mention_counts_separately_from_the_unread_total(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (author_id, _, author) = seed_and_login(&app, &state, "unread-author", false).await;
    let ws = seed_workspace(&state, author_id, "Unread WS").await;
    let ch = seed_channel(&state, ws, author_id, "main", false).await;

    let (reader_id, _) = seed(&state, "unread-reader", false).await;
    add_ws_member(&state, ws, reader_id, "member").await;
    state
        .workspace_service
        .repo
        .add_channel_member(ch, reader_id, &ChannelRole::Member)
        .await
        .expect("join");

    for content in ["plain one", "plain two"] {
        send(
            &app,
            "POST",
            &format!("/api/channels/{ch}/messages"),
            Some(&author),
            Some(json!({ "content": content })),
        )
        .await;
    }
    send(
        &app,
        "POST",
        &format!("/api/channels/{ch}/messages"),
        Some(&author),
        Some(json!({ "content": format!("hey @[Reader]({reader_id})") })),
    )
    .await;

    let (unread, mentions) = counters(&state, ch, reader_id).await;
    assert_eq!(unread, 3, "every message counts as unread");
    assert_eq!(mentions, 1, "only the mention counts as a mention");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn marking_read_clears_both_counters(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (author_id, _, author) = seed_and_login(&app, &state, "unread-author", false).await;
    let ws = seed_workspace(&state, author_id, "Unread WS").await;
    let ch = seed_channel(&state, ws, author_id, "main", false).await;

    let (reader_id, reader_email) = seed(&state, "unread-reader", false).await;
    add_ws_member(&state, ws, reader_id, "member").await;
    state
        .workspace_service
        .repo
        .add_channel_member(ch, reader_id, &ChannelRole::Member)
        .await
        .expect("join");
    let reader = login(&app, &reader_email, PASSWORD).await;

    let (_, msg) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch}/messages"),
        Some(&author),
        Some(json!({ "content": format!("hey @[Reader]({reader_id})") })),
    )
    .await;
    let msg_id = msg["id"].as_str().expect("id");

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch}/read"),
        Some(&reader),
        Some(json!({ "message_id": msg_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(counters(&state, ch, reader_id).await, (0, 0));
}

#[test_macros::db_test(migrations = "../migrations")]
async fn deleting_an_unread_message_takes_it_off_the_count(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (author_id, _, author) = seed_and_login(&app, &state, "unread-author", false).await;
    let ws = seed_workspace(&state, author_id, "Unread WS").await;
    let ch = seed_channel(&state, ws, author_id, "main", false).await;

    let (reader_id, _) = seed(&state, "unread-reader", false).await;
    add_ws_member(&state, ws, reader_id, "member").await;
    state
        .workspace_service
        .repo
        .add_channel_member(ch, reader_id, &ChannelRole::Member)
        .await
        .expect("join");

    let (_, msg) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch}/messages"),
        Some(&author),
        Some(json!({ "content": "regrettable" })),
    )
    .await;
    let msg_id = msg["id"].as_str().expect("id");
    assert_eq!(counters(&state, ch, reader_id).await.0, 1);

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/messages/{msg_id}"),
        Some(&author),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    assert_eq!(
        counters(&state, ch, reader_id).await.0,
        0,
        "a deleted message stops being unread"
    );
    assert_eq!(unread_by_subquery(&state, ch, reader_id).await, 0);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn muting_a_channel_does_not_change_its_unread_count(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (author_id, _, author) = seed_and_login(&app, &state, "unread-author", false).await;
    let ws = seed_workspace(&state, author_id, "Unread WS").await;
    let ch = seed_channel(&state, ws, author_id, "main", false).await;

    let (reader_id, reader_email) = seed(&state, "unread-reader", false).await;
    add_ws_member(&state, ws, reader_id, "member").await;
    state
        .workspace_service
        .repo
        .add_channel_member(ch, reader_id, &ChannelRole::Member)
        .await
        .expect("join");
    let reader = login(&app, &reader_email, PASSWORD).await;

    let (status, _) = send(
        &app,
        "PATCH",
        &format!("/api/channels/{ch}/notifications"),
        Some(&reader),
        Some(json!({ "muted": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    send(
        &app,
        "POST",
        &format!("/api/channels/{ch}/messages"),
        Some(&author),
        Some(json!({ "content": "still unread" })),
    )
    .await;

    assert_eq!(
        counters(&state, ch, reader_id).await.0,
        1,
        "muting decides whether you are notified, not whether you have read it"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_reconciler_corrects_a_drifted_counter(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (author_id, _, author) = seed_and_login(&app, &state, "unread-author", false).await;
    let ws = seed_workspace(&state, author_id, "Unread WS").await;
    let ch = seed_channel(&state, ws, author_id, "main", false).await;

    let (reader_id, _) = seed(&state, "unread-reader", false).await;
    add_ws_member(&state, ws, reader_id, "member").await;
    state
        .workspace_service
        .repo
        .add_channel_member(ch, reader_id, &ChannelRole::Member)
        .await
        .expect("join");

    send(
        &app,
        "POST",
        &format!("/api/channels/{ch}/messages"),
        Some(&author),
        Some(json!({ "content": "one" })),
    )
    .await;

    sqlx::query(
        "UPDATE channel_members SET unread_count = 99 WHERE channel_id = $1 AND user_id = $2",
    )
    .bind(ch)
    .bind(reader_id)
    .execute(&state.pool)
    .await
    .expect("induce drift");

    let corrected = state
        .message_repo
        .reconcile_unread_counts(24)
        .await
        .expect("reconcile");
    assert_eq!(
        corrected, 1,
        "the drifted row is reported, not silently fixed"
    );
    assert_eq!(counters(&state, ch, reader_id).await.0, 1);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_unread_endpoint_reports_counts(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (author_id, _, author) = seed_and_login(&app, &state, "unread-author", false).await;
    let ws = seed_workspace(&state, author_id, "Unread WS").await;
    let ch = seed_channel(&state, ws, author_id, "main", false).await;

    let (reader_id, reader_email) = seed(&state, "unread-reader", false).await;
    add_ws_member(&state, ws, reader_id, "member").await;
    state
        .workspace_service
        .repo
        .add_channel_member(ch, reader_id, &ChannelRole::Member)
        .await
        .expect("join");
    let reader = login(&app, &reader_email, PASSWORD).await;

    send(
        &app,
        "POST",
        &format!("/api/channels/{ch}/messages"),
        Some(&author),
        Some(json!({ "content": format!("hi @[Reader]({reader_id})") })),
    )
    .await;

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws}/channels/unread"),
        Some(&reader),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let row = body["counts"]
        .as_array()
        .expect("counts array")
        .iter()
        .find(|c| c["channel_id"] == ch.to_string())
        .expect("the channel is listed");
    assert_eq!(row["unread_count"], 1);
    assert_eq!(row["mention_count"], 1);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_moderator_puts_a_bookmark_on_the_channel(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "bm-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Bookmarks WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "main", false).await;

    let (member_id, _, member_token) = seed_and_login(&app, &state, "bm-member", false).await;
    add_ws_member(&state, ws_id, member_id, "member").await;

    let (status, created) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/bookmarks"),
        Some(&owner_token),
        Some(json!({ "label": "Runbook", "url": "https://example.test/runbook", "emoji": "📕" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created:?}");
    let bookmark_id = created["id"].as_str().expect("id").to_string();

    let (status, listing) = send(
        &app,
        "GET",
        &format!("/api/channels/{ch_id}/bookmarks"),
        Some(&member_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "a member reads the bar");
    assert_eq!(listing["data"].as_array().expect("array").len(), 1);

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/bookmarks"),
        Some(&member_token),
        Some(json!({ "label": "Mine", "url": "https://example.test/mine" })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "the bar is shared, so pinning to it is a moderator's job"
    );

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/channels/{ch_id}/bookmarks/{bookmark_id}"),
        Some(&member_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/channels/{ch_id}/bookmarks/{bookmark_id}"),
        Some(&owner_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (_, listing) = send(
        &app,
        "GET",
        &format!("/api/channels/{ch_id}/bookmarks"),
        Some(&owner_token),
        None,
    )
    .await;
    assert!(listing["data"].as_array().expect("array").is_empty());
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_bookmark_has_to_be_a_link_somebody_can_safely_click(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "bm-scheme", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Bookmarks WS").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "main", false).await;

    for url in [
        "javascript:alert(1)",
        "data:text/html,<script>alert(1)</script>",
        "/etc/passwd",
    ] {
        let (status, _) = send(
            &app,
            "POST",
            &format!("/api/channels/{ch_id}/bookmarks"),
            Some(&owner_token),
            Some(json!({ "label": "Nope", "url": url })),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{url} must be refused"
        );
    }

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/bookmarks"),
        Some(&owner_token),
        Some(json!({ "label": "   ", "url": "https://example.test" })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "a bookmark needs a label"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn bookmarks_stay_inside_the_channel_that_owns_them(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "bm-scope", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Bookmarks WS").await;
    let first = seed_channel(&state, ws_id, owner_id, "main", false).await;
    let second = seed_channel(&state, ws_id, owner_id, "random", false).await;

    let (_, created) = send(
        &app,
        "POST",
        &format!("/api/channels/{first}/bookmarks"),
        Some(&owner_token),
        Some(json!({ "label": "Runbook", "url": "https://example.test/runbook" })),
    )
    .await;
    let bookmark_id = created["id"].as_str().expect("id").to_string();

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/channels/{second}/bookmarks/{bookmark_id}"),
        Some(&owner_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

async fn set_post_policy(app: &axum::Router, token: &str, ch_id: Uuid, policy: &str) -> StatusCode {
    let (status, _) = send(
        app,
        "PATCH",
        &format!("/api/channels/{ch_id}"),
        Some(token),
        Some(json!({ "post_policy": policy })),
    )
    .await;
    status
}

/// The writers are the part that gets missed: the composer, thread replies,
/// scheduled sends and slash commands are four different files, and each of
/// them is a way into a channel that is supposed to be read-only.
#[sqlx::test(migrations = "../migrations")]
async fn an_announcement_channel_refuses_every_way_a_member_could_post(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "ann-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Announcements").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "releases", false).await;

    let (member_id, _, member_token) = seed_and_login(&app, &state, "ann-member", false).await;
    add_ws_member(&state, ws_id, member_id, "member").await;
    state
        .workspace_service
        .repo
        .add_channel_member(ch_id, member_id, &ChannelRole::Member)
        .await
        .expect("add the member to the channel");

    let (status, parent) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/messages"),
        Some(&member_token),
        Some(json!({ "content": "before the lock" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{parent:?}");
    let parent_id = parent["id"].as_str().expect("message id").to_string();

    assert_eq!(
        set_post_policy(&app, &owner_token, ch_id, "moderators").await,
        StatusCode::OK
    );

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/messages"),
        Some(&member_token),
        Some(json!({ "content": "after the lock" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "the composer");

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/messages"),
        Some(&member_token),
        Some(json!({ "content": "a reply is a post too", "thread_parent_id": parent_id })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "thread replies");

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/scheduled-messages"),
        Some(&member_token),
        Some(json!({
            "channel_id": ch_id,
            "content": "later",
            "send_at": (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339(),
        })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "scheduled sends");

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/commands"),
        Some(&member_token),
        Some(json!({ "command": "shrug", "text": "hello" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "the command still runs: {body:?}");
    assert_eq!(
        body["response_type"], "ephemeral",
        "but its answer is not posted into a channel they may not write to"
    );

    let posted: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE channel_id = $1")
        .bind(ch_id)
        .fetch_one(&state.pool)
        .await
        .expect("count");
    assert_eq!(posted, 1, "only the message from before the lock");
}

#[sqlx::test(migrations = "../migrations")]
async fn reading_and_reacting_are_untouched_by_the_policy(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "ann-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Announcements").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "releases", false).await;
    let (member_id, _, member_token) = seed_and_login(&app, &state, "ann-member", false).await;
    add_ws_member(&state, ws_id, member_id, "member").await;
    state
        .workspace_service
        .repo
        .add_channel_member(ch_id, member_id, &ChannelRole::Member)
        .await
        .expect("add the member");

    let (_, posted) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/messages"),
        Some(&owner_token),
        Some(json!({ "content": "release 2.0 is out" })),
    )
    .await;
    let msg_id = posted["id"].as_str().expect("message id");
    assert_eq!(
        set_post_policy(&app, &owner_token, ch_id, "moderators").await,
        StatusCode::OK
    );

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/channels/{ch_id}/messages"),
        Some(&member_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "reading is unaffected");

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/messages/{msg_id}/reactions"),
        Some(&member_token),
        Some(json!({ "emoji": "🎉" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "a reaction is not a post");
}

#[sqlx::test(migrations = "../migrations")]
async fn moderators_post_and_only_moderators_change_the_policy(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "ann-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Announcements").await;
    let ch_id = seed_channel(&state, ws_id, owner_id, "releases", false).await;
    let (member_id, _, member_token) = seed_and_login(&app, &state, "ann-member", false).await;
    add_ws_member(&state, ws_id, member_id, "member").await;
    state
        .workspace_service
        .repo
        .add_channel_member(ch_id, member_id, &ChannelRole::Member)
        .await
        .expect("add the member");

    assert_eq!(
        set_post_policy(&app, &member_token, ch_id, "moderators").await,
        StatusCode::FORBIDDEN,
        "a plain member cannot silence a channel"
    );
    assert_eq!(
        set_post_policy(&app, &owner_token, ch_id, "moderators").await,
        StatusCode::OK
    );
    assert_eq!(
        set_post_policy(&app, &owner_token, ch_id, "nonsense").await,
        StatusCode::UNPROCESSABLE_ENTITY,
        "the policy is a closed set"
    );

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/messages"),
        Some(&owner_token),
        Some(json!({ "content": "still allowed" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "the people who moderate can speak");

    let audited: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE action = 'channel.post_policy_changed' \
         AND details->>'from' = 'everyone' AND details->>'to' = 'moderators'",
    )
    .fetch_one(&state.pool)
    .await
    .expect("count audit");
    assert_eq!(audited, 1, "with before and after");

    assert_eq!(
        set_post_policy(&app, &owner_token, ch_id, "everyone").await,
        StatusCode::OK
    );
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/channels/{ch_id}/messages"),
        Some(&member_token),
        Some(json!({ "content": "unlocked again" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "and it is reversible");
}
