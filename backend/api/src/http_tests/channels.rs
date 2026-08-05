use super::common::*;
use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

use crate::workspace::models::ChannelRole;

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

#[sqlx::test(migrations = "../migrations")]
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

#[sqlx::test(migrations = "../migrations")]
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

#[sqlx::test(migrations = "../migrations")]
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

#[sqlx::test(migrations = "../migrations")]
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

#[sqlx::test(migrations = "../migrations")]
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

#[sqlx::test(migrations = "../migrations")]
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

#[sqlx::test(migrations = "../migrations")]
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

#[sqlx::test(migrations = "../migrations")]
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

#[sqlx::test(migrations = "../migrations")]
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

#[sqlx::test(migrations = "../migrations")]
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

#[sqlx::test(migrations = "../migrations")]
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

#[sqlx::test(migrations = "../migrations")]
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

#[sqlx::test(migrations = "../migrations")]
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

#[sqlx::test(migrations = "../migrations")]
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
