use super::common::*;
use axum::http::{HeaderMap, StatusCode};
use sqlx::PgPool;
use uuid::Uuid;

use crate::authenticate_ws;

#[sqlx::test(migrations = "../migrations")]
async fn livez_ok(pool: PgPool) {
    let app = app(manager(pool).await);
    assert_eq!(get_status(&app, "/livez").await, StatusCode::OK);
}

#[sqlx::test(migrations = "../migrations")]
async fn readyz_ok_when_db_and_redis_live(pool: PgPool) {
    let app = app(manager(pool).await);
    assert_eq!(get_status(&app, "/readyz").await, StatusCode::OK);
}

#[sqlx::test(migrations = "../migrations")]
async fn ws_upgrade_rejects_missing_origin(pool: PgPool) {
    let app = app(manager(pool.clone()).await);
    let user = seed_user(&pool).await;
    let token = mint_token(user, "access");
    let status = ws_upgrade_status_with_origin(&app, Some(&token), None).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "an upgrade without an Origin header must be rejected"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn ws_upgrade_rejects_disallowed_origin(pool: PgPool) {
    let app = app(manager(pool.clone()).await);
    let user = seed_user(&pool).await;
    let token = mint_token(user, "access");
    let status =
        ws_upgrade_status_with_origin(&app, Some(&token), Some("https://evil.example")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn ws_upgrade_allowed_origin_and_token_passes_auth_gates(pool: PgPool) {
    let app = app(manager(pool.clone()).await);
    let user = seed_user(&pool).await;
    let token = mint_token(user, "access");
    let status = ws_upgrade_status(&app, Some(&token)).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "with a valid origin and token the request must clear the auth gates and fail only on \
         the missing hyper upgrade extension (oneshot has no real socket)"
    );
}

#[test]
fn ws_auth_rejects_missing_cookie() {
    assert!(authenticate_ws(&HeaderMap::new(), JWT_SECRET).is_err());
}

#[test]
fn ws_auth_rejects_undecodable_token() {
    assert!(authenticate_ws(&cookie_header("garbage.token.value"), JWT_SECRET).is_err());
}

#[test]
fn ws_auth_rejects_token_signed_with_wrong_secret() {
    let other = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &serde_json::json!({ "sub": Uuid::new_v4(), "exp": 9_999_999_999i64, "token_type": "access" }),
        &jsonwebtoken::EncodingKey::from_secret(b"a-different-secret-key-that-is-32+chars"),
    )
    .unwrap();
    assert!(authenticate_ws(&cookie_header(&other), JWT_SECRET).is_err());
}

#[test]
fn ws_auth_rejects_non_access_token_type() {
    let token = mint_token(Uuid::new_v4(), "reset");
    assert!(
        authenticate_ws(&cookie_header(&token), JWT_SECRET).is_err(),
        "a reset token must not open a socket"
    );
}

#[test]
fn ws_auth_accepts_genuine_access_token() {
    let user = Uuid::new_v4();
    let token = mint_token(user, "access");
    let (sub, _exp) = authenticate_ws(&cookie_header(&token), JWT_SECRET)
        .expect("access token should be accepted");
    assert_eq!(sub, user);
}

#[test]
fn ws_auth_accepts_subprotocol_token() {
    let user = Uuid::new_v4();
    let token = mint_token(user, "access");
    let (sub, _exp) = authenticate_ws(&protocol_header(&token), JWT_SECRET)
        .expect("subprotocol access token should be accepted");
    assert_eq!(sub, user);
}

#[test]
fn ws_auth_rejects_non_access_subprotocol_token() {
    let token = mint_token(Uuid::new_v4(), "refresh");
    assert!(
        authenticate_ws(&protocol_header(&token), JWT_SECRET).is_err(),
        "a refresh token must not open a socket via subprotocol"
    );
}
