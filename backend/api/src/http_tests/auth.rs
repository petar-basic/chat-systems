use axum::body::Body;
use axum::http::header::{COOKIE, SET_COOKIE};
use axum::http::{Request, StatusCode};
use axum::Router;
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde_json::json;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use super::common::*;
use crate::middleware::Claims;

const TEST_JWT_SECRET: &str = "integration-test-secret-key-0123456789-abcdef";

#[sqlx::test(migrations = "../migrations")]
async fn instance_info_is_public(pool: PgPool) {
    let (app, _state) = app_and_state(pool).await;

    let (status, body) = send(&app, "GET", "/api/instance/info", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["name"], "Test");
}

#[sqlx::test(migrations = "../migrations")]
async fn update_me_requires_auth(pool: PgPool) {
    let (app, _state) = app_and_state(pool).await;

    let (status, _) = send(
        &app,
        "PATCH",
        "/api/users/me",
        None,
        Some(json!({ "display_name": "Nope" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn update_me_updates_profile(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_id, _email, token) = seed_and_login(&app, &state, "profile", false).await;

    let (status, body) = send(
        &app,
        "PATCH",
        "/api/users/me",
        Some(&token),
        Some(json!({
            "display_name": "Updated Name",
            "bio": "hello world",
            "timezone": "Europe/Berlin",
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "update profile should succeed: {body:?}"
    );
    assert_eq!(body["display_name"], "Updated Name");
    assert_eq!(body["bio"], "hello world");
    assert_eq!(body["timezone"], "Europe/Berlin");

    let (status, me) = send(&app, "GET", "/api/users/me", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["display_name"], "Updated Name");
}

#[sqlx::test(migrations = "../migrations")]
async fn update_me_sets_clears_and_rejects_avatar_urls(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_id, _email, token) = seed_and_login(&app, &state, "avatar", false).await;

    let (status, body) = send(
        &app,
        "PATCH",
        "/api/users/me",
        Some(&token),
        Some(json!({ "avatar_url": "/api/files/download/ws/id/me.png" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "setting an avatar: {body:?}");
    assert_eq!(body["avatar_url"], "/api/files/download/ws/id/me.png");

    let (status, body) = send(
        &app,
        "PATCH",
        "/api/users/me",
        Some(&token),
        Some(json!({ "avatar_url": "javascript:alert(1)" })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "hostile scheme: {body:?}"
    );

    let (_status, me) = send(&app, "GET", "/api/users/me", Some(&token), None).await;
    assert_eq!(me["avatar_url"], "/api/files/download/ws/id/me.png");

    let (status, body) = send(
        &app,
        "PATCH",
        "/api/users/me",
        Some(&token),
        Some(json!({ "avatar_url": "" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "clearing an avatar: {body:?}");
    assert!(body["avatar_url"].is_null(), "avatar should be cleared");
}

#[sqlx::test(migrations = "../migrations")]
async fn refresh_without_cookie_is_unauthorized(pool: PgPool) {
    let (app, _state) = app_and_state(pool).await;

    let (status, _) = send(&app, "POST", "/api/auth/refresh", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn logout_is_ok_and_idempotent(pool: PgPool) {
    let (app, _state) = app_and_state(pool).await;

    let (status, body) = send(&app, "POST", "/api/auth/logout", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "logged_out");

    let (status, _) = send(&app, "POST", "/api/auth/logout", None, None).await;
    assert_eq!(status, StatusCode::OK);
}

#[sqlx::test(migrations = "../migrations")]
async fn forgot_password_does_not_leak_account_existence(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_id, email) = seed(&state, "forgot", false).await;

    let (status, body) = send(
        &app,
        "POST",
        "/api/auth/forgot-password",
        None,
        Some(json!({ "email": email })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "sent");

    let (status, body) = send(
        &app,
        "POST",
        "/api/auth/forgot-password",
        None,
        Some(json!({ "email": unique_email("ghost") })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "sent");
}

#[sqlx::test(migrations = "../migrations")]
async fn reset_password_rejects_bogus_token(pool: PgPool) {
    let (app, _state) = app_and_state(pool).await;

    let (status, _) = send(
        &app,
        "POST",
        "/api/auth/reset-password",
        None,
        Some(json!({ "token": "not-a-real-token", "password": PASSWORD })),
    )
    .await;
    assert!(
        status == StatusCode::UNAUTHORIZED || status == StatusCode::BAD_REQUEST,
        "bogus reset token should be rejected, got {status}"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn complete_registration_rejects_bogus_token(pool: PgPool) {
    let (app, _state) = app_and_state(pool).await;

    let (status, _) = send(
        &app,
        "POST",
        "/api/auth/complete-registration",
        None,
        Some(json!({
            "token": "not-a-real-token",
            "display_name": "New User",
            "password": PASSWORD,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn complete_registration_activates_account_and_allows_login(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;

    let (owner_id, _owner_email) = seed(&state, "ws-owner", false).await;
    let workspace_id = seed_workspace(&state, owner_id, "Invite WS").await;

    let invitee_email = unique_email("invitee");
    let pending = state
        .auth_service
        .repo()
        .create(&invitee_email, None, None, false)
        .await
        .expect("create pending user");

    let token = state
        .auth_service
        .generate_registration_token(pending.id, &invitee_email, workspace_id, "member")
        .expect("generate registration token");

    let new_password = "newpassword123";
    let (status, body) = send(
        &app,
        "POST",
        "/api/auth/complete-registration",
        None,
        Some(json!({
            "token": token,
            "password": new_password,
            "display_name": "Invited User",
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "complete-registration should succeed: {body:?}"
    );
    assert!(
        body["access_token"].as_str().is_some_and(|t| !t.is_empty()),
        "complete-registration must return an access_token, got {body:?}"
    );
    assert_eq!(body["user"]["email"], invitee_email);
    assert_eq!(body["user"]["display_name"], "Invited User");
    assert_eq!(body["user"]["status"], "active");

    let (status, login_body) = send(
        &app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({ "email": invitee_email, "password": new_password })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "activated user should log in: {login_body:?}"
    );
    assert!(login_body["access_token"]
        .as_str()
        .is_some_and(|t| !t.is_empty()));

    let role: Option<String> = sqlx::query_scalar(
        "SELECT role::text FROM workspace_members WHERE workspace_id = $1 AND user_id = $2",
    )
    .bind(workspace_id)
    .bind(pending.id)
    .fetch_optional(&state.pool)
    .await
    .expect("query membership");
    assert_eq!(
        role.as_deref(),
        Some("member"),
        "completed registration must join the user to the invite workspace as member"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn reset_password_changes_password_and_is_single_use(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (user_id, email) = seed(&state, "reset-happy", false).await;

    let now = Utc::now();
    let exp = now + Duration::seconds(3600);
    let jti = Uuid::new_v4();
    let claims = Claims {
        sub: user_id,
        email: String::new(),
        is_instance_admin: false,
        iat: now.timestamp(),
        exp: exp.timestamp(),
        jti: Some(jti),
        token_type: "reset".to_string(),
        workspace_id: None,
        invite_role: None,
    };
    let reset_token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(TEST_JWT_SECRET.as_bytes()),
    )
    .expect("encode reset token");
    state
        .auth_service
        .repo()
        .store_reset_jti(jti, user_id, exp)
        .await
        .expect("store reset jti");

    let token = login(&app, &email, PASSWORD).await;
    assert!(!token.is_empty());

    let new_password = "freshpassword123";
    let (status, body) = send(
        &app,
        "POST",
        "/api/auth/reset-password",
        None,
        Some(json!({ "token": reset_token, "password": new_password })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "reset-password should succeed: {body:?}"
    );
    assert_eq!(body["status"], "reset");

    let (status, login_body) = send(
        &app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({ "email": email, "password": new_password })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "new password must log in: {login_body:?}"
    );
    assert!(login_body["access_token"]
        .as_str()
        .is_some_and(|t| !t.is_empty()));

    let (status, _) = send(
        &app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({ "email": email, "password": PASSWORD })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "the old password must stop working after a reset"
    );

    let (status, _) = send(
        &app,
        "POST",
        "/api/auth/reset-password",
        None,
        Some(json!({ "token": reset_token, "password": "yetanotherpw123" })),
    )
    .await;
    assert!(
        status == StatusCode::UNAUTHORIZED || status == StatusCode::BAD_REQUEST,
        "a consumed reset token must be rejected on reuse, got {status}"
    );

    let (status, _) = send(
        &app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({ "email": email, "password": new_password })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a rejected reset replay must not change the password again"
    );
}

async fn send_raw(
    app: &Router,
    method: &str,
    uri: &str,
    cookie: Option<&str>,
    body: Option<serde_json::Value>,
) -> axum::response::Response {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(c) = cookie {
        builder = builder.header(COOKIE, c);
    }
    let request = match body {
        Some(b) => builder
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&b).unwrap()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    };
    app.clone().oneshot(request).await.unwrap()
}

fn extract_refresh_cookie(resp: &axum::response::Response) -> Option<String> {
    resp.headers()
        .get_all(SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find_map(|raw| {
            let pair = raw.split(';').next()?.trim();
            if let Some(value) = pair.strip_prefix("refresh_token=") {
                if value.is_empty() {
                    None
                } else {
                    Some(pair.to_string())
                }
            } else {
                None
            }
        })
}

#[sqlx::test(migrations = "../migrations")]
async fn refresh_with_cookie_returns_new_access_token(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_id, email) = seed(&state, "refresh-cookie", false).await;

    let login_resp = send_raw(
        &app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({ "email": email, "password": PASSWORD })),
    )
    .await;
    assert_eq!(login_resp.status(), StatusCode::OK, "login should succeed");
    let refresh_cookie =
        extract_refresh_cookie(&login_resp).expect("login must set a refresh_token cookie");

    let refresh_resp = send_raw(
        &app,
        "POST",
        "/api/auth/refresh",
        Some(&refresh_cookie),
        None,
    )
    .await;
    assert_eq!(
        refresh_resp.status(),
        StatusCode::OK,
        "refresh with a valid cookie must succeed"
    );

    let bytes = axum::body::to_bytes(refresh_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    assert!(
        body["access_token"].as_str().is_some_and(|t| !t.is_empty()),
        "refresh must return a non-empty access_token, got {body:?}"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn revoke_all_rejects_every_outstanding_access_token(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (user_id, email) = seed(&state, "revoke-all", false).await;

    let first = login(&app, &email, PASSWORD).await;
    let second = login(&app, &email, PASSWORD).await;

    for token in [&first, &second] {
        let (status, _b) = send(&app, "GET", "/api/workspaces", Some(token), None).await;
        assert_eq!(status, StatusCode::OK, "both sessions start valid");
    }

    crate::sessions::revoke(&state, user_id, crate::sessions::SessionScope::All, "test")
        .await
        .expect("revoke");

    for token in [&first, &second] {
        let (status, _b) = send(&app, "GET", "/api/workspaces", Some(token), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "every session is cut off");
    }
}

#[sqlx::test(migrations = "../migrations")]
async fn revoke_all_except_spares_the_named_session(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (user_id, email) = seed(&state, "revoke-except", false).await;

    let survivor = login(&app, &email, PASSWORD).await;
    let doomed = login(&app, &email, PASSWORD).await;

    let survivor_jti = access_token_jti(&survivor);

    crate::sessions::revoke(
        &state,
        user_id,
        crate::sessions::SessionScope::AllExcept(survivor_jti),
        "password changed",
    )
    .await
    .expect("revoke");

    let (kept, _b) = send(&app, "GET", "/api/workspaces", Some(&survivor), None).await;
    assert_eq!(
        kept,
        StatusCode::OK,
        "the session that initiated the change keeps working"
    );

    let (cut, _b) = send(&app, "GET", "/api/workspaces", Some(&doomed), None).await;
    assert_eq!(cut, StatusCode::UNAUTHORIZED, "every other session is cut");
}

#[sqlx::test(migrations = "../migrations")]
async fn revoke_all_leaves_the_refresh_token_of_the_spared_session(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (user_id, email) = seed(&state, "revoke-refresh", false).await;

    let (status, body) = send(
        &app,
        "POST",
        "/api/auth/login",
        None,
        Some(serde_json::json!({ "email": email, "password": PASSWORD })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "login: {body:?}");
    let refresh = body["refresh_token"]
        .as_str()
        .expect("refresh_token")
        .to_string();
    let access = body["access_token"]
        .as_str()
        .expect("access_token")
        .to_string();

    crate::sessions::revoke(
        &state,
        user_id,
        crate::sessions::SessionScope::AllExcept(access_token_jti(&access)),
        "password changed",
    )
    .await
    .expect("revoke");

    let (status, body) = send(&app, "POST", "/api/auth/refresh", Some(&refresh), None).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the spared session can still rotate its refresh token: {body:?}"
    );
}

fn access_token_jti(token: &str) -> Uuid {
    use base64::Engine;
    let payload = token.split('.').nth(1).expect("jwt has a payload segment");
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .expect("payload is base64url");
    let claims: serde_json::Value = serde_json::from_slice(&bytes).expect("payload is json");
    claims["jti"]
        .as_str()
        .expect("access tokens carry a jti")
        .parse()
        .expect("jti is a uuid")
}

#[sqlx::test(migrations = "../migrations")]
async fn a_legacy_revocation_record_still_locks_the_session_out(pool: PgPool) {
    use redis::AsyncCommands;

    let (app, state) = app_and_state(pool).await;
    let (user_id, email) = seed(&state, "legacy-revocation", false).await;
    let token = login(&app, &email, PASSWORD).await;

    // What an older build wrote: the bare value `1`, not a JSON record.
    let mut redis = state.redis.clone();
    let _: () = redis
        .set_ex(format!("revoked:{user_id}"), 1, 60)
        .await
        .expect("write legacy revocation record");

    let (status, _b) = send(&app, "GET", "/api/workspaces", Some(&token), None).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "an unparseable revocation record must fail closed, not grant access"
    );

    let _: () = redis
        .del(format!("revoked:{user_id}"))
        .await
        .expect("clear revocation");
}
