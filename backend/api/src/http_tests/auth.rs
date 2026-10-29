use axum::body::Body;
use axum::http::header::{COOKIE, SET_COOKIE};
use axum::http::{Request, StatusCode};
use axum::Router;
use chrono::{Duration, Utc};
use serde_json::json;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use super::common::*;
use crate::middleware::Claims;
use crate::workspace::models::WorkspaceRole;

#[test_macros::db_test(migrations = "../migrations")]
async fn instance_info_is_public(pool: PgPool) {
    let (app, _state) = app_and_state(pool).await;

    let (status, body) = send(&app, "GET", "/api/instance/info", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["name"], "Test");
}

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
async fn refresh_without_cookie_is_unauthorized(pool: PgPool) {
    let (app, _state) = app_and_state(pool).await;

    let (status, _) = send(&app, "POST", "/api/auth/refresh", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn logout_is_ok_and_idempotent(pool: PgPool) {
    let (app, _state) = app_and_state(pool).await;

    let (status, body) = send(&app, "POST", "/api/auth/logout", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "logged_out");

    let (status, _) = send(&app, "POST", "/api/auth/logout", None, None).await;
    assert_eq!(status, StatusCode::OK);
}

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

    let (_, token) = email_invite(
        &state,
        workspace_id,
        owner_id,
        pending.id,
        &invitee_email,
        WorkspaceRole::Member,
    )
    .await;

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

#[test_macros::db_test(migrations = "../migrations")]
async fn an_email_invite_link_lives_and_dies_with_its_invite(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _) = seed(&state, "ws-owner", false).await;
    let workspace_id = seed_workspace(&state, owner_id, "Invite WS").await;
    let pending = |label: &'static str| {
        let state = state.clone();
        async move {
            let email = unique_email(label);
            let user = state
                .auth_service
                .repo()
                .create(&email, None, None, false)
                .await
                .expect("create pending user");
            (user.id, email)
        }
    };
    let verify = |token: String| {
        let app = app.clone();
        async move {
            send(
                &app,
                "GET",
                &format!("/api/auth/invites/{token}/verify"),
                None,
                None,
            )
            .await
            .0
        }
    };
    let complete = |token: String| {
        let app = app.clone();
        async move {
            send(
                &app,
                "POST",
                "/api/auth/complete-registration",
                None,
                Some(json!({
                    "token": token,
                    "password": "newpassword123",
                    "display_name": "Invited User",
                })),
            )
            .await
            .0
        }
    };

    let (revoked_id, revoked_email) = pending("revoked").await;
    let (invite_id, token) = email_invite(
        &state,
        workspace_id,
        owner_id,
        revoked_id,
        &revoked_email,
        WorkspaceRole::Member,
    )
    .await;
    assert_eq!(verify(token.clone()).await, StatusCode::OK);
    state
        .workspace_service
        .repo
        .delete_invite(invite_id)
        .await
        .expect("revoke");
    assert_eq!(
        verify(token.clone()).await,
        StatusCode::UNAUTHORIZED,
        "a revoked invite's link must stop opening"
    );
    assert_eq!(
        complete(token).await,
        StatusCode::UNAUTHORIZED,
        "a revoked invite's link must not register anyone"
    );

    let (admin_id, admin_email) = pending("admin").await;
    let (_, token) = email_invite(
        &state,
        workspace_id,
        owner_id,
        admin_id,
        &admin_email,
        WorkspaceRole::Admin,
    )
    .await;
    assert_eq!(complete(token.clone()).await, StatusCode::OK);
    let role: Option<String> = sqlx::query_scalar(
        "SELECT role::text FROM workspace_members WHERE workspace_id = $1 AND user_id = $2",
    )
    .bind(workspace_id)
    .bind(admin_id)
    .fetch_optional(&state.pool)
    .await
    .expect("query membership");
    assert_eq!(role.as_deref(), Some("admin"), "the role is the invite's");
    assert_eq!(
        verify(token).await,
        StatusCode::UNAUTHORIZED,
        "a used invite's link must stop opening"
    );

    let (stale_id, stale_email) = pending("stale").await;
    let now = Utc::now();
    let stale = state
        .config
        .jwt_keys
        .encode(&Claims {
            sub: stale_id,
            email: stale_email,
            is_instance_admin: false,
            iat: now.timestamp(),
            exp: (now + Duration::days(7)).timestamp(),
            jti: None,
            token_type: "registration".to_string(),
            invite_id: None,
        })
        .expect("encode");
    assert_eq!(
        verify(stale).await,
        StatusCode::UNAUTHORIZED,
        "a link minted before invites were bound to their row is refused"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
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
        invite_id: None,
    };
    let reset_token = state
        .config
        .jwt_keys
        .encode(&claims)
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
async fn a_password_reset_ends_every_existing_session(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (user_id, email) = seed(&state, "reset-revokes", false).await;

    let stolen = login(&app, &email, PASSWORD).await;
    let (status, _b) = send(&app, "GET", "/api/workspaces", Some(&stolen), None).await;
    assert_eq!(status, StatusCode::OK, "the session starts valid");

    let reset_token = state
        .auth_service
        .issue_reset_token_for_test(user_id)
        .await
        .expect("issue reset token");

    let (status, body) = send(
        &app,
        "POST",
        "/api/auth/reset-password",
        None,
        Some(json!({ "token": reset_token, "password": "a-brand-new-password" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "reset: {body:?}");

    let (status, _b) = send(&app, "GET", "/api/workspaces", Some(&stolen), None).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "recovering the account must evict whoever else was holding a session"
    );

    let refresh_left: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM refresh_tokens WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&state.pool)
            .await
            .expect("count refresh tokens");
    assert_eq!(refresh_left, 0, "refresh tokens must be gone too");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn changing_the_password_keeps_the_session_that_did_it(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_user_id, email) = seed(&state, "change-keeps-self", false).await;

    let mine = login(&app, &email, PASSWORD).await;
    let other_device = login(&app, &email, PASSWORD).await;

    let (status, body) = send(
        &app,
        "PATCH",
        "/api/users/me/password",
        Some(&mine),
        Some(json!({ "current_password": PASSWORD, "new_password": "a-brand-new-password" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "change password: {body:?}");

    let (status, _b) = send(&app, "GET", "/api/workspaces", Some(&mine), None).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the tab that changed the password stays signed in"
    );

    let (status, _b) = send(&app, "GET", "/api/workspaces", Some(&other_device), None).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "every other device is out"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn every_login_failure_looks_the_same(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;

    let (active_id, active_email) = seed(&state, "uniform-active", false).await;
    let pending_email = unique_email("uniform-pending");
    state
        .auth_service
        .repo()
        .create(&pending_email, None, None, false)
        .await
        .expect("provision a pending user");
    sqlx::query("UPDATE users SET status = 'suspended' WHERE id = $1")
        .bind(active_id)
        .execute(&state.pool)
        .await
        .expect("suspend");

    let attempts = [
        ("unknown address", unique_email("uniform-nobody")),
        ("suspended account", active_email),
        ("invited but not registered", pending_email),
    ];

    let mut seen: Vec<(axum::http::StatusCode, serde_json::Value)> = Vec::new();
    for (label, email) in attempts {
        let (status, body) = send(
            &app,
            "POST",
            "/api/auth/login",
            None,
            Some(json!({ "email": email, "password": "whatever-goes-here" })),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{label}");
        seen.push((status, body));
    }

    let first = &seen[0];
    for (status, body) in &seen[1..] {
        assert_eq!(
            (status, body),
            (&first.0, &first.1),
            "every failure must be indistinguishable, or the login form is an address book"
        );
    }
}

/// The failure mode CS-033 depends on. A Redis that cannot answer used to mean
/// "not revoked", so during an incident a suspended or deprovisioned person kept
/// working for the life of their access token.
#[sqlx::test(migrations = "../migrations")]
async fn a_revocation_store_that_cannot_answer_refuses_the_request(pool: PgPool) {
    use crate::middleware::{RevocationRecord, RevocationState, RevocationStore};

    let (app, state) = app_and_state(pool).await;
    let (user_id, _, token) = seed_and_login(&app, &state, "revoke", false).await;

    let healthy = RevocationStore(state.redis.clone());
    let claims = crate::middleware::Claims {
        sub: user_id,
        email: "revoke@test.local".into(),
        is_instance_admin: false,
        iat: chrono::Utc::now().timestamp(),
        exp: chrono::Utc::now().timestamp() + 900,
        jti: Some(uuid::Uuid::new_v4()),
        token_type: "access".into(),
        invite_id: None,
    };
    assert_eq!(
        healthy.revocation_state(&claims).await,
        RevocationState::Valid,
        "a healthy store with no record says the session is fine"
    );

    healthy
        .revoke(
            user_id,
            &RevocationRecord {
                at: chrono::Utc::now().timestamp() + 1,
                except_jti: None,
            },
            900,
        )
        .await;
    assert_eq!(
        healthy.revocation_state(&claims).await,
        RevocationState::Revoked
    );

    // A Redis nobody is listening on: the connection manager reconnects
    // forever, so every lookup errors or times out.
    let dead = redis::Client::open("redis://127.0.0.1:6399")
        .expect("client")
        .get_connection_manager()
        .await;
    if let Ok(conn) = dead {
        assert_eq!(
            RevocationStore(conn).revocation_state(&claims).await,
            RevocationState::Unknown,
            "an unreachable store is not the same answer as a valid session"
        );
    }

    let (status, _) = send(&app, "GET", "/api/users/me", Some(&token), None).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "the revoked session is refused on the healthy path"
    );
}

async fn emails_to(state: &crate::state::AppState, email: &str) -> Vec<(String, String)> {
    sqlx::query_as(
        "SELECT subject, text_body FROM outbound_emails WHERE to_address = $1 ORDER BY created_at",
    )
    .bind(email)
    .fetch_all(&state.pool)
    .await
    .expect("outbound emails")
}

async fn reset_tokens_of(state: &crate::state::AppState, email: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM password_reset_tokens t JOIN users u ON u.id = t.user_id WHERE u.email = $1",
    )
    .bind(email)
    .fetch_one(&state.pool)
    .await
    .expect("count")
}

async fn forgot(app: &Router, email: &str) {
    let (status, _) = send(
        app,
        "POST",
        "/api/auth/forgot-password",
        None,
        Some(json!({ "email": email })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_short_password_does_not_burn_the_reset_link(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_, email) = seed(&state, "short-reset", false).await;

    forgot(&app, &email).await;
    let (_, body) = emails_to(&state, &email).await.pop().expect("reset email");
    let token = body
        .split("token=")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .expect("token in the email")
        .to_string();

    let (status, _) = send(
        &app,
        "POST",
        "/api/auth/reset-password",
        None,
        Some(json!({ "token": token, "password": "short" })),
    )
    .await;
    assert!(status.is_client_error());
    assert_ne!(
        status,
        StatusCode::UNAUTHORIZED,
        "the password is what was wrong"
    );

    let (status, body) = send(
        &app,
        "POST",
        "/api/auth/reset-password",
        None,
        Some(json!({ "token": token, "password": "long-enough-now-123" })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the same link still works: {body:?}"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn forgetting_a_password_never_set_sends_the_invite_again(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "pending-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Pending WS").await;
    let invitee = format!("pending-{}@test.local", Uuid::new_v4().simple());
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/invites"),
        Some(&owner_token),
        Some(json!({ "email": invitee })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    forgot(&app, &invitee).await;

    let sent = emails_to(&state, &invitee).await;
    assert_eq!(sent.len(), 2, "the invite, then the invite again: {sent:?}");
    assert!(sent[1].0.starts_with("Join "), "{sent:?}");
    assert!(sent[1].1.contains("/invite/"));
    assert_eq!(
        reset_tokens_of(&state, &invitee).await,
        0,
        "a reset would not let them in"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_sso_account_is_told_to_use_sso_instead_of_a_reset(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let email = format!("sso-{}@test.local", Uuid::new_v4().simple());
    state
        .auth_service
        .repo()
        .create_sso_user(&email)
        .await
        .expect("sso user");

    forgot(&app, &email).await;

    let sent = emails_to(&state, &email).await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, "You sign in with SSO");
    assert_eq!(reset_tokens_of(&state, &email).await, 0);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_invite_to_a_mixed_case_address_registers_and_signs_in_in_any_case(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _) = seed(&state, "ws-owner", false).await;
    let workspace_id = seed_workspace(&state, owner_id, "Invite WS").await;

    let typed = unique_email("Ana.Case").replace("test.local", "Firma.Test");
    let stored = typed.to_lowercase();
    let pending = state
        .auth_service
        .repo()
        .create(&typed, None, None, false)
        .await
        .expect("create pending user");
    assert_eq!(pending.email, stored);
    let (_, token) = email_invite(
        &state,
        workspace_id,
        owner_id,
        pending.id,
        &typed,
        WorkspaceRole::Member,
    )
    .await;

    let (status, body) = send(
        &app,
        "POST",
        "/api/auth/complete-registration",
        None,
        Some(json!({ "token": token, "password": "newpassword123", "display_name": "Ana" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["user"]["email"], stored);

    for spelling in [typed.clone(), stored.clone(), typed.to_uppercase()] {
        let (status, body) = send(
            &app,
            "POST",
            "/api/auth/login",
            None,
            Some(json!({ "email": spelling, "password": "newpassword123" })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{spelling}: {body:?}");
    }
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_address_migration_stops_on_a_clash_and_lowercases_otherwise(pool: PgPool) {
    let migration = include_str!("../../../migrations/20240305000081_lowercase_emails.sql");
    sqlx::raw_sql(
        "ALTER TABLE users DROP CONSTRAINT users_email_lowercase;
         ALTER TABLE workspace_invites DROP CONSTRAINT workspace_invites_email_lowercase;",
    )
    .execute(&pool)
    .await
    .expect("undo the migration");
    let insert = |email: &'static str| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, Uuid>(
                "INSERT INTO users (email, status) VALUES ($1, 'active') RETURNING id",
            )
            .bind(email)
            .fetch_one(&pool)
            .await
            .expect("insert user")
        }
    };
    insert("Mixed@Case.Test").await;
    let twin = insert("mixed@case.test").await;
    insert("Alone@Case.Test").await;

    let refused = sqlx::raw_sql(migration)
        .execute(&pool)
        .await
        .expect_err("two spellings of one address stop it");
    let message = refused.to_string();
    assert!(
        message.contains("Mixed@Case.Test") && message.contains("mixed@case.test"),
        "{message}"
    );
    let untouched: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE email = 'Alone@Case.Test'")
            .fetch_one(&pool)
            .await
            .expect("count");
    assert_eq!(untouched, 1, "nothing changes when it stops");

    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(twin)
        .execute(&pool)
        .await
        .expect("resolve the clash");
    sqlx::raw_sql(migration)
        .execute(&pool)
        .await
        .expect("runs once the clash is gone");
    let emails: Vec<String> =
        sqlx::query_scalar("SELECT email FROM users WHERE email LIKE '%@case.test' ORDER BY email")
            .fetch_all(&pool)
            .await
            .expect("emails");
    assert_eq!(emails, vec!["alone@case.test", "mixed@case.test"]);
    assert!(
        sqlx::query("INSERT INTO users (email, status) VALUES ('Again@Case.Test', 'active')")
            .execute(&pool)
            .await
            .is_err(),
        "a mixed-case address is refused from then on"
    );
}
