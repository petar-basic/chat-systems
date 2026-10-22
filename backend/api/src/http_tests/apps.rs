use std::sync::Arc;

use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};
use sqlx::PgPool;

use super::common::*;
use crate::integrations::service::{self, AppUnfurl};

async fn fake_atlassian() -> String {
    async fn token(Json(body): Json<Value>) -> (StatusCode, Json<Value>) {
        let ok = match body["grant_type"].as_str() {
            Some("authorization_code") => {
                body["code"] == "good-code"
                    && body["code_verifier"]
                        .as_str()
                        .is_some_and(|v| v.len() >= 43)
            }
            Some("refresh_token") => body["refresh_token"] == "rt-1",
            _ => false,
        };
        if !ok {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "invalid_grant" })),
            );
        }
        (
            StatusCode::OK,
            Json(json!({ "access_token": "at-1", "refresh_token": "rt-1", "expires_in": 3600 })),
        )
    }
    fn authorized(headers: &HeaderMap) -> bool {
        headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v == "Bearer at-1")
    }
    async fn sites(headers: HeaderMap) -> (StatusCode, Json<Value>) {
        if !authorized(&headers) {
            return (StatusCode::UNAUTHORIZED, Json(json!({})));
        }
        (
            StatusCode::OK,
            Json(json!([{ "id": "cloud-1", "url": "https://acme.atlassian.net" }])),
        )
    }
    async fn me(headers: HeaderMap) -> (StatusCode, Json<Value>) {
        if !authorized(&headers) {
            return (StatusCode::UNAUTHORIZED, Json(json!({})));
        }
        (
            StatusCode::OK,
            Json(json!({ "name": "Ana", "email": "ana@acme.test" })),
        )
    }
    async fn issue(
        Path((cloud, key)): Path<(String, String)>,
        headers: HeaderMap,
    ) -> (StatusCode, Json<Value>) {
        if !authorized(&headers) {
            return (StatusCode::UNAUTHORIZED, Json(json!({})));
        }
        if cloud != "cloud-1" || key != "PT-262" {
            return (StatusCode::NOT_FOUND, Json(json!({})));
        }
        (
            StatusCode::OK,
            Json(json!({ "fields": {
                "summary": "Focus Axon planning",
                "issuetype": { "name": "Story" },
                "status": { "name": "In Progress", "statusCategory": { "key": "indeterminate" } },
                "priority": { "name": "Medium" },
                "assignee": { "displayName": "Lazar" }
            }})),
        )
    }
    let router = Router::new()
        .route("/oauth/token", post(token))
        .route("/oauth/token/accessible-resources", get(sites))
        .route("/me", get(me))
        .route("/ex/jira/{cloud}/rest/api/3/issue/{key}", get(issue));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    format!("http://{addr}")
}

struct Setup {
    app: axum::Router,
    state: Arc<crate::state::AppState>,
    owner: uuid::Uuid,
    owner_token: String,
    ws_id: uuid::Uuid,
}

async fn setup(pool: PgPool) -> Setup {
    let base = fake_atlassian().await;
    let config = crate::config::AppConfig {
        atlassian_auth_url: base.clone(),
        atlassian_api_url: base,
        ..test_config()
    };
    let (app, state) = app_and_state_with(pool, config).await;
    let (owner, _email, owner_token) = seed_and_login(&app, &state, "apps-owner", false).await;
    let ws_id = seed_workspace(&state, owner, &format!("ws-{}", uuid::Uuid::new_v4())).await;
    Setup {
        app,
        state,
        owner,
        owner_token,
        ws_id,
    }
}

async fn configure(s: &Setup, token: &str, body: Value) -> (StatusCode, Value) {
    send(
        &s.app,
        "PUT",
        &format!("/api/workspaces/{}/apps/jira", s.ws_id),
        Some(token),
        Some(body),
    )
    .await
}

