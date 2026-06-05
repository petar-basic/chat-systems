use super::common::*;
use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

#[sqlx::test(migrations = "../migrations")]
async fn stats_as_admin_returns_200(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_id, _email, token) = seed_and_login(&app, &state, "admin-stats", true).await;

    let (status, body) = send(&app, "GET", "/api/admin/stats", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK, "stats: {body:?}");
    assert!(body["users"].is_number(), "stats body: {body:?}");
}

#[sqlx::test(migrations = "../migrations")]
async fn stats_without_token_returns_401(pool: PgPool) {
    let (app, _state) = app_and_state(pool).await;
    let (status, _body) = send(&app, "GET", "/api/admin/stats", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn stats_as_non_admin_returns_403(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_id, _email, token) = seed_and_login(&app, &state, "stats-nonadmin", false).await;

    let (status, _body) = send(&app, "GET", "/api/admin/stats", Some(&token), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../migrations")]
async fn health_as_admin_returns_200(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_id, _email, token) = seed_and_login(&app, &state, "admin-health", true).await;

    let (status, body) = send(&app, "GET", "/api/admin/health", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK, "health: {body:?}");
}

#[sqlx::test(migrations = "../migrations")]
async fn health_as_non_admin_returns_403(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_id, _email, token) = seed_and_login(&app, &state, "health-nonadmin", false).await;

    let (status, _body) = send(&app, "GET", "/api/admin/health", Some(&token), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../migrations")]
async fn list_users_as_admin_returns_200(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_id, _email, token) = seed_and_login(&app, &state, "admin-listusers", true).await;
    let _ = seed(&state, "listed-a", false).await;
    let _ = seed(&state, "listed-b", false).await;

    let (status, body) = send(&app, "GET", "/api/admin/users", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK, "list_users: {body:?}");
    assert!(body["data"].is_array(), "list_users body: {body:?}");
}

#[sqlx::test(migrations = "../migrations")]
async fn list_users_paginated_returns_200(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_id, _email, token) = seed_and_login(&app, &state, "admin-listpage", true).await;
    let _ = seed(&state, "page-a", false).await;
    let _ = seed(&state, "page-b", false).await;

    let (status, body) = send(
        &app,
        "GET",
        "/api/admin/users?limit=1&offset=0",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "list_users paginated: {body:?}");
    let data = body["data"].as_array().expect("data array");
    assert_eq!(data.len(), 1, "limit=1 should yield one row: {body:?}");
}

#[sqlx::test(migrations = "../migrations")]
async fn list_users_without_token_returns_401(pool: PgPool) {
    let (app, _state) = app_and_state(pool).await;
    let (status, _body) = send(&app, "GET", "/api/admin/users", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn list_users_as_non_admin_returns_403(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_id, _email, token) = seed_and_login(&app, &state, "listusers-nonadmin", false).await;

    let (status, _body) = send(&app, "GET", "/api/admin/users", Some(&token), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../migrations")]
async fn suspend_user_as_admin_returns_200(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_admin_id, _email, token) = seed_and_login(&app, &state, "admin-suspend", true).await;
    let (target_id, _target_email) = seed(&state, "suspend-target", false).await;

    let uri = format!("/api/admin/users/{target_id}/suspend");
    let (status, body) = send(&app, "POST", &uri, Some(&token), None).await;
    assert_eq!(status, StatusCode::OK, "suspend: {body:?}");
}

#[sqlx::test(migrations = "../migrations")]
async fn activate_user_as_admin_returns_200(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_admin_id, _email, token) = seed_and_login(&app, &state, "admin-activate", true).await;
    let (target_id, _target_email) = seed(&state, "activate-target", false).await;

    let suspend_uri = format!("/api/admin/users/{target_id}/suspend");
    let (s_status, _s_body) = send(&app, "POST", &suspend_uri, Some(&token), None).await;
    assert_eq!(s_status, StatusCode::OK);

    let activate_uri = format!("/api/admin/users/{target_id}/activate");
    let (status, body) = send(&app, "POST", &activate_uri, Some(&token), None).await;
    assert_eq!(status, StatusCode::OK, "activate: {body:?}");
}

#[sqlx::test(migrations = "../migrations")]
async fn suspend_user_without_token_returns_401(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (target_id, _target_email) = seed(&state, "suspend-noauth", false).await;

    let uri = format!("/api/admin/users/{target_id}/suspend");
    let (status, _body) = send(&app, "POST", &uri, None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn suspend_user_as_non_admin_returns_403(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_id, _email, token) = seed_and_login(&app, &state, "suspend-nonadmin", false).await;
    let (target_id, _target_email) = seed(&state, "suspend-target2", false).await;

    let uri = format!("/api/admin/users/{target_id}/suspend");
    let (status, _body) = send(&app, "POST", &uri, Some(&token), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../migrations")]
async fn activate_user_as_non_admin_returns_403(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_id, _email, token) = seed_and_login(&app, &state, "activate-nonadmin", false).await;
    let (target_id, _target_email) = seed(&state, "activate-target2", false).await;

    let uri = format!("/api/admin/users/{target_id}/activate");
    let (status, _body) = send(&app, "POST", &uri, Some(&token), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../migrations")]
async fn update_instance_role_as_admin_returns_200(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_admin_id, _email, token) = seed_and_login(&app, &state, "admin-role", true).await;
    let (target_id, _target_email) = seed(&state, "role-target", false).await;

    let uri = format!("/api/admin/users/{target_id}/instance-role");
    let (status, body) = send(
        &app,
        "PATCH",
        &uri,
        Some(&token),
        Some(json!({ "is_instance_admin": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "update_instance_role: {body:?}");
    assert_eq!(body["is_instance_admin"], true, "role body: {body:?}");
}

#[sqlx::test(migrations = "../migrations")]
async fn update_instance_role_bad_body_returns_422(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_admin_id, _email, token) = seed_and_login(&app, &state, "admin-rolebad", true).await;
    let (target_id, _target_email) = seed(&state, "rolebad-target", false).await;

    let uri = format!("/api/admin/users/{target_id}/instance-role");
    let (status, _body) = send(&app, "PATCH", &uri, Some(&token), Some(json!({}))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrations = "../migrations")]
async fn update_instance_role_without_token_returns_401(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (target_id, _target_email) = seed(&state, "role-noauth", false).await;

    let uri = format!("/api/admin/users/{target_id}/instance-role");
    let (status, _body) = send(
        &app,
        "PATCH",
        &uri,
        None,
        Some(json!({ "is_instance_admin": true })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn update_instance_role_as_non_admin_returns_403(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_id, _email, token) = seed_and_login(&app, &state, "role-nonadmin", false).await;
    let (target_id, _target_email) = seed(&state, "role-target2", false).await;

    let uri = format!("/api/admin/users/{target_id}/instance-role");
    let (status, _body) = send(
        &app,
        "PATCH",
        &uri,
        Some(&token),
        Some(json!({ "is_instance_admin": true })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../migrations")]
async fn list_workspaces_as_admin_returns_200(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (admin_id, _email, token) = seed_and_login(&app, &state, "admin-listws", true).await;
    let _ws = seed_workspace(&state, admin_id, "Admin WS").await;

    let (status, body) = send(&app, "GET", "/api/admin/workspaces", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK, "list_workspaces: {body:?}");
    assert!(body["data"].is_array(), "list_workspaces body: {body:?}");
}

#[sqlx::test(migrations = "../migrations")]
async fn list_workspaces_without_token_returns_401(pool: PgPool) {
    let (app, _state) = app_and_state(pool).await;
    let (status, _body) = send(&app, "GET", "/api/admin/workspaces", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn list_workspaces_as_non_admin_returns_403(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_id, _email, token) = seed_and_login(&app, &state, "listws-nonadmin", false).await;

    let (status, _body) = send(&app, "GET", "/api/admin/workspaces", Some(&token), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../migrations")]
async fn delete_workspace_as_admin_returns_200(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (admin_id, _email, token) = seed_and_login(&app, &state, "admin-delws", true).await;
    let ws_id = seed_workspace(&state, admin_id, "Doomed WS").await;

    let uri = format!("/api/admin/workspaces/{ws_id}");
    let (status, body) = send(&app, "DELETE", &uri, Some(&token), None).await;
    assert_eq!(status, StatusCode::OK, "delete_workspace: {body:?}");
}

#[sqlx::test(migrations = "../migrations")]
async fn delete_workspace_without_token_returns_401(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _email) = seed(&state, "delws-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Survives WS").await;

    let uri = format!("/api/admin/workspaces/{ws_id}");
    let (status, _body) = send(&app, "DELETE", &uri, None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn delete_workspace_as_non_admin_returns_403(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _email, token) = seed_and_login(&app, &state, "delws-nonadmin", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Protected WS").await;

    let uri = format!("/api/admin/workspaces/{ws_id}");
    let (status, _body) = send(&app, "DELETE", &uri, Some(&token), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
