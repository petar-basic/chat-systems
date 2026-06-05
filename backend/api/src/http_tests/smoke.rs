use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

use super::common::*;

#[sqlx::test(migrations = "../migrations")]
async fn health_probes_report_ok(pool: PgPool) {
    let (app, _state) = app_and_state(pool).await;

    let (status, _) = send(&app, "GET", "/livez", None, None).await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = send(&app, "GET", "/readyz", None, None).await;
    assert_eq!(status, StatusCode::OK);
}

#[sqlx::test(migrations = "../migrations")]
async fn login_rejects_bad_credentials_and_authenticates_valid_user(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_, email) = seed(&state, "login", false).await;

    let (status, _) = send(
        &app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({ "email": unique_email("ghost"), "password": PASSWORD })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _) = send(
        &app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({ "email": email, "password": "wrong-password" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let token = login(&app, &email, PASSWORD).await;

    let (status, _) = send(&app, "GET", "/api/users/me", None, None).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "no token must be rejected"
    );

    let (status, body) = send(&app, "GET", "/api/users/me", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["email"], email);
}

#[sqlx::test(migrations = "../migrations")]
async fn hooks_enforce_cross_tenant_authorization(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;

    let (a_id, _a_email) = seed(&state, "owner-a", false).await;
    let (b_id, b_email) = seed(&state, "owner-b", false).await;
    let ws_a = seed_workspace(&state, a_id, "Workspace A").await;
    let ws_b = seed_workspace(&state, b_id, "Workspace B").await;

    let b_token = login(&app, &b_email, PASSWORD).await;

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_a}/hooks"),
        Some(&b_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_b}/hooks"),
        Some(&b_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{ws_b}/hooks"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn change_password_revokes_old_password(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_, email) = seed(&state, "pwchange", false).await;
    let token = login(&app, &email, PASSWORD).await;

    let (status, _) = send(
        &app,
        "PATCH",
        "/api/users/me/password",
        Some(&token),
        Some(json!({ "current_password": PASSWORD, "new_password": "password123-new" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "change-password should succeed");

    let (status, _) = send(
        &app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({ "email": email, "password": PASSWORD })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let _ = login(&app, &email, "password123-new").await;
}

#[sqlx::test(migrations = "../migrations")]
async fn unknown_route_returns_404(pool: PgPool) {
    let (app, _state) = app_and_state(pool).await;
    let (status, _) = send(&app, "GET", "/api/this-route-does-not-exist", None, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
