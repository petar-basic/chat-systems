use axum::http::StatusCode;

use super::common::*;
use crate::audit::ClientIp;
use crate::auth::models::UserStatus;
use crate::auth::oidc::{OidcSettings, Provisioning, VerifiedIdentity};
use crate::auth::oidc_routes::link_or_create;
use crate::config::AppConfig;

fn settings(provisioning: Provisioning) -> OidcSettings {
    OidcSettings {
        issuer: "http://localhost:8090/chat".into(),
        client_id: "chat-systems".into(),
        client_secret: "dev-secret".into(),
        redirect_url: "http://localhost/api/auth/oidc/callback".into(),
        provisioning,
    }
}

fn identity(subject: &str, email: &str) -> VerifiedIdentity {
    VerifiedIdentity {
        subject: subject.into(),
        email: email.into(),
    }
}

fn configured() -> AppConfig {
    AppConfig {
        oidc_issuer: "http://localhost:8090/chat".into(),
        oidc_client_id: "chat-systems".into(),
        oidc_client_secret: "dev-secret".into(),
        ..test_config()
    }
}

#[sqlx::test(migrations = "../migrations")]
async fn starting_sso_on_an_instance_without_it_is_a_clear_no(pool: sqlx::PgPool) {
    let (app, _state) = app_and_state(pool).await;
    let (status, _) = send(&app, "GET", "/api/auth/oidc/start", None, None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../migrations")]
async fn sso_can_be_configured_and_still_switched_off(pool: sqlx::PgPool) {
    let config = AppConfig {
        oidc_provisioning: "disabled".into(),
        ..configured()
    };
    let (app, _state) = app_and_state_with(pool, config).await;
    let (status, _) = send(&app, "GET", "/api/auth/oidc/start", None, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

/// The callback is the half an attacker can reach directly, so arriving without
/// the handle this server issued has to fail before any code is exchanged.
#[sqlx::test(migrations = "../migrations")]
async fn a_callback_that_did_not_start_here_is_refused(pool: sqlx::PgPool) {
    let (app, _state) = app_and_state_with(pool, configured()).await;
    let (status, _) = send(
        &app,
        "GET",
        "/api/auth/oidc/callback?code=stolen&state=guessed",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn a_provider_error_never_becomes_a_session(pool: sqlx::PgPool) {
    let (app, _state) = app_and_state_with(pool, configured()).await;
    let (status, _) = send(
        &app,
        "GET",
        "/api/auth/oidc/callback?error=access_denied",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../migrations")]
async fn signing_in_links_the_existing_account_and_closes_the_password_door(pool: sqlx::PgPool) {
    let (_app, state) = app_and_state(pool).await;
    let (user_id, email) = seed(&state, "sso", false).await;

    let user = link_or_create(
        &state,
        &settings(Provisioning::InviteOnly),
        &identity("subject-1", &email),
        &ClientIp(None),
    )
    .await
    .expect("existing accounts sign in even when nothing may be created");

    assert_eq!(user.id, user_id);
    assert!(
        user.password_hash.is_none(),
        "an SSO account keeps no second way in"
    );

    let linked: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM oauth_accounts WHERE user_id = $1 AND provider_id = 'subject-1'",
    )
    .bind(user_id)
    .fetch_one(&state.pool)
    .await
    .expect("count links");
    assert_eq!(linked, 1);

    let audited: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM audit_log WHERE action = 'sso.linked'")
            .fetch_one(&state.pool)
            .await
            .expect("count audit");
    assert_eq!(audited, 1);
}

/// A break-glass local admin has to survive: locking the only admin out of an
/// instance whose provider is down is worse than the shadow-password risk.
#[sqlx::test(migrations = "../migrations")]
async fn an_instance_admin_keeps_their_password(pool: sqlx::PgPool) {
    let (_app, state) = app_and_state(pool).await;
    let (_id, email) = seed(&state, "admin", true).await;

    let user = link_or_create(
        &state,
        &settings(Provisioning::InviteOnly),
        &identity("subject-admin", &email),
        &ClientIp(None),
    )
    .await
    .expect("link");

    assert!(user.password_hash.is_some());
}

#[sqlx::test(migrations = "../migrations")]
async fn invite_only_will_not_create_an_account_for_a_stranger(pool: sqlx::PgPool) {
    let (_app, state) = app_and_state(pool).await;

    let result = link_or_create(
        &state,
        &settings(Provisioning::InviteOnly),
        &identity("subject-new", "stranger@elsewhere.test"),
        &ClientIp(None),
    )
    .await;

    assert!(matches!(
        result,
        Err(shared_common::errors::AppError::Forbidden(_))
    ));
}

#[sqlx::test(migrations = "../migrations")]
async fn the_allowlist_provisions_an_active_account_without_a_password(pool: sqlx::PgPool) {
    let (_app, state) = app_and_state(pool).await;
    let policy = Provisioning::parse("domain_allowlist", "allowed.test");

    let user = link_or_create(
        &state,
        &settings(policy),
        &identity("subject-new", "newcomer@allowed.test"),
        &ClientIp(None),
    )
    .await
    .expect("provision");

    assert_eq!(user.email, "newcomer@allowed.test");
    assert_eq!(user.status, UserStatus::Active);
    assert!(user.password_hash.is_none());
}

/// Email at the provider is editable; the subject is not. Following the subject
/// is what stops a renamed account becoming a second account.
#[sqlx::test(migrations = "../migrations")]
async fn the_subject_follows_the_person_when_their_email_changes(pool: sqlx::PgPool) {
    let (_app, state) = app_and_state(pool).await;
    let (user_id, email) = seed(&state, "renamed", false).await;
    let policy = Provisioning::parse("domain_allowlist", "test.local");

    link_or_create(
        &state,
        &settings(policy.clone()),
        &identity("stable-subject", &email),
        &ClientIp(None),
    )
    .await
    .expect("first sign-in");

    let again = link_or_create(
        &state,
        &settings(policy),
        &identity("stable-subject", "renamed@test.local"),
        &ClientIp(None),
    )
    .await
    .expect("second sign-in");

    assert_eq!(again.id, user_id, "same person, not a new account");
    let users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&state.pool)
        .await
        .expect("count users");
    assert_eq!(users, 1);
}

#[sqlx::test(migrations = "../migrations")]
async fn a_suspended_account_cannot_sign_in_around_the_suspension(pool: sqlx::PgPool) {
    let (_app, state) = app_and_state(pool).await;
    let (user_id, email) = seed(&state, "suspended", false).await;
    sqlx::query("UPDATE users SET status = 'suspended' WHERE id = $1")
        .bind(user_id)
        .execute(&state.pool)
        .await
        .expect("suspend");

    let result = link_or_create(
        &state,
        &settings(Provisioning::InviteOnly),
        &identity("subject-suspended", &email),
        &ClientIp(None),
    )
    .await;

    assert!(matches!(
        result,
        Err(shared_common::errors::AppError::Unauthorized(_))
    ));
}
