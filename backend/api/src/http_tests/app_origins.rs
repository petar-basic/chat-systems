use axum::body::Body;
use axum::http::header::{
    ACCESS_CONTROL_ALLOW_CREDENTIALS, ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_REQUEST_METHOD,
    AUTHORIZATION, CONTENT_TYPE, COOKIE, ORIGIN, SET_COOKIE,
};
use axum::http::{HeaderMap, Request, StatusCode};
use axum::Router;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;

use super::avatar::{download_path, upload_photo, PNG};
use super::common::*;
use crate::middleware::{Claims, MEDIA_TOKEN};

const APP: &str = "tauri://localhost";

async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    headers: &[(axum::http::HeaderName, String)],
    body: Option<Value>,
) -> (StatusCode, HeaderMap, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    for (name, value) in headers {
        builder = builder.header(name, value);
    }
    let request = match body {
        Some(b) => builder
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(b.to_string()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    };
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (
        status,
        headers,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn preflight(app: &Router, origin: &str, path: &str) -> HeaderMap {
    let (_, headers, _) = call(
        app,
        "OPTIONS",
        path,
        &[
            (ORIGIN, origin.to_string()),
            (ACCESS_CONTROL_REQUEST_METHOD, "GET".to_string()),
        ],
        None,
    )
    .await;
    headers
}

fn bearer(token: &str) -> (axum::http::HeaderName, String) {
    (AUTHORIZATION, format!("Bearer {token}"))
}

fn from_app() -> (axum::http::HeaderName, String) {
    (ORIGIN, APP.to_string())
}

async fn app_sign_in(app: &Router, email: &str) -> Value {
    let (status, headers, body) = call(
        app,
        "POST",
        "/api/v1/auth/login",
        &[from_app()],
        Some(json!({ "email": email, "password": PASSWORD })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert!(
        headers.get(SET_COOKIE).is_none(),
        "the app is never handed a cookie"
    );
    body
}

async fn media_token(app: &Router, access: &str) -> String {
    let (status, _, body) = call(
        app,
        "POST",
        "/api/v1/auth/media-token",
        &[bearer(access)],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["expires_in"], 3600);
    body["token"].as_str().expect("media token").to_string()
}

async fn fetch_with_media(app: &Router, path: &str, token: &str) -> StatusCode {
    let (status, _, _) = call(app, "GET", &format!("{path}?media={token}"), &[], None).await;
    status
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_bundled_app_is_allowed_without_credentials(pool: PgPool) {
    let (app, _state) = app_and_state(pool).await;

    for origin in shared_common::cors::APP_ORIGINS.split(',') {
        let headers = preflight(&app, origin, "/api/v1/workspaces").await;
        assert_eq!(
            headers
                .get(ACCESS_CONTROL_ALLOW_ORIGIN)
                .and_then(|v| v.to_str().ok()),
            Some(origin),
            "{origin} may call the API"
        );
        assert!(
            headers.get(ACCESS_CONTROL_ALLOW_CREDENTIALS).is_none(),
            "{origin} is never allowed credentials"
        );
    }

    let web = preflight(&app, "http://localhost", "/api/v1/workspaces").await;
    assert_eq!(
        web.get(ACCESS_CONTROL_ALLOW_CREDENTIALS)
            .and_then(|v| v.to_str().ok()),
        Some("true"),
        "the web client keeps its cookies"
    );

    let stranger = preflight(&app, "https://evil.example", "/api/v1/workspaces").await;
    assert!(stranger.get(ACCESS_CONTROL_ALLOW_ORIGIN).is_none());
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_bundled_app_cannot_reach_the_machine_routes(pool: PgPool) {
    let (app, _state) = app_and_state(pool).await;
    for path in [
        "/api/v1/scim/v2/Users",
        "/api/v1/hooks/incoming/abc",
        "/api/v1/workflows/webhook/abc",
        "/api/v1/commands/responses/abc",
        "/api/v1/interactions/responses/abc",
    ] {
        let headers = preflight(&app, APP, path).await;
        assert!(
            headers.get(ACCESS_CONTROL_ALLOW_ORIGIN).is_none(),
            "{path} is not for the app"
        );
    }
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_app_signs_in_with_tokens_and_never_with_a_cookie(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_, email) = seed(&state, "app-sign-in", false).await;

    let session = app_sign_in(&app, &email).await;
    let access = session["access_token"].as_str().expect("access token");
    assert!(session["refresh_token"].is_string());

    let (status, _, _) = call(
        &app,
        "GET",
        "/api/v1/workspaces",
        &[from_app(), bearer(access)],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let cookie = (COOKIE, format!("access_token={access}"));
    let (status, _, _) = call(
        &app,
        "GET",
        "/api/v1/workspaces",
        &[from_app(), cookie.clone()],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "the app never sends one");

    let (status, _, _) = call(&app, "GET", "/api/v1/workspaces", &[cookie], None).await;
    assert_eq!(status, StatusCode::OK, "the web client still does");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn signing_out_with_the_refresh_token_ends_the_session(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_, email) = seed(&state, "app-sign-out", false).await;
    let session = app_sign_in(&app, &email).await;
    let access = session["access_token"].as_str().expect("access token");
    let refresh = session["refresh_token"].as_str().expect("refresh token");

    let (status, _, _) = call(
        &app,
        "POST",
        "/api/v1/auth/logout",
        &[from_app(), bearer(access)],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, renewed) = call(
        &app,
        "POST",
        "/api/v1/auth/refresh",
        &[from_app(), bearer(refresh)],
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "only the refresh token signs out: {renewed:?}"
    );
    let refresh = renewed["refresh_token"].as_str().expect("refresh token");

    let (status, _, _) = call(
        &app,
        "POST",
        "/api/v1/auth/logout",
        &[from_app(), bearer(refresh)],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = call(
        &app,
        "POST",
        "/api/v1/auth/refresh",
        &[from_app(), bearer(refresh)],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "the session is over");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_media_link_downloads_a_file_and_does_nothing_else(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_, email) = seed(&state, "media-alice", false).await;
    let session = app_sign_in(&app, &email).await;
    let alice = session["access_token"].as_str().expect("access token");
    let refresh = session["refresh_token"].as_str().expect("refresh token");
    let (_, _, stranger) = seed_and_login(&app, &state, "media-stranger", false).await;

    let (status, me) = upload_photo(&app, alice, PNG).await;
    assert_eq!(status, StatusCode::OK, "{me:?}");
    let path = download_path(me["avatar_url"].as_str().expect("avatar url"));

    let media = media_token(&app, alice).await;
    assert_eq!(
        fetch_with_media(&app, &path, &media).await,
        StatusCode::OK,
        "no cookie and no header needed"
    );
    assert_eq!(
        fetch_with_media(&app, &path, alice).await,
        StatusCode::UNAUTHORIZED,
        "an access token does not go in a link"
    );

    let (status, _, _) = call(&app, "GET", "/api/v1/workspaces", &[bearer(&media)], None).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "a media token is no session"
    );
    let (status, _, _) = call(&app, "POST", "/api/v1/auth/logout", &[bearer(&media)], None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = call(
        &app,
        "POST",
        "/api/v1/auth/refresh",
        &[bearer(refresh)],
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a leaked link cannot sign anyone out"
    );

    let theirs = media_token(&app, &stranger).await;
    assert_eq!(
        fetch_with_media(&app, &path, &theirs).await,
        StatusCode::FORBIDDEN,
        "a link opens only what its owner may see"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_media_link_dies_with_the_session(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (alice_id, _, alice) = seed_and_login(&app, &state, "media-revoked", false).await;
    let (status, me) = upload_photo(&app, &alice, PNG).await;
    assert_eq!(status, StatusCode::OK, "{me:?}");
    let path = download_path(me["avatar_url"].as_str().expect("avatar url"));
    let media = media_token(&app, &alice).await;

    crate::sessions::revoke(&state, alice_id, crate::sessions::SessionScope::All, "test")
        .await
        .expect("revoke");

    assert_eq!(
        fetch_with_media(&app, &path, &media).await,
        StatusCode::UNAUTHORIZED
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_expired_media_link_is_refused(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (alice_id, _, alice) = seed_and_login(&app, &state, "media-expired", false).await;
    let (status, me) = upload_photo(&app, &alice, PNG).await;
    assert_eq!(status, StatusCode::OK, "{me:?}");
    let path = download_path(me["avatar_url"].as_str().expect("avatar url"));

    let hour_ago = chrono::Utc::now() - chrono::Duration::hours(1);
    let expired = state
        .config
        .jwt_keys
        .encode(&Claims {
            sub: alice_id,
            email: String::new(),
            is_instance_admin: false,
            iat: (hour_ago - chrono::Duration::hours(1)).timestamp(),
            exp: hour_ago.timestamp(),
            jti: None,
            token_type: MEDIA_TOKEN.to_string(),
        })
        .expect("encode");

    assert_eq!(
        fetch_with_media(&app, &path, &expired).await,
        StatusCode::UNAUTHORIZED
    );
}
