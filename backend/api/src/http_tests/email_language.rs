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

fn escaped(text: &str) -> String {
    text.replace('&', "&#38;")
        .replace('<', "&#60;")
        .replace('>', "&#62;")
        .replace('"', "&#34;")
        .replace('\'', "&#39;")
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_japanese_invite_writes_the_names_it_was_given_as_plain_text(pool: PgPool) {
    let inviter = r#"<img src=x onerror="alert(1)"> {{name}}"#;
    let workspace = r#"<img src=x onerror="alert(1)"> {{instance}}"#;
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, owner_token) = seed_and_login(&app, &state, "ja-names", false).await;
    sqlx::query("UPDATE users SET display_name = $2 WHERE id = $1")
        .bind(owner_id)
        .bind(inviter)
        .execute(&state.pool)
        .await
        .expect("set the name");
    let ws_id = seed_workspace(&state, owner_id, workspace).await;
    prefer(&state, owner_id, "ja").await;
    let invitee = unique_email("ja-names-invitee");
    post(
        &app,
        &format!("/api/workspaces/{ws_id}/invites"),
        Some(&owner_token),
        json!({ "email": invitee }),
        None,
    )
    .await;
    sent(&state, &invitee, 1).await;

    let (subject, text, html): (String, String, String) = sqlx::query_as(
        "SELECT subject, text_body, html_body FROM outbound_emails WHERE to_address = $1",
    )
    .bind(&invitee)
    .fetch_one(&state.pool)
    .await
    .expect("the invite");

    let body =
        format!("{inviter} さんが、Test のワークスペース「{workspace}」にあなたを招待しました。");
    assert_eq!(subject, format!("Test の {workspace} への招待"));
    assert!(text.lines().any(|line| line == body), "{text}");
    assert!(
        text.lines()
            .any(|line| line == format!("{workspace} への招待")),
        "{text}"
    );
    assert!(!html.contains("<img src=x"), "{html}");
    assert!(!html.contains("onerror=\""), "{html}");
    assert!(html.contains(&format!(">{}</p>", escaped(&body))), "{html}");
    assert!(
        html.contains(&format!(">{} への招待</h1>", escaped(workspace))),
        "{html}"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_japanese_invite_request_writes_what_the_requester_wrote_as_plain_text(pool: PgPool) {
    let requester = r#"<img src=x onerror="alert(1)"> {{workspace}}"#;
    let reason = r#"<img src=x onerror="alert(1)"> {{emails}}"#;
    let (app, state) = app_and_state(pool).await;
    let (owner_id, owner_address, owner_token) =
        seed_and_login(&app, &state, "ja-asked", false).await;
    let ws_id = seed_workspace(&state, owner_id, "Ja Requests").await;
    let vendor = seed_channel(&state, ws_id, owner_id, "vendor", false).await;
    let (member_id, _, member_token) = seed_and_login(&app, &state, "ja-asker", false).await;
    add_ws_member(&state, ws_id, member_id, "member").await;
    for (id, name) in [(owner_id, "Ana"), (member_id, requester)] {
        sqlx::query("UPDATE users SET display_name = $2 WHERE id = $1")
            .bind(id)
            .bind(name)
            .execute(&state.pool)
            .await
            .expect("set the name");
    }
    prefer(&state, owner_id, "ja").await;
    let newcomer = unique_email("ja-asked-for");
    let (status, request) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/invite-requests"),
        Some(&member_token),
        Some(json!({
            "emails": [newcomer],
            "role": "guest",
            "guest_kind": "single",
            "channel_ids": [vendor],
            "reason": reason,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{request}");
    sent(&state, &owner_address, 1).await;

    let (subject, text, html): (String, String, String) = sqlx::query_as(
        "SELECT subject, text_body, html_body FROM outbound_emails WHERE to_address = $1",
    )
    .bind(&owner_address)
    .fetch_one(&state.pool)
    .await
    .expect("the request");

    let body = format!(
        "{requester} さんから、{newcomer} を Ja Requests にゲストとして招待してほしいという依頼がありました。"
    );
    let because = format!("理由: {reason}");
    assert_eq!(
        subject,
        format!("{requester} さんから Ja Requests への招待の依頼")
    );
    for line in [
        body.as_str(),
        because.as_str(),
        "招待の依頼",
        "30日以内に誰も対応しなかった場合、この依頼は期限切れになります。",
    ] {
        assert!(text.lines().any(|l| l == line), "{line} in {text}");
    }
    assert!(!html.contains("<img src=x"), "{html}");
    assert!(!html.contains("onerror=\""), "{html}");
    assert!(html.contains(&format!(">{}</p>", escaped(&body))), "{html}");
    assert!(
        html.contains(&format!(">{}</p>", escaped(&because))),
        "{html}"
    );
    assert!(html.contains(">依頼を確認</a>"), "{html}");

    let id = request["id"].as_str().expect("request id");
    let (status, approved) = send(
        &app,
        "POST",
        &format!("/api/workspaces/{ws_id}/invite-requests/{id}/approve"),
        Some(&owner_token),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{approved}");
    sent(&state, &newcomer, 1).await;

    let (text, html): (String, String) =
        sqlx::query_as("SELECT text_body, html_body FROM outbound_emails WHERE to_address = $1")
            .bind(&newcomer)
            .fetch_one(&state.pool)
            .await
            .expect("the invite");
    let invited = format!(
        "{requester} さんの依頼を受けて、Ana さんが Test のワークスペース「Ja Requests」にあなたを招待しました。"
    );
    assert!(text.lines().any(|l| l == invited), "{text}");
    assert!(!html.contains("<img src=x"), "{html}");
    assert!(!html.contains("onerror=\""), "{html}");
    assert!(
        html.contains(&format!(">{}</p>", escaped(&invited))),
        "{html}"
    );
}
