use axum::http::StatusCode;
use axum::Router;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;
use webauthn_authenticator_rs::WebauthnAuthenticator;
use webauthn_rs::prelude::RequestChallengeResponse;

use super::common::*;
use super::passkeys::{add_passkey, device, origin};
use super::totp::enrol;
use crate::config::AppConfig;
use crate::state::AppState;

async fn ask(app: &Router, email: &str) -> (StatusCode, Value) {
    send(
        app,
        "POST",
        "/api/auth/email-code",
        None,
        Some(json!({ "email": email })),
    )
    .await
}

async fn verify(app: &Router, email: &str, code: &str) -> (StatusCode, Value) {
    send(
        app,
        "POST",
        "/api/auth/email-code/verify",
        None,
        Some(json!({ "email": email, "code": code })),
    )
    .await
}

async fn emails_to(state: &AppState, email: &str) -> Vec<(String, String)> {
    sqlx::query_as(
        "SELECT subject, text_body FROM outbound_emails WHERE to_address = $1 ORDER BY created_at",
    )
    .bind(email)
    .fetch_all(&state.pool)
    .await
    .expect("outbound emails")
}

async fn wait_for_emails(state: &AppState, email: &str, count: usize) -> Vec<(String, String)> {
    for _ in 0..250 {
        let sent = emails_to(state, email).await;
        if sent.len() >= count {
            return sent;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("{count} emails to {email} never arrived");
}

async fn code_number(state: &AppState, email: &str, n: usize) -> String {
    let (subject, body) = wait_for_emails(state, email, n).await.swap_remove(n - 1);
    let code = subject.split(' ').next().expect("code").to_string();
    assert_eq!(code.len(), 6, "{subject}");
    assert!(code.chars().all(|c| c.is_ascii_digit()), "{subject}");
    assert!(body.contains(&code), "{body}");
    code
}

fn another_code(code: &str) -> String {
    let last = code.as_bytes()[5] - b'0';
    format!("{}{}", &code[..5], (last + 1) % 10)
}

async fn actions_of(state: &AppState, user_id: Uuid) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT action FROM audit_log WHERE user_id = $1 AND action LIKE 'email_code.%' ORDER BY created_at",
    )
    .bind(user_id)
    .fetch_all(&state.pool)
    .await
    .expect("audit")
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_emailed_code_signs_in_once(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (user_id, email) = seed(&state, "code-once", false).await;

    let (status, info) = send(&app, "GET", "/api/instance/info", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(info["email_code_sign_in"], true);

    let (status, body) = ask(&app, &email).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let code = code_number(&state, &email, 1).await;

    let (status, session) = verify(&app, &email, &code).await;
    assert_eq!(status, StatusCode::OK, "{session:?}");
    let access = session["access_token"].as_str().expect("access token");
    let (status, me) = send(&app, "GET", "/api/users/me", Some(access), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["id"], user_id.to_string());

    let (status, _) = verify(&app, &email, &code).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "a used code is spent");
    assert_eq!(
        actions_of(&state, user_id).await,
        ["email_code.sent", "email_code.used"]
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_answer_is_the_same_for_an_address_with_no_account(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_, email) = seed(&state, "code-known", false).await;
    let nobody = unique_email("code-nobody");
    let sso = unique_email("code-sso");
    state
        .auth_service
        .repo()
        .create_sso_user(&sso)
        .await
        .expect("sso user");

    let known = ask(&app, &email).await;
    let unknown = ask(&app, &nobody).await;
    let single_sign_on = ask(&app, &sso).await;
    assert_eq!(known.0, StatusCode::OK);
    assert_eq!(known, unknown);
    assert_eq!(known, single_sign_on);

    let sent = wait_for_emails(&state, &sso, 1).await;
    assert!(emails_to(&state, &nobody).await.is_empty());
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, "You sign in with SSO");

    let (status, _) = verify(&app, &nobody, "123456").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_sixth_try_is_refused_even_with_the_right_code(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (user_id, email) = seed(&state, "code-tries", false).await;
    ask(&app, &email).await;
    let code = code_number(&state, &email, 1).await;
    let wrong = another_code(&code);

    for _ in 0..5 {
        let (status, _) = verify(&app, &email, &wrong).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
    let (status, _) = verify(&app, &email, &code).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "five tries is all a code gets"
    );
    assert_eq!(
        actions_of(&state, user_id).await,
        ["email_code.sent", "email_code.exhausted"]
    );
}

async fn failures_of(state: &AppState, user_id: Uuid) -> i32 {
    sqlx::query_scalar("SELECT failures FROM email_sign_in_codes WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(&state.pool)
        .await
        .expect("failures")
}

async fn set_failures(state: &AppState, user_id: Uuid, failures: i32, hours_ago: i32) {
    sqlx::query(
        "UPDATE email_sign_in_codes SET failures = $2, failures_since = NOW() - make_interval(hours => $3) WHERE user_id = $1",
    )
    .bind(user_id)
    .bind(failures)
    .bind(hours_ago)
    .execute(&state.pool)
    .await
    .expect("set failures");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_twenty_first_wrong_code_in_a_day_is_refused_even_with_a_fresh_code(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (user_id, email) = seed(&state, "code-ceiling", false).await;
    ask(&app, &email).await;
    let first = code_number(&state, &email, 1).await;
    let (status, _) = verify(&app, &email, &another_code(&first)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(failures_of(&state, user_id).await, 1);

    ask(&app, &email).await;
    let fresh = code_number(&state, &email, 2).await;
    assert_eq!(
        failures_of(&state, user_id).await,
        1,
        "a new code does not wipe the day's count"
    );
    set_failures(&state, user_id, 19, 1).await;
    let (status, _) = verify(&app, &email, &another_code(&fresh)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(failures_of(&state, user_id).await, 20);

    let (status, _) = verify(&app, &email, &fresh).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "after twenty wrong codes in a day even the right one is refused"
    );

    let (status, body) = ask(&app, &email).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "sent");
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert_eq!(
        emails_to(&state, &email).await.len(),
        2,
        "and no new code is sent"
    );

    set_failures(&state, user_id, 20, 25).await;
    let (status, session) = verify(&app, &email, &fresh).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a day later codes work again: {session:?}"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn guessing_codes_leaves_the_password_alone(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_, email) = seed(&state, "code-guess", false).await;
    for _ in 0..state.config.login_attempts_per_email {
        verify(&app, &email, "000000").await;
    }
    let (status, _) = verify(&app, &email, "000000").await;
    assert_eq!(
        status,
        StatusCode::TOO_MANY_REQUESTS,
        "guessing codes has a limit of its own"
    );

    login(&app, &email, PASSWORD).await;
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_pending_account_whose_invite_expired_is_told_so(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "expired-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Expired WS").await;
    let invitee = unique_email("expired-invitee");
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/invites"),
        Some(&owner_token),
        Some(json!({ "email": invitee })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    sqlx::query(
        "UPDATE workspace_invites SET expires_at = NOW() - INTERVAL '1 day' WHERE email = $1",
    )
    .bind(&invitee)
    .execute(&state.pool)
    .await
    .expect("expire the invite");

    let (status, body) = ask(&app, &invitee).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "sent");

    let sent = wait_for_emails(&state, &invitee, 2).await;
    assert_eq!(sent[1].0, "Your invitation has expired");
    assert!(sent[1].1.contains("Expired WS"), "{sent:?}");
    assert!(!sent[1].1.contains("/invite/"), "{sent:?}");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_code_older_than_ten_minutes_is_refused(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (user_id, email) = seed(&state, "code-old", false).await;
    ask(&app, &email).await;
    let code = code_number(&state, &email, 1).await;

    let lifetime: f64 = sqlx::query_scalar(
        "SELECT EXTRACT(EPOCH FROM expires_at - created_at)::float8 FROM email_sign_in_codes WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_one(&state.pool)
    .await
    .expect("lifetime");
    assert_eq!(lifetime, 600.0);

    sqlx::query(
        "UPDATE email_sign_in_codes SET created_at = created_at - INTERVAL '11 minutes', expires_at = expires_at - INTERVAL '11 minutes' WHERE user_id = $1",
    )
    .bind(user_id)
    .execute(&state.pool)
    .await
    .expect("age the code");

    let (status, _) = verify(&app, &email, &code).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_new_code_replaces_the_old_one(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_, email) = seed(&state, "code-new", false).await;
    ask(&app, &email).await;
    let first = code_number(&state, &email, 1).await;
    ask(&app, &email).await;
    let second = code_number(&state, &email, 2).await;

    if first != second {
        let (status, _) = verify(&app, &email, &first).await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "the old code stops working"
        );
    }
    let (status, _) = verify(&app, &email, &second).await;
    assert_eq!(status, StatusCode::OK);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn asking_too_often_is_rate_limited(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_, email) = seed(&state, "code-limit", false).await;

    for _ in 0..5 {
        let (status, _) = ask(&app, &email).await;
        assert_eq!(status, StatusCode::OK);
    }
    let (status, _) = ask(&app, &email).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_account_with_totp_is_asked_for_its_code_after_the_emailed_one(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (user_id, email, token) = seed_and_login(&app, &state, "code-totp", false).await;
    let recovery = enrol(&app, &state, &token, &email).await;
    ask(&app, &email).await;
    let code = code_number(&state, &email, 1).await;

    let (status, body) = verify(&app, &email, &code).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"], "totp_required", "{body:?}");

    let (status, session) = send(
        &app,
        "POST",
        "/api/auth/email-code/verify",
        None,
        Some(json!({ "email": email, "code": code, "totp_code": recovery[0] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{session:?}");
    assert_eq!(session["user"]["id"], user_id.to_string());
}

#[test_macros::db_test(migrations = "../migrations")]
async fn after_an_emailed_code_a_passkey_stands_in_for_the_totp_code(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (user_id, email, token) = seed_and_login(&app, &state, "code-passkey", false).await;
    enrol(&app, &state, &token, &email).await;
    let mut device = device();
    add_passkey(&app, &token, &mut device).await;
    ask(&app, &email).await;
    let code = code_number(&state, &email, 1).await;

    let (status, body) = verify(&app, &email, &code).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"], "totp_or_passkey_required", "{body:?}");

    let (status, body) = send(
        &app,
        "POST",
        "/api/auth/passkeys/second-factor/start",
        None,
        Some(json!({ "email": email, "email_code": code })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let options: RequestChallengeResponse =
        serde_json::from_value(body["options"].clone()).expect("options");
    let assertion = device
        .do_authentication(origin(), options)
        .expect("authentication");

    let (status, session) = send(
        &app,
        "POST",
        "/api/auth/passkeys/second-factor",
        None,
        Some(json!({ "challenge_id": body["challenge_id"], "credential": assertion })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{session:?}");
    assert_eq!(session["user"]["id"], user_id.to_string());

    let (status, _) = verify(&app, &email, &code).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "the passkey spent the code"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_instance_admin_can_turn_the_option_off(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_, _, admin) = seed_and_login(&app, &state, "code-admin", true).await;
    let (_, email) = seed(&state, "code-switch", false).await;

    let (status, settings) = send(
        &app,
        "PATCH",
        "/api/admin/settings",
        Some(&admin),
        Some(json!({ "email_code_sign_in": false })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{settings:?}");
    assert_eq!(settings["email_code_sign_in"], false);
    assert_eq!(settings["email_configured"], true);

    let (_, info) = send(&app, "GET", "/api/instance/info", None, None).await;
    assert_eq!(info["email_code_sign_in"], false);
    let (status, _) = ask(&app, &email).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(emails_to(&state, &email).await.is_empty());

    let (status, _) = send(
        &app,
        "PATCH",
        "/api/admin/settings",
        Some(&admin),
        Some(json!({ "email_code_sign_in": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, info) = send(&app, "GET", "/api/instance/info", None, None).await;
    assert_eq!(info["email_code_sign_in"], true);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_instance_with_no_smtp_does_not_offer_the_option(pool: PgPool) {
    let config = AppConfig {
        smtp_host: String::new(),
        ..test_config()
    };
    let (app, state) = app_and_state_with(pool, config).await;
    let (_, email) = seed(&state, "code-nosmtp", false).await;

    let (_, info) = send(&app, "GET", "/api/instance/info", None, None).await;
    assert_eq!(info["email_code_sign_in"], false);
    let (status, _) = ask(&app, &email).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
