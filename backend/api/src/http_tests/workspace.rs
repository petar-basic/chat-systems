use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

use super::common::*;

#[test_macros::db_test(migrations = "../migrations")]
async fn list_workspaces_requires_auth_and_returns_data(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "ws-list", false).await;
    seed_workspace(&state, owner_id, "List WS").await;

    let (status, _) = send(&app, "GET", "/api/workspaces", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, body) = send(&app, "GET", "/api/workspaces", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body["data"].is_array(),
        "expected data array, got: {body:?}"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn create_workspace_happy_path(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_, _, token) = seed_and_login(&app, &state, "ws-create", false).await;

    let (status, body) = send(
        &app,
        "POST",
        "/api/workspaces",
        Some(&token),
        Some(json!({ "name": format!("Create WS {}", uuid_suffix()) })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "create workspace: {body:?}");
    assert!(body["id"].is_string(), "expected id, got: {body:?}");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn create_workspace_rejects_unauthenticated(pool: PgPool) {
    let (app, _state) = app_and_state(pool).await;
    let (status, _) = send(
        &app,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({ "name": "Nope WS" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn create_workspace_rejects_blank_name(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_, _, token) = seed_and_login(&app, &state, "ws-blank", false).await;

    let (status, _) = send(
        &app,
        "POST",
        "/api/workspaces",
        Some(&token),
        Some(json!({ "name": "" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn get_workspace_owner_ok_nonmember_forbidden(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "ws-get-owner", false).await;
    let (_, _, outsider_token) = seed_and_login(&app, &state, "ws-get-out", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Get WS").await;

    let (status, _) = send(&app, "GET", &format!("/api/workspaces/{ws_id}"), None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}"),
        Some(&owner_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["id"], ws_id.to_string());

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}"),
        Some(&outsider_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn update_workspace_admin_ok_member_and_nonmember_forbidden(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "ws-upd-owner", false).await;
    let (member_id, _, member_token) = seed_and_login(&app, &state, "ws-upd-mem", false).await;
    let (_, _, outsider_token) = seed_and_login(&app, &state, "ws-upd-out", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Update WS").await;
    add_ws_member(&state, ws_id, member_id, "member").await;

    let body = json!({ "description": "updated desc" });

    let (status, _) = send(
        &app,
        "PATCH",
        &format!("/api/workspaces/{ws_id}"),
        None,
        Some(body.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, resp) = send(
        &app,
        "PATCH",
        &format!("/api/workspaces/{ws_id}"),
        Some(&owner_token),
        Some(body.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "owner update: {resp:?}");

    let (status, _) = send(
        &app,
        "PATCH",
        &format!("/api/workspaces/{ws_id}"),
        Some(&member_token),
        Some(body.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = send(
        &app,
        "PATCH",
        &format!("/api/workspaces/{ws_id}"),
        Some(&outsider_token),
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn delete_and_restore_workspace_admin_ok_nonadmin_forbidden(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "ws-del-owner", false).await;
    let (member_id, _, member_token) = seed_and_login(&app, &state, "ws-del-mem", false).await;
    let (_, _, outsider_token) = seed_and_login(&app, &state, "ws-del-out", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Delete WS").await;
    add_ws_member(&state, ws_id, member_id, "member").await;

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/workspaces/{ws_id}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/workspaces/{ws_id}"),
        Some(&member_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/workspaces/{ws_id}"),
        Some(&outsider_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, body) = send(
        &app,
        "DELETE",
        &format!("/api/workspaces/{ws_id}"),
        Some(&owner_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "owner soft delete: {body:?}");

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/restore"),
        Some(&owner_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "owner restore: {body:?}");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn restore_workspace_rejects_unauthenticated(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "ws-restore", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Restore WS").await;

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/restore"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn list_deleted_workspaces_requires_auth(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_, _, token) = seed_and_login(&app, &state, "ws-deleted", false).await;

    let (status, _) = send(&app, "GET", "/api/workspaces/deleted", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, body) = send(&app, "GET", "/api/workspaces/deleted", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body["data"].is_array(),
        "expected data array, got: {body:?}"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn list_members_member_ok_nonmember_forbidden(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "mem-list-owner", false).await;
    let (_, _, outsider_token) = seed_and_login(&app, &state, "mem-list-out", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Members WS").await;

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/members"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/members"),
        Some(&owner_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body["data"].is_array(),
        "expected data array, got: {body:?}"
    );

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_id}/members"),
        Some(&outsider_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn update_member_role_admin_only(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "role-owner", false).await;
    let (member_id, _, member_token) = seed_and_login(&app, &state, "role-mem", false).await;
    let (target_id, _, _) = seed_and_login(&app, &state, "role-target", false).await;
    let (_, _, outsider_token) = seed_and_login(&app, &state, "role-out", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Role WS").await;
    add_ws_member(&state, ws_id, member_id, "member").await;
    add_ws_member(&state, ws_id, target_id, "member").await;

    let path = format!("/api/workspaces/{ws_id}/members/{target_id}/role");
    let body = json!({ "role": "admin" });

    let (status, _) = send(&app, "PATCH", &path, None, Some(body.clone())).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _) = send(
        &app,
        "PATCH",
        &path,
        Some(&member_token),
        Some(body.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = send(
        &app,
        "PATCH",
        &path,
        Some(&outsider_token),
        Some(body.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, resp) = send(&app, "PATCH", &path, Some(&owner_token), Some(body)).await;
    assert_eq!(status, StatusCode::OK, "owner role update: {resp:?}");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn remove_member_admin_only(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "rm-owner", false).await;
    let (member_id, _, member_token) = seed_and_login(&app, &state, "rm-mem", false).await;
    let (target_id, _, _) = seed_and_login(&app, &state, "rm-target", false).await;
    let (_, _, outsider_token) = seed_and_login(&app, &state, "rm-out", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Remove WS").await;
    add_ws_member(&state, ws_id, member_id, "member").await;
    add_ws_member(&state, ws_id, target_id, "member").await;

    let path = format!("/api/workspaces/{ws_id}/members/{target_id}");

    let (status, _) = send(&app, "DELETE", &path, None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _) = send(&app, "DELETE", &path, Some(&member_token), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = send(&app, "DELETE", &path, Some(&outsider_token), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, resp) = send(&app, "DELETE", &path, Some(&owner_token), None).await;
    assert_eq!(status, StatusCode::OK, "owner remove member: {resp:?}");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn list_invites_admin_only(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "inv-list-owner", false).await;
    let (member_id, _, member_token) = seed_and_login(&app, &state, "inv-list-mem", false).await;
    let (_, _, outsider_token) = seed_and_login(&app, &state, "inv-list-out", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Invite List WS").await;
    add_ws_member(&state, ws_id, member_id, "member").await;

    let path = format!("/api/workspaces/{ws_id}/invites");

    let (status, _) = send(&app, "GET", &path, None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _) = send(&app, "GET", &path, Some(&member_token), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = send(&app, "GET", &path, Some(&outsider_token), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, body) = send(&app, "GET", &path, Some(&owner_token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body["data"].is_array(),
        "expected data array, got: {body:?}"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn create_invite_admin_only(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "inv-new-owner", false).await;
    let (member_id, _, member_token) = seed_and_login(&app, &state, "inv-new-mem", false).await;
    let (_, _, outsider_token) = seed_and_login(&app, &state, "inv-new-out", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Invite Create WS").await;
    add_ws_member(&state, ws_id, member_id, "member").await;

    let path = format!("/api/workspaces/{ws_id}/invites");
    let body = json!({ "role": "member", "max_uses": 5 });

    let (status, _) = send(&app, "POST", &path, None, Some(body.clone())).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _) = send(&app, "POST", &path, Some(&member_token), Some(body.clone())).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = send(
        &app,
        "POST",
        &path,
        Some(&outsider_token),
        Some(body.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, resp) = send(&app, "POST", &path, Some(&owner_token), Some(body)).await;
    assert_eq!(status, StatusCode::OK, "owner create invite: {resp:?}");
    assert!(
        resp["token"].is_string(),
        "expected invite token, got: {resp:?}"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn accept_invite_bogus_token_is_404_and_valid_token_joins(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "inv-acc-owner", false).await;
    let (_, _, joiner_token) = seed_and_login(&app, &state, "inv-acc-join", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Accept WS").await;

    let (status, _) = send(&app, "POST", "/api/invites/whatever/accept", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _) = send(
        &app,
        "POST",
        "/api/invites/this-token-does-not-exist/accept",
        Some(&joiner_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, invite) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/invites"),
        Some(&owner_token),
        Some(json!({ "role": "member", "max_uses": 5 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "create invite: {invite:?}");
    let token = invite["token"].as_str().expect("invite token");

    let (status, resp) = send(
        &app,
        "POST",
        &format!("/api/invites/{token}/accept"),
        Some(&joiner_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "accept invite: {resp:?}");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn revoke_invite_admin_only(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "inv-rev-owner", false).await;
    let (member_id, _, member_token) = seed_and_login(&app, &state, "inv-rev-mem", false).await;
    let (_, _, outsider_token) = seed_and_login(&app, &state, "inv-rev-out", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Revoke WS").await;
    add_ws_member(&state, ws_id, member_id, "member").await;

    let (status, invite) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/invites"),
        Some(&owner_token),
        Some(json!({ "role": "member", "max_uses": 5 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "create invite: {invite:?}");
    let invite_id = invite["id"].as_str().expect("invite id");
    let path = format!("/api/workspaces/{ws_id}/invites/{invite_id}");

    let (status, _) = send(&app, "DELETE", &path, None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _) = send(&app, "DELETE", &path, Some(&member_token), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = send(&app, "DELETE", &path, Some(&outsider_token), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, resp) = send(&app, "DELETE", &path, Some(&owner_token), None).await;
    assert_eq!(status, StatusCode::OK, "owner revoke invite: {resp:?}");
}

fn uuid_suffix() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

#[test_macros::db_test(migrations = "../migrations")]
async fn workspace_admin_cannot_outrank_owner_or_itself(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, _) = seed_and_login(&app, &state, "esc-owner", false).await;
    let (admin_id, _, admin_token) = seed_and_login(&app, &state, "esc-admin", false).await;
    let (member_id, _, _) = seed_and_login(&app, &state, "esc-member", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Escalation WS").await;
    add_ws_member(&state, ws_id, admin_id, "admin").await;
    add_ws_member(&state, ws_id, member_id, "member").await;

    let (status, body) = send(
        &app,
        "PATCH",
        &format!("/api/workspaces/{ws_id}/members/{admin_id}/role"),
        Some(&admin_token),
        Some(json!({ "role": "owner" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "self promotion: {body:?}");

    let (status, body) = send(
        &app,
        "PATCH",
        &format!("/api/workspaces/{ws_id}/members/{owner_id}/role"),
        Some(&admin_token),
        Some(json!({ "role": "member", "max_uses": 5 })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "owner demotion: {body:?}");

    let (status, body) = send(
        &app,
        "PATCH",
        &format!("/api/workspaces/{ws_id}/members/{member_id}/role"),
        Some(&admin_token),
        Some(json!({ "role": "owner" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "granting owner: {body:?}");

    let (status, body) = send(
        &app,
        "DELETE",
        &format!("/api/workspaces/{ws_id}/members/{owner_id}"),
        Some(&admin_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "removing owner: {body:?}");

    let (status, body) = send(
        &app,
        "PATCH",
        &format!("/api/workspaces/{ws_id}/members/{member_id}/role"),
        Some(&admin_token),
        Some(json!({ "role": "admin" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "admin promoting a member: {body:?}");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn deleting_a_workspace_is_owner_only(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "del-owner", false).await;
    let (admin_id, _, admin_token) = seed_and_login(&app, &state, "del-admin", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Delete WS").await;
    add_ws_member(&state, ws_id, admin_id, "admin").await;

    let path = format!("/api/workspaces/{ws_id}");
    let (status, body) = send(&app, "DELETE", &path, Some(&admin_token), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "admin delete: {body:?}");

    let (status, body) = send(&app, "DELETE", &path, Some(&owner_token), None).await;
    assert_eq!(status, StatusCode::OK, "owner delete: {body:?}");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn guests_cannot_create_channels_or_reach_public_ones(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "guest-owner", false).await;
    let (guest_id, _, guest_token) = seed_and_login(&app, &state, "guest-user", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Guest WS").await;
    add_ws_member(&state, ws_id, guest_id, "guest").await;
    let public_ch = seed_channel(&state, ws_id, owner_id, "public-room", false).await;

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/channels"),
        Some(&guest_token),
        Some(json!({ "name": "guest-room" })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "guest create channel: {body:?}"
    );

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/channels/{public_ch}/messages"),
        Some(&guest_token),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "guest reads public: {body:?}"
    );

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/channels/{public_ch}/members"),
        Some(&owner_token),
        Some(json!({ "user_id": guest_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "adding guest to channel: {body:?}");

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/channels/{public_ch}/members"),
        Some(&owner_token),
        Some(json!({ "user_id": guest_id })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "re-adding an existing member: {body:?}"
    );

    let (status, body) = send(
        &app,
        "GET",
        &format!("/api/channels/{public_ch}/messages"),
        Some(&guest_token),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "guest reads joined channel: {body:?}"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn link_invite_requires_an_explicit_use_limit(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "inv-cap-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Invite Cap WS").await;
    let path = format!("/api/workspaces/{ws_id}/invites");

    let (status, body) = send(
        &app,
        "POST",
        &path,
        Some(&owner_token),
        Some(json!({ "role": "member" })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "a link invite with no max_uses must be refused: {body:?}"
    );

    let (status, body) = send(
        &app,
        "POST",
        &path,
        Some(&owner_token),
        Some(json!({ "role": "member", "max_uses": 1000 })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "over the cap: {body:?}"
    );

    let (status, body) = send(
        &app,
        "POST",
        &path,
        Some(&owner_token),
        Some(json!({ "role": "member", "max_uses": 5, "expires_in_hours": 1000 })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "over the max lifetime: {body:?}"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn every_new_invite_carries_an_expiry_and_a_use_limit(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "inv-bounded-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Invite Bounded WS").await;

    let (status, invite) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/invites"),
        Some(&owner_token),
        Some(json!({ "role": "member", "max_uses": 3 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "create invite: {invite:?}");
    assert_eq!(invite["max_uses"], 3);
    assert!(
        invite["expires_at"].is_string(),
        "an invite without an expiry is a permanent key: {invite:?}"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_email_invite_only_works_for_the_address_it_was_issued_to(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "inv-bind-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Invite Bind WS").await;

    let invited_email = unique_email("inv-bind-invited");
    let (status, invite) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/invites"),
        Some(&owner_token),
        Some(json!({ "role": "member", "email": invited_email })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "create invite: {invite:?}");
    assert_eq!(invite["max_uses"], 1, "an email invite is for one person");
    let token = invite["token"].as_str().expect("invite token").to_string();

    let (_, _, wrong_token) = seed_and_login(&app, &state, "inv-bind-wrong", false).await;
    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/invites/{token}/accept"),
        Some(&wrong_token),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a forwarded invite must not let a different account in: {body:?}"
    );

    let hash = crate::auth::service::AuthService::hash_password(PASSWORD).expect("hash");
    state
        .auth_service
        .repo()
        .activate(
            state
                .auth_service
                .repo()
                .find_by_email(&invited_email)
                .await
                .expect("find invited")
                .expect("invited user was provisioned")
                .id,
            &hash,
            "Invited",
        )
        .await
        .expect("activate invited");
    let invited_token = login(&app, &invited_email, PASSWORD).await;

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/invites/{token}/accept"),
        Some(&invited_token),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the invited address joins: {body:?}"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_expired_invite_is_refused(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "inv-exp-owner", false).await;
    let (_, _, joiner_token) = seed_and_login(&app, &state, "inv-exp-joiner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Invite Expiry WS").await;

    let (_, invite) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/invites"),
        Some(&owner_token),
        Some(json!({ "role": "member", "max_uses": 5 })),
    )
    .await;
    let token = invite["token"].as_str().expect("invite token").to_string();

    sqlx::query(
        "UPDATE workspace_invites SET expires_at = NOW() - INTERVAL '1 hour' WHERE token = $1",
    )
    .bind(&token)
    .execute(&state.pool)
    .await
    .expect("expire the invite");

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/invites/{token}/accept"),
        Some(&joiner_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "expired invite: {body:?}");
}
