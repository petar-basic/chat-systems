use axum::http::StatusCode;
use axum::Router;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;
use webauthn_authenticator_rs::softpasskey::SoftPasskey;
use webauthn_authenticator_rs::WebauthnAuthenticator;
use webauthn_rs::prelude::{
    CreationChallengeResponse, PublicKeyCredential, RequestChallengeResponse, Url,
};

use super::common::*;
use super::totp::enrol;
use crate::config::AppConfig;

pub(super) fn origin() -> Url {
    Url::parse("http://localhost").expect("origin")
}

pub(super) fn device() -> SoftPasskey {
    SoftPasskey::new(true)
}

pub(super) async fn add_passkey(app: &Router, token: &str, device: &mut SoftPasskey) -> String {
    let (status, body) = send(
        app,
        "POST",
        "/api/auth/passkeys/registration",
        Some(token),
        Some(json!({ "password": PASSWORD })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");

    let mut options = body["options"].clone();
    let selection = &mut options["publicKey"]["authenticatorSelection"];
    assert_eq!(
        selection["residentKey"], "required",
        "a passkey has to be discoverable to sign in with no email typed"
    );
    selection["requireResidentKey"] = json!(false);
    selection
        .as_object_mut()
        .expect("selection")
        .remove("residentKey");
    let options: CreationChallengeResponse = serde_json::from_value(options).expect("options");

    let credential = device
        .do_registration(origin(), options)
        .expect("registration");
    let credential_id = credential.id.clone();

    let (status, added) = send(
        app,
        "POST",
        "/api/auth/passkeys",
        Some(token),
        Some(json!({
            "challenge_id": body["challenge_id"],
            "name": "Laptop",
            "credential": credential,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{added:?}");
    assert_eq!(added["name"], "Laptop");
    credential_id
}

async fn start_sign_in(app: &Router) -> (Value, Value) {
    let (status, body) = send(app, "POST", "/api/auth/passkeys/sign-in/start", None, None).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    (body["challenge_id"].clone(), body["options"].clone())
}

fn assert_with(
    device: &mut SoftPasskey,
    mut options: Value,
    credential_id: &str,
    user_id: Uuid,
) -> PublicKeyCredential {
    options["publicKey"]["allowCredentials"] =
        json!([{ "type": "public-key", "id": credential_id }]);
    let options: RequestChallengeResponse = serde_json::from_value(options).expect("options");
    let mut assertion = device
        .do_authentication(origin(), options)
        .expect("authentication");
    assertion.response.user_handle = Some(user_id.as_bytes().to_vec());
    assertion
}

async fn sign_in(
    app: &Router,
    challenge_id: &Value,
    assertion: &PublicKeyCredential,
) -> (StatusCode, Value) {
    send(
        app,
        "POST",
        "/api/auth/passkeys/sign-in",
        None,
        Some(json!({ "challenge_id": challenge_id, "credential": assertion })),
    )
    .await
}

async fn password_login(app: &Router, email: &str) -> (StatusCode, Value) {
    send(
        app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({ "email": email, "password": PASSWORD })),
    )
    .await
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_passkey_signs_in_with_no_email_and_skips_the_totp_code(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (user_id, email, token) = seed_and_login(&app, &state, "passkey-user", false).await;
    enrol(&app, &state, &token, &email).await;
    let mut device = device();
    let credential_id = add_passkey(&app, &token, &mut device).await;

    let (status, body) = password_login(&app, &email).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"], "totp_or_passkey_required", "{body:?}");

    let (challenge_id, options) = start_sign_in(&app).await;
    let assertion = assert_with(&mut device, options, &credential_id, user_id);
    let (status, session) = sign_in(&app, &challenge_id, &assertion).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a passkey the device verified is a second factor in itself: {session:?}"
    );
    let access = session["access_token"].as_str().expect("access token");

    let (status, me) = send(&app, "GET", "/api/users/me", Some(access), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["id"], user_id.to_string());

    let (_, listed) = send(&app, "GET", "/api/auth/passkeys", Some(&token), None).await;
    assert!(
        listed["data"][0]["last_used_at"].is_string(),
        "the list says when a passkey was last used: {listed:?}"
    );

    let actions: Vec<String> = sqlx::query_scalar(
        "SELECT action FROM audit_log WHERE user_id = $1 AND action LIKE 'passkey.%' ORDER BY created_at",
    )
    .bind(user_id)
    .fetch_all(&state.pool)
    .await
    .expect("audit");
    assert_eq!(actions, ["passkey.added", "passkey.used"]);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_challenge_is_spent_on_first_use(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (user_id, _email, token) = seed_and_login(&app, &state, "passkey-replay", false).await;
    let mut device = device();
    let credential_id = add_passkey(&app, &token, &mut device).await;

    let (challenge_id, options) = start_sign_in(&app).await;
    let assertion = assert_with(&mut device, options, &credential_id, user_id);
    let (status, _) = sign_in(&app, &challenge_id, &assertion).await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = sign_in(&app, &challenge_id, &assertion).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "a replayed sign-in finds its challenge already spent"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_challenge_older_than_two_minutes_is_refused(pool: PgPool) {
    use redis::AsyncCommands;

    let (app, state) = app_and_state(pool).await;
    let (user_id, _email, token) = seed_and_login(&app, &state, "passkey-expired", false).await;
    let mut device = device();
    let credential_id = add_passkey(&app, &token, &mut device).await;

    let (challenge_id, options) = start_sign_in(&app).await;
    let key = format!(
        "passkey:challenge:{}",
        challenge_id.as_str().expect("challenge id")
    );
    let mut redis = state.redis.clone();
    let ttl: i64 = redis.ttl(&key).await.expect("ttl");
    assert!(
        (1..=120).contains(&ttl),
        "challenges live for two minutes, not {ttl}s"
    );
    let _: bool = redis.pexpire(&key, 1).await.expect("expire");
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;

    let assertion = assert_with(&mut device, options, &credential_id, user_id);
    let (status, _) = sign_in(&app, &challenge_id, &assertion).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_passkey_the_device_did_not_verify_signs_in_to_nothing(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (user_id, _email, token) = seed_and_login(&app, &state, "passkey-uv", false).await;
    let mut device = device();
    let credential_id = add_passkey(&app, &token, &mut device).await;

    let (challenge_id, mut options) = start_sign_in(&app).await;
    assert_eq!(options["publicKey"]["userVerification"], "required");
    options["publicKey"]["userVerification"] = json!("discouraged");
    let assertion = assert_with(&mut device, options, &credential_id, user_id);

    let (status, _) = sign_in(&app, &challenge_id, &assertion).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "a touch with no PIN or biometric is not a second factor"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn after_a_password_a_passkey_stands_in_for_the_code(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (user_id, email, token) = seed_and_login(&app, &state, "passkey-second", false).await;
    enrol(&app, &state, &token, &email).await;
    let mut device = device();
    add_passkey(&app, &token, &mut device).await;

    let (status, body) = send(
        &app,
        "POST",
        "/api/auth/passkeys/second-factor/start",
        None,
        Some(json!({ "email": email, "password": "not the password" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body:?}");

    let (status, body) = send(
        &app,
        "POST",
        "/api/auth/passkeys/second-factor/start",
        None,
        Some(json!({ "email": email, "password": PASSWORD })),
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
}

#[test_macros::db_test(migrations = "../migrations")]
async fn adding_or_removing_asks_for_the_password_first(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_user_id, email, token) = seed_and_login(&app, &state, "passkey-presence", false).await;

    let (status, _) = send(
        &app,
        "POST",
        "/api/auth/passkeys/registration",
        Some(&token),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = send(
        &app,
        "POST",
        "/api/auth/passkeys/registration",
        Some(&token),
        Some(json!({ "password": "not the password" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let mut device = device();
    add_passkey(&app, &token, &mut device).await;
    let (_, listed) = send(&app, "GET", "/api/auth/passkeys", Some(&token), None).await;
    let id = listed["data"][0]["id"].as_str().expect("id").to_string();

    let (status, _) = send(
        &app,
        "PATCH",
        &format!("/api/auth/passkeys/{id}"),
        Some(&token),
        Some(json!({ "name": "Phone" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/auth/passkeys/{id}/remove"),
        Some(&token),
        Some(json!({ "password": "not the password" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/auth/passkeys/{id}/remove"),
        Some(&token),
        Some(json!({ "password": PASSWORD })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (_, listed) = send(&app, "GET", "/api/auth/passkeys", Some(&token), None).await;
    assert_eq!(listed["data"], json!([]));
    let (status, _) = password_login(&app, &email).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "with the last passkey gone the password still signs in"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn sso_accounts_are_not_offered_passkeys(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (user_id, _email, token) = seed_and_login(&app, &state, "passkey-sso", false).await;
    sqlx::query("UPDATE users SET password_hash = NULL WHERE id = $1")
        .bind(user_id)
        .execute(&state.pool)
        .await
        .expect("make sso");

    let (status, _) = send(
        &app,
        "POST",
        "/api/auth/passkeys/registration",
        Some(&token),
        Some(json!({ "password": PASSWORD })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (_, listed) = send(&app, "GET", "/api/auth/passkeys", Some(&token), None).await;
    assert_eq!(listed["offered"], false, "{listed:?}");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_suspended_accounts_passkey_signs_in_to_nothing(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (user_id, _email, token) = seed_and_login(&app, &state, "passkey-suspended", false).await;
    let mut device = device();
    let credential_id = add_passkey(&app, &token, &mut device).await;
    sqlx::query("UPDATE users SET status = 'suspended' WHERE id = $1")
        .bind(user_id)
        .execute(&state.pool)
        .await
        .expect("suspend");

    let (challenge_id, options) = start_sign_in(&app).await;
    let assertion = assert_with(&mut device, options, &credential_id, user_id);
    let (status, _) = sign_in(&app, &challenge_id, &assertion).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_passkey_satisfies_an_instance_that_requires_admins_to_have_a_second_factor(
    pool: PgPool,
) {
    let (open_app, state) = app_and_state(pool.clone()).await;
    let (admin_id, admin_email, token) =
        seed_and_login(&open_app, &state, "passkey-admin", true).await;
    let mut device = device();
    let credential_id = add_passkey(&open_app, &token, &mut device).await;

    let config = AppConfig {
        require_admin_totp: true,
        ..test_config()
    };
    let (app, _) = app_and_state_with(pool, config).await;

    let (status, body) = password_login(&app, &admin_email).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"], "passkey_required", "{body:?}");

    let (challenge_id, options) = start_sign_in(&app).await;
    let assertion = assert_with(&mut device, options, &credential_id, admin_id);
    let (status, session) = sign_in(&app, &challenge_id, &assertion).await;
    assert_eq!(status, StatusCode::OK, "{session:?}");

    let (_, listed) = send(&app, "GET", "/api/auth/passkeys", Some(&token), None).await;
    let id = listed["data"][0]["id"].as_str().expect("id").to_string();
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/auth/passkeys/{id}/remove"),
        Some(&token),
        Some(json!({ "password": PASSWORD })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "the admin's only second factor stays until another takes its place"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_passkey_used_in_the_system_browser_signs_the_desktop_app_in(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (user_id, email, token) = seed_and_login(&app, &state, "passkey-desktop", false).await;
    enrol(&app, &state, &token, &email).await;
    let mut device = device();
    let credential_id = add_passkey(&app, &token, &mut device).await;

    let (challenge_id, options) = start_sign_in(&app).await;
    let assertion = assert_with(&mut device, options, &credential_id, user_id);
    let (status, body) = send(
        &app,
        "POST",
        "/api/auth/passkeys/sign-in/desktop",
        None,
        Some(json!({ "challenge_id": challenge_id, "credential": assertion })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert!(
        body.get("access_token").is_none(),
        "the browser hands the app a code, never a session of its own"
    );
    let code = body["code"].as_str().expect("code").to_string();

    let (status, exchanged) = send(
        &app,
        "POST",
        "/api/auth/oidc/exchange",
        None,
        Some(json!({ "code": code })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{exchanged:?}");
    assert_eq!(exchanged["landing"], "/app");

    let (status, _) = send(
        &app,
        "POST",
        "/api/auth/oidc/exchange",
        None,
        Some(json!({ "code": code })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "the code works once");
}