async fn list(s: &Setup, token: &str) -> Value {
    let (status, body) = send(
        &s.app,
        "GET",
        &format!("/api/workspaces/{}/apps", s.ws_id),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    body["data"][0].clone()
}

async fn connect(s: &Setup, token: &str) -> String {
    let (status, body) = send(
        &s.app,
        "POST",
        &format!("/api/workspaces/{}/apps/jira/connect", s.ws_id),
        Some(token),
        Some(json!({ "return_to": "/app/somewhere" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let url = reqwest::Url::parse(body["authorize_url"].as_str().expect("url")).expect("parse");
    assert_eq!(
        url.query_pairs()
            .find(|(k, _)| k == "client_id")
            .map(|(_, v)| v.into_owned()),
        Some("client-1".into())
    );
    assert!(url.query_pairs().any(|(k, _)| k == "code_challenge"));
    url.query_pairs()
        .find(|(k, _)| k == "state")
        .map(|(_, v)| v.into_owned())
        .expect("state")
}

async fn complete(s: &Setup, token: &str, state: &str) -> (StatusCode, Value) {
    send(
        &s.app,
        "POST",
        "/api/apps/connections",
        Some(token),
        Some(json!({ "code": "good-code", "state": state })),
    )
    .await
}

#[test_macros::db_test(migrations = "../migrations")]
async fn only_an_admin_sets_up_an_app_and_the_secret_stays_sealed(pool: PgPool) {
    let s = setup(pool).await;
    let (member, _email, member_token) =
        seed_and_login(&s.app, &s.state, "apps-member", false).await;
    add_ws_member(&s.state, s.ws_id, member, "member").await;

    let (status, _) = configure(
        &s,
        &member_token,
        json!({ "client_id": "client-1", "client_secret": "shh", "enabled": true }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = configure(
        &s,
        &s.owner_token,
        json!({ "client_id": "client-1", "enabled": true }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "the first setup needs a secret"
    );
    let (status, body) = configure(
        &s,
        &s.owner_token,
        json!({ "client_id": "client-1", "client_secret": "shh", "enabled": true }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");

    let status = list(&s, &member_token).await;
    assert_eq!(status["configured"], json!(true));
    assert_eq!(status["client_id"], json!("client-1"));
    assert!(status["redirect_url"]
        .as_str()
        .unwrap()
        .ends_with("/oauth/apps/callback"));
    assert!(!status.to_string().contains("shh"));

    let stored: Vec<u8> = sqlx::query_scalar(
        "SELECT client_secret_encrypted FROM workspace_apps WHERE workspace_id = $1",
    )
    .bind(s.ws_id)
    .fetch_one(&s.state.pool)
    .await
    .expect("stored");
    assert_ne!(stored, b"shh".to_vec());

    let (status, _) = configure(
        &s,
        &s.owner_token,
        json!({ "client_id": "client-1", "enabled": false }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a later edit keeps the stored secret"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_member_connects_once_and_cards_use_their_account(pool: PgPool) {
    let s = setup(pool).await;
    configure(
        &s,
        &s.owner_token,
        json!({ "client_id": "client-1", "client_secret": "shh", "enabled": true }),
    )
    .await;
    let (member, _email, member_token) =
        seed_and_login(&s.app, &s.state, "apps-member", false).await;
    add_ws_member(&s.state, s.ws_id, member, "member").await;

    let oauth_state = connect(&s, &member_token).await;
    let (status, body) = complete(&s, &member_token, &oauth_state).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["external_account"], json!("Ana (ana@acme.test)"));
    assert_eq!(body["return_to"], json!("/app/somewhere"));
    assert_eq!(
        list(&s, &member_token).await["connected_account"],
        json!("Ana (ana@acme.test)")
    );
    let (status, _) = complete(&s, &member_token, &oauth_state).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "a state is good for one use"
    );

    let issue = "https://acme.atlassian.net/browse/PT-262";
    match service::unfurl(&s.state, s.ws_id, member, issue)
        .await
        .expect("unfurl")
    {
        AppUnfurl::Card(card) => {
            assert_eq!(card.title, "Focus Axon planning");
            assert_eq!(card.subtitle.as_deref(), Some("Story PT-262"));
        }
        _ => panic!("the connected poster gets a card"),
    }
    assert!(matches!(
        service::unfurl(&s.state, s.ws_id, s.owner, issue)
            .await
            .expect("unfurl"),
        AppUnfurl::Connect(_)
    ));
    assert!(matches!(
        service::unfurl(
            &s.state,
            s.ws_id,
            member,
            "https://other.atlassian.net/browse/PT-262"
        )
        .await
        .expect("unfurl"),
        AppUnfurl::Unclaimed
    ));
    assert!(matches!(
        service::unfurl(&s.state, s.ws_id, member, "https://example.com/page")
            .await
            .expect("unfurl"),
        AppUnfurl::Unclaimed
    ));

    let (status, _) = send(
        &s.app,
        "DELETE",
        &format!("/api/workspaces/{}/apps/jira/connection", s.ws_id),
        Some(&member_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        list(&s, &member_token).await["connected_account"],
        json!(null)
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_connection_cannot_be_finished_by_someone_else(pool: PgPool) {
    let s = setup(pool).await;
    configure(
        &s,
        &s.owner_token,
        json!({ "client_id": "client-1", "client_secret": "shh", "enabled": true }),
    )
    .await;
    let (member, _email, member_token) =
        seed_and_login(&s.app, &s.state, "apps-member", false).await;
    add_ws_member(&s.state, s.ws_id, member, "member").await;

    let oauth_state = connect(&s, &member_token).await;
    let (status, _) = complete(&s, &s.owner_token, &oauth_state).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_disabled_app_previews_nothing_and_cannot_be_connected(pool: PgPool) {
    let s = setup(pool).await;
    configure(
        &s,
        &s.owner_token,
        json!({ "client_id": "client-1", "client_secret": "shh", "enabled": false }),
    )
    .await;
    let (status, _) = send(
        &s.app,
        "POST",
        &format!("/api/workspaces/{}/apps/jira/connect", s.ws_id),
        Some(&s.owner_token),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(matches!(
        service::unfurl(
            &s.state,
            s.ws_id,
            s.owner,
            "https://acme.atlassian.net/browse/PT-262"
        )
        .await
        .expect("unfurl"),
        AppUnfurl::Unclaimed
    ));
}

#[test_macros::db_test(migrations = "../migrations")]
async fn never_ask_again_silences_the_prompt_until_the_member_connects(pool: PgPool) {
    let s = setup(pool).await;
    configure(
        &s,
        &s.owner_token,
        json!({ "client_id": "client-1", "client_secret": "shh", "enabled": true }),
    )
    .await;
    let (member, _email, member_token) =
        seed_and_login(&s.app, &s.state, "apps-member", false).await;
    add_ws_member(&s.state, s.ws_id, member, "member").await;
    let issue = "https://acme.atlassian.net/browse/PT-262";
    let prompts = format!("/api/workspaces/{}/apps/jira/prompts", s.ws_id);

    let (status, _) = send(&s.app, "PUT", &prompts, Some(&member_token), None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send(&s.app, "PUT", &prompts, Some(&member_token), None).await;
    assert_eq!(status, StatusCode::OK, "turning prompts off twice is fine");
    assert_eq!(list(&s, &member_token).await["prompts_off"], json!(true));
    assert_eq!(list(&s, &s.owner_token).await["prompts_off"], json!(false));
    assert!(matches!(
        service::unfurl(&s.state, s.ws_id, member, issue)
            .await
            .expect("unfurl"),
        AppUnfurl::Unclaimed
    ));
    assert!(matches!(
        service::unfurl(&s.state, s.ws_id, s.owner, issue)
            .await
            .expect("unfurl"),
        AppUnfurl::Connect(_)
    ));

    let (status, _) = send(&s.app, "DELETE", &prompts, Some(&member_token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(matches!(
        service::unfurl(&s.state, s.ws_id, member, issue)
            .await
            .expect("unfurl"),
        AppUnfurl::Connect(_)
    ));

    send(&s.app, "PUT", &prompts, Some(&member_token), None).await;
    let oauth_state = connect(&s, &member_token).await;
    assert_eq!(
        complete(&s, &member_token, &oauth_state).await.0,
        StatusCode::OK
    );
    assert_eq!(
        list(&s, &member_token).await["prompts_off"],
        json!(false),
        "connecting turns the prompts back on"
    );
    assert!(matches!(
        service::unfurl(&s.state, s.ws_id, member, issue)
            .await
            .expect("unfurl"),
        AppUnfurl::Card(_)
    ));

    let (status, _) = send(
        &s.app,
        "PUT",
        &format!("/api/workspaces/{}/apps/nope/prompts", s.ws_id),
        Some(&member_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn suspending_a_user_drops_their_connections(pool: PgPool) {
    let s = setup(pool).await;
    configure(
        &s,
        &s.owner_token,
        json!({ "client_id": "client-1", "client_secret": "shh", "enabled": true }),
    )
    .await;
    let oauth_state = connect(&s, &s.owner_token).await;
    assert_eq!(
        complete(&s, &s.owner_token, &oauth_state).await.0,
        StatusCode::OK
    );

    let (_admin, _email, admin_token) = seed_and_login(&s.app, &s.state, "apps-admin", true).await;
    let (status, _) = send(
        &s.app,
        "POST",
        &format!("/api/admin/users/{}/suspend", s.owner),
        Some(&admin_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM app_connections WHERE user_id = $1")
        .bind(s.owner)
        .fetch_one(&s.state.pool)
        .await
        .expect("count");
    assert_eq!(left, 0);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn apps_outsider_forbidden(pool: PgPool) {
    let s = setup(pool).await;
    let (_outsider, _email, outsider_token) =
        seed_and_login(&s.app, &s.state, "outsider", false).await;
    let (status, _) = send(
        &s.app,
        "GET",
        &format!("/api/workspaces/{}/apps", s.ws_id),
        Some(&outsider_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = send(
        &s.app,
        "PUT",
        &format!("/api/workspaces/{}/apps/jira/prompts", s.ws_id),
        Some(&outsider_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = send(
        &s.app,
        "PUT",
        &format!("/api/workspaces/{}/apps/jira/prompts", s.ws_id),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

async fn fake_github() -> String {
    async fn token(Json(body): Json<Value>) -> Json<Value> {
        if body["code"] == "good-code"
            && body["code_verifier"]
                .as_str()
                .is_some_and(|v| v.len() >= 43)
        {
            return Json(
                json!({ "access_token": "gh-1", "refresh_token": "ghr-1", "expires_in": 28800 }),
            );
        }
        Json(json!({ "error": "bad_verification_code" }))
    }
    fn authorized(headers: &HeaderMap) -> bool {
        headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v == "Bearer gh-1")
    }
    async fn user(headers: HeaderMap) -> (StatusCode, Json<Value>) {
        if !authorized(&headers) {
            return (StatusCode::UNAUTHORIZED, Json(json!({})));
        }
        (
            StatusCode::OK,
            Json(json!({ "login": "ana", "name": "Ana" })),
        )
    }
    async fn public_pull() -> Json<Value> {
        Json(json!({
            "title": "Ship it", "state": "open", "draft": false, "merged": false,
            "user": { "login": "ana" }, "additions": 1, "deletions": 0, "changed_files": 1, "labels": []
        }))
    }
    async fn private_issue(headers: HeaderMap) -> (StatusCode, Json<Value>) {
        if !authorized(&headers) {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "message": "Not Found" })),
            );
        }
        (
            StatusCode::OK,
            Json(
                json!({ "title": "Secret bug", "state": "open", "user": { "login": "ana" }, "labels": [] }),
            ),
        )
    }
    async fn issue_nine() -> Json<Value> {
        Json(
            json!({ "title": "Really an issue", "state": "open", "user": { "login": "ana" }, "labels": [] }),
        )
    }
    let router = Router::new()
        .route("/login/oauth/access_token", post(token))
        .route("/user", get(user))
        .route("/repos/acme/rocket/pulls/42", get(public_pull))
        .route("/repos/acme/secret/issues/7", get(private_issue))
        .route("/repos/acme/rocket/issues/9", get(issue_nine));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    format!("http://{addr}")
}

async fn github_setup(pool: PgPool) -> Setup {
    let base = fake_github().await;
    let config = crate::config::AppConfig {
        github_web_url: base.clone(),
        github_api_url: base,
        ..test_config()
    };
    let (app, state) = app_and_state_with(pool, config).await;
    let (owner, _email, owner_token) = seed_and_login(&app, &state, "gh-owner", false).await;
    let ws_id = seed_workspace(&state, owner, &format!("ws-{}", uuid::Uuid::new_v4())).await;
    Setup {
        app,
        state,
        owner,
        owner_token,
        ws_id,
    }
}

const PUBLIC_PR: &str = "https://github.com/acme/rocket/pull/42";
const PRIVATE_ISSUE: &str = "https://github.com/acme/secret/issues/7";

#[test_macros::db_test(migrations = "../migrations")]
async fn a_public_pull_request_gets_a_card_with_no_setup_at_all(pool: PgPool) {
    let s = github_setup(pool).await;
    match service::unfurl(&s.state, s.ws_id, s.owner, PUBLIC_PR)
        .await
        .expect("unfurl")
    {
        AppUnfurl::Card(card) => {
            assert_eq!(card.title, "Ship it");
            assert_eq!(card.provider.as_deref(), Some("github"));
        }
        _ => panic!("a public pull request is carded anonymously"),
    }
    assert!(matches!(
        service::unfurl(&s.state, s.ws_id, s.owner, PRIVATE_ISSUE)
            .await
            .expect("unfurl"),
        AppUnfurl::Unclaimed
    ));
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_private_issue_needs_the_posters_github_account(pool: PgPool) {
    let s = github_setup(pool).await;
    let (status, body) = send(
        &s.app,
        "PUT",
        &format!("/api/workspaces/{}/apps/github", s.ws_id),
        Some(&s.owner_token),
        Some(json!({ "client_id": "gh-client", "client_secret": "shh", "enabled": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert!(matches!(
        service::unfurl(&s.state, s.ws_id, s.owner, PRIVATE_ISSUE)
            .await
            .expect("unfurl"),
        AppUnfurl::Connect(_)
    ));

    let (status, body) = send(
        &s.app,
        "POST",
        &format!("/api/workspaces/{}/apps/github/connect", s.ws_id),
        Some(&s.owner_token),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let url = reqwest::Url::parse(body["authorize_url"].as_str().expect("url")).expect("parse");
    assert!(url.path().ends_with("/login/oauth/authorize"));
    let oauth_state = url
        .query_pairs()
        .find(|(k, _)| k == "state")
        .map(|(_, v)| v.into_owned())
        .expect("state");

    let (status, body) = send(
        &s.app,
        "POST",
        "/api/apps/connections",
        Some(&s.owner_token),
        Some(json!({ "code": "good-code", "state": oauth_state })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["external_account"], json!("Ana (@ana)"));

    match service::unfurl(&s.state, s.ws_id, s.owner, PRIVATE_ISSUE)
        .await
        .expect("unfurl")
    {
        AppUnfurl::Card(card) => assert_eq!(card.title, "Secret bug"),
        _ => panic!("the connected poster's private issue is carded"),
    }
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_rejected_github_code_is_an_error_not_a_connection(pool: PgPool) {
    let s = github_setup(pool).await;
    send(
        &s.app,
        "PUT",
        &format!("/api/workspaces/{}/apps/github", s.ws_id),
        Some(&s.owner_token),
        Some(json!({ "client_id": "gh-client", "client_secret": "shh", "enabled": true })),
    )
    .await;
    let (_, body) = send(
        &s.app,
        "POST",
        &format!("/api/workspaces/{}/apps/github/connect", s.ws_id),
        Some(&s.owner_token),
        Some(json!({})),
    )
    .await;
    let url = reqwest::Url::parse(body["authorize_url"].as_str().expect("url")).expect("parse");
    let oauth_state = url
        .query_pairs()
        .find(|(k, _)| k == "state")
        .map(|(_, v)| v.into_owned())
        .expect("state");
    let (status, _) = send(
        &s.app,
        "POST",
        "/api/apps/connections",
        Some(&s.owner_token),
        Some(json!({ "code": "bad-code", "state": oauth_state })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let connected: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM app_connections WHERE user_id = $1")
            .bind(s.owner)
            .fetch_one(&s.state.pool)
            .await
            .expect("count");
    assert_eq!(connected, 0);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_pull_request_link_to_an_issue_gets_the_issue_card(pool: PgPool) {
    let s = github_setup(pool).await;
    match service::unfurl(
        &s.state,
        s.ws_id,
        s.owner,
        "https://github.com/acme/rocket/pull/9",
    )
    .await
    .expect("unfurl")
    {
        AppUnfurl::Card(card) => {
            assert_eq!(card.title, "Really an issue");
            assert_eq!(card.subtitle.as_deref(), Some("Issue #9 in acme/rocket"));
        }
        _ => panic!("GitHub serves /pull/N for issues too, and so do we"),
    }
}

async fn fake_google() -> String {
    async fn token(
        axum::Form(form): axum::Form<std::collections::HashMap<String, String>>,
    ) -> (StatusCode, Json<Value>) {
        let get = |k: &str| form.get(k).map(String::as_str);
        match get("grant_type") {
            Some("authorization_code")
                if get("code") == Some("good-code")
                    && get("code_verifier").is_some_and(|v| v.len() >= 43) =>
            {
                (
                    StatusCode::OK,
                    Json(
                        json!({ "access_token": "g-1", "refresh_token": "gr-1", "expires_in": 3599 }),
                    ),
                )
            }
            Some("refresh_token") if get("refresh_token") == Some("gr-1") => (
                StatusCode::OK,
                Json(json!({ "access_token": "g-2", "expires_in": 3599 })),
            ),
            _ => (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "invalid_grant" })),
            ),
        }
    }
    fn authorized(headers: &HeaderMap) -> bool {
        headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v == "Bearer g-1" || v == "Bearer g-2")
    }
    async fn about(headers: HeaderMap) -> (StatusCode, Json<Value>) {
        if !authorized(&headers) {
            return (StatusCode::UNAUTHORIZED, Json(json!({})));
        }
        (
            StatusCode::OK,
            Json(json!({ "user": { "displayName": "Ana", "emailAddress": "ana@acme.test" } })),
        )
    }
    async fn file(Path(id): Path<String>, headers: HeaderMap) -> (StatusCode, Json<Value>) {
        if !authorized(&headers) {
            return (StatusCode::UNAUTHORIZED, Json(json!({})));
        }
        if id != "1AbCdEfGhIjKlMnOp" {
            return (StatusCode::NOT_FOUND, Json(json!({})));
        }
        (
            StatusCode::OK,
            Json(json!({
                "name": "Q4 plan",
                "mimeType": "application/vnd.google-apps.document",
                "owners": [{ "displayName": "Ana" }],
                "modifiedTime": "2026-10-21T09:30:00Z"
            })),
        )
    }
    let router = Router::new()
        .route("/token", post(token))
        .route("/drive/v3/about", get(about))
        .route("/drive/v3/files/{id}", get(file));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    format!("http://{addr}")
}

const DOC: &str = "https://docs.google.com/document/d/1AbCdEfGhIjKlMnOp/edit";

async fn google_connected(pool: PgPool) -> Setup {
    let base = fake_google().await;
    let config = crate::config::AppConfig {
        google_accounts_url: base.clone(),
        google_oauth_url: base.clone(),
        google_api_url: base,
        ..test_config()
    };
    let (app, state) = app_and_state_with(pool, config).await;
    let (owner, _email, owner_token) = seed_and_login(&app, &state, "g-owner", false).await;
    let ws_id = seed_workspace(&state, owner, &format!("ws-{}", uuid::Uuid::new_v4())).await;
    let s = Setup {
        app,
        state,
        owner,
        owner_token,
        ws_id,
    };

    assert!(matches!(
        service::unfurl(&s.state, s.ws_id, s.owner, DOC)
            .await
            .expect("unfurl"),
        AppUnfurl::Unclaimed
    ));
    let (status, body) = send(
        &s.app,
        "PUT",
        &format!("/api/workspaces/{}/apps/google_drive", s.ws_id),
        Some(&s.owner_token),
        Some(json!({ "client_id": "g-client", "client_secret": "shh", "enabled": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert!(matches!(
        service::unfurl(&s.state, s.ws_id, s.owner, DOC)
            .await
            .expect("unfurl"),
        AppUnfurl::Connect(_)
    ));

    let (status, body) = send(
        &s.app,
        "POST",
        &format!("/api/workspaces/{}/apps/google_drive/connect", s.ws_id),
        Some(&s.owner_token),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let url = reqwest::Url::parse(body["authorize_url"].as_str().expect("url")).expect("parse");
    assert!(url.path().ends_with("/o/oauth2/v2/auth"));
    assert!(url
        .query_pairs()
        .any(|(k, v)| k == "access_type" && v == "offline"));
    let oauth_state = url
        .query_pairs()
        .find(|(k, _)| k == "state")
        .map(|(_, v)| v.into_owned())
        .expect("state");
    let (status, body) = send(
        &s.app,
        "POST",
        "/api/apps/connections",
        Some(&s.owner_token),
        Some(json!({ "code": "good-code", "state": oauth_state })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["external_account"], json!("Ana (ana@acme.test)"));
    s
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_google_doc_needs_setup_and_a_connection_then_gets_a_card(pool: PgPool) {
    let s = google_connected(pool).await;
    match service::unfurl(&s.state, s.ws_id, s.owner, DOC)
        .await
        .expect("unfurl")
    {
        AppUnfurl::Card(card) => {
            assert_eq!(card.title, "Q4 plan");
            assert_eq!(card.subtitle.as_deref(), Some("Google Doc"));
        }
        _ => panic!("the connected poster's doc is carded"),
    }
    assert!(matches!(
        service::unfurl(
            &s.state,
            s.ws_id,
            s.owner,
            "https://docs.google.com/document/d/1ZzZzZzZzZzZzZzZz/edit"
        )
        .await
        .expect("unfurl"),
        AppUnfurl::Unclaimed
    ));
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_expired_google_token_is_refreshed_and_the_refresh_token_kept(pool: PgPool) {
    let s = google_connected(pool).await;
    sqlx::query(
        "UPDATE app_connections SET expires_at = NOW() - INTERVAL '1 hour' WHERE user_id = $1",
    )
    .bind(s.owner)
    .execute(&s.state.pool)
    .await
    .expect("expire");

    assert!(matches!(
        service::unfurl(&s.state, s.ws_id, s.owner, DOC)
            .await
            .expect("unfurl"),
        AppUnfurl::Card(_)
    ));
    let (expires_at, refresh): (Option<chrono::DateTime<chrono::Utc>>, Option<Vec<u8>>) =
        sqlx::query_as(
            "SELECT expires_at, refresh_token_encrypted FROM app_connections WHERE user_id = $1",
        )
        .bind(s.owner)
        .fetch_one(&s.state.pool)
        .await
        .expect("connection");
    assert!(
        expires_at.is_some_and(|at| at > chrono::Utc::now()),
        "a fresh expiry was stored"
    );
    assert!(
        refresh.is_some(),
        "Google sends no new refresh token; the old one stays"
    );
}
