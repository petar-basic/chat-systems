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
