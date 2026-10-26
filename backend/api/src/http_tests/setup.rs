use axum::http::StatusCode;
use serde_json::{json, Value};
use sqlx::PgPool;

use super::common::*;
use crate::state::AppState;

async fn setup_token(state: &AppState) -> String {
    crate::setup::announce(state).await.expect("announce");
    let nonce = state
        .setup_repo
        .current_nonce()
        .await
        .expect("nonce")
        .expect("a fresh instance has a setup nonce");
    crate::setup::token_for(&state.config.jwt_secret, &nonce)
}

fn request(token: &str, email: &str) -> Value {
    json!({
        "token": token,
        "display_name": "First Admin",
        "email": email,
        "password": "a-long-enough-password",
        "workspace_name": "Acme",
    })
}

async fn setup_required(app: &axum::Router) -> bool {
    let (status, body) = send(app, "GET", "/api/instance/info", None, None).await;
    assert_eq!(status, StatusCode::OK);
    body["setup_required"].as_bool().expect("setup_required")
}

#[test_macros::db_test(migrations = "../migrations")]
async fn setup_is_refused_without_the_token(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let token = setup_token(&state).await;
    assert!(setup_required(&app).await);

    for wrong in ["", "not-the-token", &token[..token.len() - 1]] {
        let (status, _) = send(
            &app,
            "POST",
            "/api/setup",
            None,
            Some(request(wrong, &unique_email("first"))),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{wrong:?} is not the token");
    }
    assert!(setup_required(&app).await, "nothing was created");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn setup_is_refused_once_an_instance_admin_exists(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let token = setup_token(&state).await;
    seed(&state, "admin", true).await;
    assert!(!setup_required(&app).await);

    let (status, _) = send(
        &app,
        "POST",
        "/api/setup",
        None,
        Some(request(&token, &unique_email("late"))),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    crate::setup::announce(&state).await.expect("announce");
    assert!(
        state.setup_repo.current_nonce().await.unwrap().is_none(),
        "an instance with an admin has no setup token left"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_setup_token_works_once(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let token = setup_token(&state).await;
    assert_eq!(
        setup_token(&state).await,
        token,
        "every start prints the same token until setup is done"
    );

    let email = unique_email("first");
    let mut weak = request(&token, &email);
    weak["password"] = json!("short");
    let (status, _) = send(&app, "POST", "/api/setup", None, Some(weak)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (status, body) = send(
        &app,
        "POST",
        "/api/setup",
        None,
        Some(request(&token, &email)),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a refused attempt keeps the token: {body}"
    );
    assert_eq!(body["session"]["user"]["email"], json!(email));
    assert_eq!(body["session"]["user"]["is_instance_admin"], json!(true));
    assert!(!setup_required(&app).await);

    let access = body["session"]["access_token"].as_str().unwrap();
    let workspace_id = body["workspace_id"].as_str().unwrap();
    let (status, channels) = send(
        &app,
        "GET",
        &format!("/api/workspaces/{workspace_id}/channels"),
        Some(access),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(channels["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["name"] == "general"));

    login(&app, &email, "a-long-enough-password").await;

    let (status, _) = send(
        &app,
        "POST",
        "/api/setup",
        None,
        Some(request(&token, &unique_email("second"))),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    sqlx::query("UPDATE users SET is_instance_admin = FALSE")
        .execute(&state.pool)
        .await
        .unwrap();
    let (status, _) = send(
        &app,
        "POST",
        "/api/setup",
        None,
        Some(request(&token, &unique_email("third"))),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a used token stays used even if the instance loses its admin"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn the_test_email_reports_a_refused_connection(pool: PgPool) {
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let config = crate::config::AppConfig {
        smtp_host: "127.0.0.1".into(),
        smtp_port: port,
        smtp_tls_mode: crate::config::SmtpTlsMode::None,
        ..test_config()
    };
    let (app, state) = app_and_state_with(pool, config).await;
    let (_, admin_email, admin) = seed_and_login(&app, &state, "admin", true).await;
    let (_, _, member) = seed_and_login(&app, &state, "member", false).await;

    let (status, _) = send(&app, "POST", "/api/admin/email/test", Some(&member), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, body) = send(&app, "POST", "/api/admin/email/test", Some(&admin), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["sent_to"], json!(admin_email));
    let error = body["error"].as_str().expect("the failure is reported");
    assert!(
        error.contains(&format!(
            "Nothing accepted the connection at 127.0.0.1:{port}"
        )),
        "{error}"
    );
}
