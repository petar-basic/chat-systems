use super::common::*;
use axum::http::{HeaderMap, StatusCode};
use sqlx::PgPool;
use uuid::Uuid;

use crate::authenticate_ws;

#[test_macros::db_test(migrations = "../migrations")]
async fn livez_ok(pool: PgPool) {
    let app = app(manager(pool).await);
    assert_eq!(get_status(&app, "/livez").await, StatusCode::OK);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn readyz_ok_when_db_and_redis_live(pool: PgPool) {
    let app = app(manager(pool).await);
    assert_eq!(get_status(&app, "/readyz").await, StatusCode::OK);
}

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
async fn ws_upgrade_rejects_disallowed_origin(pool: PgPool) {
    let app = app(manager(pool.clone()).await);
    let user = seed_user(&pool).await;
    let token = mint_token(user, "access");
    let status =
        ws_upgrade_status_with_origin(&app, Some(&token), Some("https://evil.example")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[test_macros::db_test(migrations = "../migrations")]
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
    assert!(authenticate_ws(&HeaderMap::new(), &gateway_keys()).is_err());
}

#[test]
fn ws_auth_rejects_undecodable_token() {
    assert!(authenticate_ws(&cookie_header("garbage.token.value"), &gateway_keys()).is_err());
}

fn access_claims() -> serde_json::Value {
    serde_json::json!({ "sub": Uuid::new_v4(), "exp": 9_999_999_999i64, "token_type": "access" })
}

#[test]
fn ws_auth_rejects_token_signed_with_another_key_under_the_same_id() {
    let impostor = shared_common::session_keys::SigningKeys::parse(
        "test:MC4CAQAwBQYDK2VwBCIEIMmVeJctnPI4cTDrkYWTEXJUa2oijgBEjxlvG4x9LXe/",
    )
    .expect("keys")
    .encode(&access_claims())
    .expect("encode");
    assert!(authenticate_ws(&cookie_header(&impostor), &gateway_keys()).is_err());
}

#[test]
fn ws_auth_rejects_a_key_it_does_not_list() {
    let unknown = shared_common::session_keys::SigningKeys::parse(
        "elsewhere:MC4CAQAwBQYDK2VwBCIEIMmVeJctnPI4cTDrkYWTEXJUa2oijgBEjxlvG4x9LXe/",
    )
    .expect("keys")
    .encode(&access_claims())
    .expect("encode");
    assert!(authenticate_ws(&cookie_header(&unknown), &gateway_keys()).is_err());
}

#[test]
fn ws_auth_rejects_hs256() {
    let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256);
    header.kid = Some("test".into());
    let forged = jsonwebtoken::encode(
        &header,
        &access_claims(),
        &jsonwebtoken::EncodingKey::from_secret(b"integration-test-secret-key-0123456789-abcdef"),
    )
    .unwrap();
    assert!(authenticate_ws(&cookie_header(&forged), &gateway_keys()).is_err());
}

#[test]
fn ws_auth_rejects_non_access_token_type() {
    let token = mint_token(Uuid::new_v4(), "reset");
    assert!(
        authenticate_ws(&cookie_header(&token), &gateway_keys()).is_err(),
        "a reset token must not open a socket"
    );
}

#[test]
fn ws_auth_accepts_genuine_access_token() {
    let user = Uuid::new_v4();
    let token = mint_token(user, "access");
    let claims = authenticate_ws(&cookie_header(&token), &gateway_keys())
        .expect("access token should be accepted");
    assert_eq!(claims.sub, user);
}

#[test]
fn ws_auth_accepts_subprotocol_token() {
    let user = Uuid::new_v4();
    let token = mint_token(user, "access");
    let claims = authenticate_ws(&protocol_header(&token), &gateway_keys())
        .expect("subprotocol access token should be accepted");
    assert_eq!(claims.sub, user);
}

#[test]
fn ws_auth_rejects_non_access_subprotocol_token() {
    let token = mint_token(Uuid::new_v4(), "refresh");
    assert!(
        authenticate_ws(&protocol_header(&token), &gateway_keys()).is_err(),
        "a refresh token must not open a socket via subprotocol"
    );
}
