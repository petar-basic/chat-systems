use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

use super::common::*;
use crate::auth::totp;
use crate::config::AppConfig;

/// The code an authenticator would be showing right now for this secret.
fn code_for(state: &crate::state::AppState, secret: &str, email: &str) -> String {
    let bytes = totp_rs::Secret::Encoded(secret.to_string())
        .to_bytes()
        .expect("secret");
    let totp = totp_rs::TOTP::new(
        totp_rs::Algorithm::SHA1,
        6,
        1,
        30,
        bytes,
        Some(state.config.instance_name.clone()),
        email.to_string(),
    )
    .expect("totp");
    totp.generate_current().expect("code")
}

async fn enrol(
    app: &axum::Router,
    state: &crate::state::AppState,
    token: &str,
    email: &str,
) -> Vec<String> {
    let (status, body) = send(app, "POST", "/api/auth/totp/enrol", Some(token), None).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let secret = body["secret"].as_str().expect("secret").to_string();

    let (status, confirmed) = send(
        app,
        "POST",
        "/api/auth/totp/confirm",
        Some(token),
        Some(json!({ "code": code_for(state, &secret, email) })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{confirmed:?}");

    let codes: Vec<String> = confirmed["recovery_codes"]
        .as_array()
        .expect("recovery codes")
        .iter()
        .map(|c| c.as_str().unwrap_or_default().to_string())
        .collect();
    std::mem::forget(secret.clone());
    codes
}

#[sqlx::test(migrations = "../migrations")]
async fn enrolment_needs_a_working_code_before_it_counts(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_id, email, token) = seed_and_login(&app, &state, "totp-user", false).await;

    let (status, body) = send(&app, "POST", "/api/auth/totp/enrol", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert!(body["provisioning_uri"]
        .as_str()
        .expect("uri")
        .starts_with("otpauth://totp/"));

    let (status, _) = send(
        &app,
        "POST",
        "/api/auth/totp/confirm",
        Some(&token),
        Some(json!({ "code": "000000" })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "a wrong code confirms nothing"
    );

    // Still not enforced: an unconfirmed enrolment must not lock anybody out.
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
        StatusCode::OK,
        "an abandoned enrolment locks nobody out"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn once_enrolled_the_password_alone_is_not_enough(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_id, email, token) = seed_and_login(&app, &state, "totp-user", false).await;
    enrol(&app, &state, &token, &email).await;

    let (status, body) = send(
        &app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({ "email": email, "password": PASSWORD })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "the client is told to ask for a code, not for the password again: {body:?}"
    );

    let (status, _) = send(
        &app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({ "email": email, "password": PASSWORD, "totp_code": "000000" })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "a wrong code is a wrong login"
    );
}

/// Waits out the tail of a step when there is not enough of it left for two
/// requests. Without this the test is a coin flip: crossing the boundary between
/// confirming and replaying means the second call claims a newer step, which is
/// the guard working rather than failing.
async fn wait_for_a_fresh_step() {
    let seconds_into_step = chrono::Utc::now().timestamp() % 30;
    let remaining = 30 - seconds_into_step;
    if remaining < 5 {
        tokio::time::sleep(std::time::Duration::from_secs(remaining as u64 + 1)).await;
    }
}

#[sqlx::test(migrations = "../migrations")]
async fn a_code_works_once_and_not_twice(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_id, email, token) = seed_and_login(&app, &state, "totp-user", false).await;

    wait_for_a_fresh_step().await;

    let (_, body) = send(&app, "POST", "/api/auth/totp/enrol", Some(&token), None).await;
    let secret = body["secret"].as_str().expect("secret").to_string();
    let code = code_for(&state, &secret, &email);
    let (status, _) = send(
        &app,
        "POST",
        "/api/auth/totp/confirm",
        Some(&token),
        Some(json!({ "code": code.clone() })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // The same code, inside the same window: correct digits, already spent.
    let (status, _) = send(
        &app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({ "email": email, "password": PASSWORD, "totp_code": code })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "a replayed code must not work while it is still valid"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn a_recovery_code_works_once(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_id, email, token) = seed_and_login(&app, &state, "totp-user", false).await;
    let codes = enrol(&app, &state, &token, &email).await;
    assert_eq!(codes.len(), totp::RECOVERY_CODE_COUNT);

    let (status, body) = send(
        &app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({ "email": email, "password": PASSWORD, "totp_code": codes[0] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "the way back in: {body:?}");

    let (status, _) = send(
        &app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({ "email": email, "password": PASSWORD, "totp_code": codes[0] })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "and only once");

    let stored: Option<String> =
        sqlx::query_scalar("SELECT code_hash FROM totp_recovery_codes LIMIT 1")
            .fetch_optional(&state.pool)
            .await
            .expect("query");
    let stored = stored.expect("a stored code");
    assert!(
        !codes.iter().any(|c| c == &stored),
        "recovery codes are hashed, not kept in the clear"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn removing_the_factor_needs_a_current_code(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_id, email, token) = seed_and_login(&app, &state, "totp-user", false).await;

    let (_, body) = send(&app, "POST", "/api/auth/totp/enrol", Some(&token), None).await;
    let secret = body["secret"].as_str().expect("secret").to_string();
    send(
        &app,
        "POST",
        "/api/auth/totp/confirm",
        Some(&token),
        Some(json!({ "code": code_for(&state, &secret, &email) })),
    )
    .await;

    let (status, _) = send(
        &app,
        "POST",
        "/api/auth/totp/disable",
        Some(&token),
        Some(json!({ "code": "000000" })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "a stolen session must not be able to strip the factor that would stop it"
    );
}

#[sqlx::test(migrations = "../migrations")]
async fn an_instance_that_requires_it_refuses_an_admin_without_one(pool: PgPool) {
    let config = AppConfig {
        require_admin_totp: true,
        ..test_config()
    };
    let (app, state) = app_and_state_with(pool, config).await;

    let (_admin_id, admin_email) = seed(&state, "totp-admin", true).await;
    let (status, _) = send(
        &app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({ "email": admin_email, "password": PASSWORD })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "an admin who can delete any workspace does not get in on a password alone"
    );

    let (_user_id, user_email) = seed(&state, "totp-plain", false).await;
    let (status, _) = send(
        &app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({ "email": user_email, "password": PASSWORD })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "everyone else is unaffected");
}

#[sqlx::test(migrations = "../migrations")]
async fn every_second_factor_event_is_audited(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_id, email, token) = seed_and_login(&app, &state, "totp-user", false).await;
    enrol(&app, &state, &token, &email).await;

    send(
        &app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({ "email": email, "password": PASSWORD, "totp_code": "000000" })),
    )
    .await;

    for action in ["totp.enrolled", "totp.failed"] {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_log WHERE action = $1")
            .bind(action)
            .fetch_one(&state.pool)
            .await
            .expect("count");
        assert!(count >= 1, "{action} is on the record");
    }
}
