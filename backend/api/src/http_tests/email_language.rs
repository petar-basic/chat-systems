use std::time::Duration;

use axum::http::header::ACCEPT_LANGUAGE;
use axum::http::StatusCode;
use axum::Router;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;
use crate::notifications::text;
use crate::profile::models::Language;
use crate::state::AppState;

async fn prefer(state: &AppState, user_id: Uuid, language: &str) {
    sqlx::query(
        "UPDATE users SET preferences = preferences || jsonb_build_object('language', $2::text) WHERE id = $1",
    )
    .bind(user_id)
    .bind(language)
    .execute(&state.pool)
    .await
    .expect("set the language");
}

async fn post(
    app: &Router,
    path: &str,
    token: Option<&str>,
    body: serde_json::Value,
    accept: Option<&str>,
) {
    let (status, body) = send_with_header(
        app,
        "POST",
        path,
        token,
        Some(body),
        accept.map(|value| (ACCEPT_LANGUAGE, value)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

async fn forgot(app: &Router, email: &str, accept: Option<&str>) {
    post(
        app,
        "/api/auth/forgot-password",
        None,
        json!({ "email": email }),
        accept,
    )
    .await;
}

async fn ask_for_code(app: &Router, email: &str, accept: Option<&str>) {
    post(
        app,
        "/api/auth/email-code",
        None,
        json!({ "email": email }),
        accept,
    )
    .await;
}

async fn sent(state: &AppState, email: &str, count: usize) -> Vec<(String, String)> {
    for _ in 0..250 {
        let rows: Vec<(String, Option<String>)> = sqlx::query_as(
            "SELECT subject, html_body FROM outbound_emails WHERE to_address = $1 ORDER BY created_at",
        )
        .bind(email)
        .fetch_all(&state.pool)
        .await
        .expect("outbound emails");
        if rows.len() >= count {
            return rows
                .into_iter()
                .map(|(subject, html)| (subject, lang_of(&html.expect("an HTML part"))))
                .collect();
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("{count} emails to {email} never arrived");
}

fn lang_of(html: &str) -> String {
    html.split("<html lang=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("a lang attribute")
        .to_string()
}

fn langs(emails: &[(String, String)]) -> Vec<&str> {
    emails.iter().map(|(_, lang)| lang.as_str()).collect()
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_japanese_setting_writes_the_reset_and_the_code_in_japanese_whatever_the_browser(
    pool: PgPool,
) {
    let (app, state) = app_and_state(pool).await;
    let (user_id, email) = seed(&state, "ja-setting", false).await;
    prefer(&state, user_id, "ja").await;

    forgot(&app, &email, Some("en-US,en;q=0.9")).await;
    sent(&state, &email, 1).await;
    ask_for_code(&app, &email, None).await;
    let emails = sent(&state, &email, 2).await;

    assert_eq!(langs(&emails), ["ja", "ja"]);
    let japanese = text::words(Language::Ja);
    assert_eq!(
        emails[0].0,
        japanese.get("email.reset.subject", &[("instance", "Test")])
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn while_the_setting_follows_the_browser_the_request_decides(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (_, email) = seed(&state, "ja-browser", false).await;

    forgot(&app, &email, Some("ja-JP,ja;q=0.9,en;q=0.8")).await;
    sent(&state, &email, 1).await;
    ask_for_code(&app, &email, Some("ja")).await;
    sent(&state, &email, 2).await;
    forgot(&app, &email, Some("fr-FR, en;q=0.5")).await;
    sent(&state, &email, 3).await;
    ask_for_code(&app, &email, None).await;
    let emails = sent(&state, &email, 4).await;

    assert_eq!(langs(&emails), ["ja", "ja", "en", "en"]);
    assert!(
        emails[3].0.ends_with("is your Test sign-in code"),
        "{}",
        emails[3].0
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_invite_is_in_the_language_of_the_admin_who_sent_it(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "ja-owner", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Ja WS").await;
    let invites = format!("/api/workspaces/{ws_id}/invites");
    let invitee = |n: u8| format!("ja-invitee-{n}-{}@test.local", Uuid::new_v4().simple());
    let (first, second, third) = (invitee(1), invitee(2), invitee(3));

    prefer(&state, owner_id, "ja").await;
    post(
        &app,
        &invites,
        Some(&owner_token),
        json!({ "email": first }),
        Some("en"),
    )
    .await;

    prefer(&state, owner_id, "browser").await;
    post(
        &app,
        &invites,
        Some(&owner_token),
        json!({ "email": second }),
        Some("ja-JP"),
    )
    .await;
    post(
        &app,
        &invites,
        Some(&owner_token),
        json!({ "email": third }),
        None,
    )
    .await;

    assert_eq!(langs(&sent(&state, &first, 1).await), ["ja"]);
    assert_eq!(langs(&sent(&state, &second, 1).await), ["ja"]);
    assert_eq!(langs(&sent(&state, &third, 1).await), ["en"]);

    forgot(&app, &first, Some("en-GB")).await;
    let again = sent(&state, &first, 2).await;
    assert!(again[1].0.starts_with("Join "), "{again:?}");
    assert_eq!(
        langs(&again),
        ["ja", "en"],
        "the invite again follows that request"
    );
}
