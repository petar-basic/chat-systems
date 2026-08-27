use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

use super::common::*;
use crate::auth::totp;
use crate::config::AppConfig;

fn authenticator(state: &crate::state::AppState, secret: &str, email: &str) -> totp_rs::Totp {
    totp_rs::Builder::new()
        .with_algorithm(totp_rs::Algorithm::SHA1)
        .with_digits(6)
        .with_skew(1)
        .with_step_duration(30)
        .with_secret(totp_rs::Secret::try_from_base32(secret).expect("secret"))
        .with_issuer(Some(state.config.instance_name.clone()))
        .with_account_name(email.to_string())
        .build()
        .expect("totp")
}

/// The code an authenticator would be showing right now for this secret.
fn code_for(state: &crate::state::AppState, secret: &str, email: &str) -> String {
    authenticator(state, secret, email)
        .generate_current()
        .to_string()
}

/// The code the previous step produced. Skew still accepts it, so a request
/// carrying it is already one step past the step it belongs to.
fn code_from_the_previous_step(
    state: &crate::state::AppState,
    secret: &str,
    email: &str,
) -> String {
    let step = totp::current_step(chrono::Utc::now().timestamp()) - 1;
    authenticator(state, secret, email)
        .generate(step as u64 * 30)
        .to_string()
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
async fn a_code_works_once_and_not_twice(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_id, email, token) = seed_and_login(&app, &state, "totp-user", false).await;

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

    // The same code again: correct digits, already spent. The guard pins a code
    // to the step that produced it, so this holds whether the replay lands in
    // that step or in the next one.
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

#[test_macros::db_test(migrations = "../migrations")]
async fn a_code_stays_spent_after_the_clock_ticks_over(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_id, email, token) = seed_and_login(&app, &state, "totp-user", false).await;

    let (_, body) = send(&app, "POST", "/api/auth/totp/enrol", Some(&token), None).await;
    let secret = body["secret"].as_str().expect("secret").to_string();

    // Enrolling with a code the clock has already ticked past puts the replay
    // below in the window a shoulder-surfer would use — the digits are still
    // valid on skew, but their step is behind us — without waiting for it.
    let code = code_from_the_previous_step(&state, &secret, &email);
    let (status, _) = send(
        &app,
        "POST",
        "/api/auth/totp/confirm",
        Some(&token),
        Some(json!({ "code": code.clone() })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

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
        "a spent code must stay spent for the rest of its skew window"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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

#[test_macros::db_test(migrations = "../migrations")]
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
