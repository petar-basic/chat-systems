use std::sync::Arc;

use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};
use sqlx::PgPool;

use super::common::*;
use crate::integrations::service::{self, AppUnfurl};
use crate::sealed::reseal::{self, Report};
use crate::sealed::Keyring;

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
    async fn content(
        Path((cloud, collection, id)): Path<(String, String, String)>,
        headers: HeaderMap,
    ) -> (StatusCode, Json<Value>) {
        if !authorized(&headers) {
            return (StatusCode::UNAUTHORIZED, Json(json!({})));
        }
        let version = json!({ "createdAt": "2026-10-21T09:30:00.000Z", "authorId": "acc-lazar" });
        match (cloud.as_str(), collection.as_str(), id.as_str()) {
            ("cloud-1", "pages", "12345") => (
                StatusCode::OK,
                Json(json!({
                    "title": "Onboarding guide",
                    "spaceId": "777",
                    "ownerId": "acc-ana",
                    "authorId": "acc-ana",
                    "version": version
                })),
            ),
            ("cloud-1", "blogposts", "999") => (
                StatusCode::OK,
                Json(json!({
                    "title": "Release notes",
                    "spaceId": "777",
                    "authorId": "acc-ana",
                    "version": version
                })),
            ),
            _ => (StatusCode::NOT_FOUND, Json(json!({}))),
        }
    }
    async fn space(
        Path((_cloud, id)): Path<(String, String)>,
        headers: HeaderMap,
    ) -> (StatusCode, Json<Value>) {
        if !authorized(&headers) || id != "777" {
            return (StatusCode::NOT_FOUND, Json(json!({})));
        }
        (
            StatusCode::OK,
            Json(json!({ "id": "777", "key": "ENG", "name": "Engineering" })),
        )
    }
    async fn users(headers: HeaderMap, Json(body): Json<Value>) -> (StatusCode, Json<Value>) {
        if !authorized(&headers) {
            return (StatusCode::UNAUTHORIZED, Json(json!({})));
        }
        let known = [("acc-ana", "Ana"), ("acc-lazar", "Lazar")];
        let results: Vec<Value> = body["accountIds"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|id| {
                known
                    .iter()
                    .find(|(k, _)| Some(*k) == id.as_str())
                    .map(|(k, name)| json!({ "accountId": k, "displayName": name }))
            })
            .collect();
        (StatusCode::OK, Json(json!({ "results": results })))
    }
    let router = Router::new()
        .route("/oauth/token", post(token))
        .route("/oauth/token/accessible-resources", get(sites))
        .route("/me", get(me))
        .route("/ex/jira/{cloud}/rest/api/3/issue/{key}", get(issue))
        .route("/ex/confluence/{cloud}/wiki/api/v2/spaces/{id}", get(space))
        .route("/ex/confluence/{cloud}/wiki/api/v2/users-bulk", post(users))
        .route(
            "/ex/confluence/{cloud}/wiki/api/v2/{collection}/{id}",
            get(content),
        );
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

const FIGMA_KEY: &str = "AbCdEfGhIjKlMnOpQrStUv";

async fn fake_figma() -> String {
    fn client_authenticated(headers: &HeaderMap) -> bool {
        use base64::Engine;
        let expected = format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode("figma-client:shh")
        );
        headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v == expected)
    }
    async fn token(
        headers: HeaderMap,
        axum::Form(form): axum::Form<std::collections::HashMap<String, String>>,
    ) -> (StatusCode, Json<Value>) {
        let get = |k: &str| form.get(k).map(String::as_str);
        if client_authenticated(&headers)
            && get("grant_type") == Some("authorization_code")
            && get("code") == Some("good-code")
            && get("redirect_uri").is_some_and(|u| u.ends_with("/oauth/apps/callback"))
            && get("code_verifier").is_some_and(|v| v.len() >= 43)
        {
            return (
                StatusCode::OK,
                Json(json!({
                    "user_id_string": "42",
                    "access_token": "f-1",
                    "token_type": "bearer",
                    "expires_in": 7_776_000,
                    "refresh_token": "fr-1"
                })),
            );
        }
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "invalid_grant" })),
        )
    }
    async fn refresh(
        headers: HeaderMap,
        axum::Form(form): axum::Form<std::collections::HashMap<String, String>>,
    ) -> (StatusCode, Json<Value>) {
        if client_authenticated(&headers)
            && form.get("refresh_token").map(String::as_str) == Some("fr-1")
        {
            return (
                StatusCode::OK,
                Json(
                    json!({ "access_token": "f-2", "token_type": "bearer", "expires_in": 7_776_000 }),
                ),
            );
        }
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "invalid_grant" })),
        )
    }
    fn authorized(headers: &HeaderMap) -> bool {
        headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v == "Bearer f-1" || v == "Bearer f-2")
    }
    async fn me(headers: HeaderMap) -> (StatusCode, Json<Value>) {
        if !authorized(&headers) {
            return (StatusCode::FORBIDDEN, Json(json!({})));
        }
        (
            StatusCode::OK,
            Json(json!({ "id": "42", "handle": "Ana", "email": "ana@acme.test" })),
        )
    }
    async fn meta(Path(key): Path<String>, headers: HeaderMap) -> (StatusCode, Json<Value>) {
        if !authorized(&headers) {
            return (StatusCode::FORBIDDEN, Json(json!({})));
        }
        if key != FIGMA_KEY {
            return (StatusCode::NOT_FOUND, Json(json!({})));
        }
        (
            StatusCode::OK,
            Json(json!({ "file": {
                "name": "Checkout redesign",
                "folder_name": "Payments",
                "editorType": "figma",
                "last_touched_at": "2026-10-21T09:30:00Z",
                "last_touched_by": { "handle": "Lazar" },
                "thumbnail_url": "https://s3.test/thumb.png"
            }})),
        )
    }
    let router = Router::new()
        .route("/v1/oauth/token", post(token))
        .route("/v1/oauth/refresh", post(refresh))
        .route("/v1/me", get(me))
        .route("/v1/files/{key}/meta", get(meta));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    format!("http://{addr}")
}

async fn figma_connected(pool: PgPool) -> Setup {
    let base = fake_figma().await;
    let config = crate::config::AppConfig {
        figma_web_url: base.clone(),
        figma_api_url: base,
        ..test_config()
    };
    let (app, state) = app_and_state_with(pool, config).await;
    let (owner, _email, owner_token) = seed_and_login(&app, &state, "f-owner", false).await;
    let ws_id = seed_workspace(&state, owner, &format!("ws-{}", uuid::Uuid::new_v4())).await;
    let s = Setup {
        app,
        state,
        owner,
        owner_token,
        ws_id,
    };
    let design = format!("https://www.figma.com/design/{FIGMA_KEY}/Checkout");

    assert!(matches!(
        service::unfurl(&s.state, s.ws_id, s.owner, &design)
            .await
            .expect("unfurl"),
        AppUnfurl::Unclaimed
    ));
    let (status, body) = send(
        &s.app,
        "PUT",
        &format!("/api/workspaces/{}/apps/figma", s.ws_id),
        Some(&s.owner_token),
        Some(json!({ "client_id": "figma-client", "client_secret": "shh", "enabled": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert!(matches!(
        service::unfurl(&s.state, s.ws_id, s.owner, &design)
            .await
            .expect("unfurl"),
        AppUnfurl::Connect(_)
    ));

    let (status, body) = send(
        &s.app,
        "POST",
        &format!("/api/workspaces/{}/apps/figma/connect", s.ws_id),
        Some(&s.owner_token),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let url = reqwest::Url::parse(body["authorize_url"].as_str().expect("url")).expect("parse");
    assert_eq!(url.path(), "/oauth");
    assert!(url
        .query_pairs()
        .any(|(k, v)| k == "scope" && v == "file_metadata:read,current_user:read"));
    assert!(url
        .query_pairs()
        .any(|(k, v)| k == "code_challenge_method" && v == "S256"));
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
async fn a_figma_design_gets_a_card_with_its_thumbnail(pool: PgPool) {
    let s = figma_connected(pool).await;
    match service::unfurl(
        &s.state,
        s.ws_id,
        s.owner,
        &format!("https://www.figma.com/design/{FIGMA_KEY}/Checkout?node-id=1-2"),
    )
    .await
    .expect("unfurl")
    {
        AppUnfurl::Card(card) => {
            assert_eq!(card.title, "Checkout redesign");
            assert_eq!(card.subtitle.as_deref(), Some("Figma design"));
            assert_eq!(card.provider.as_deref(), Some("figma"));
            assert_eq!(card.image_url.as_deref(), Some("https://s3.test/thumb.png"));
        }
        _ => panic!("the connected poster's design is carded"),
    }
    assert!(matches!(
        service::unfurl(
            &s.state,
            s.ws_id,
            s.owner,
            "https://www.figma.com/design/ZzZzZzZzZzZzZzZzZzZzZz/Secret"
        )
        .await
        .expect("unfurl"),
        AppUnfurl::Unclaimed
    ));
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_expired_figma_token_is_refreshed_and_the_refresh_token_kept(pool: PgPool) {
    let s = figma_connected(pool).await;
    sqlx::query(
        "UPDATE app_connections SET expires_at = NOW() - INTERVAL '1 hour' WHERE user_id = $1",
    )
    .bind(s.owner)
    .execute(&s.state.pool)
    .await
    .expect("expire");

    assert!(matches!(
        service::unfurl(
            &s.state,
            s.ws_id,
            s.owner,
            &format!("https://www.figma.com/board/{FIGMA_KEY}/Retro"),
        )
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
        "Figma sends no new refresh token; the old one stays"
    );
}

const PAGE: &str = "https://acme.atlassian.net/wiki/spaces/ENG/pages/12345/Onboarding+guide";

async fn configure_confluence(s: &Setup, token: &str, body: Value) -> (StatusCode, Value) {
    send(
        &s.app,
        "PUT",
        &format!("/api/workspaces/{}/apps/confluence", s.ws_id),
        Some(token),
        Some(body),
    )
    .await
}

#[test_macros::db_test(migrations = "../migrations")]
async fn confluence_reuses_the_jira_app_without_the_secret_leaving_the_server(pool: PgPool) {
    let s = setup(pool).await;
    let reuse = json!({ "client_id": "", "enabled": true, "reuse_from": "jira" });
    let (status, _) = configure_confluence(&s, &s.owner_token, reuse.clone()).await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "there is no Jira app to reuse yet"
    );

    configure(
        &s,
        &s.owner_token,
        json!({ "client_id": "client-1", "client_secret": "shh", "enabled": true }),
    )
    .await;
    let (member, _email, member_token) =
        seed_and_login(&s.app, &s.state, "apps-member", false).await;
    add_ws_member(&s.state, s.ws_id, member, "member").await;
    let (status, _) = configure_confluence(&s, &member_token, reuse.clone()).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, body) = configure_confluence(&s, &s.owner_token, reuse).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let (status, body) = send(
        &s.app,
        "GET",
        &format!("/api/workspaces/{}/apps", s.ws_id),
        Some(&s.owner_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let confluence = body["data"]
        .as_array()
        .expect("apps")
        .iter()
        .find(|a| a["provider"] == "confluence")
        .expect("confluence")
        .clone();
    assert_eq!(confluence["configured"], json!(true));
    assert_eq!(confluence["client_id"], json!("client-1"));
    assert!(!body.to_string().contains("shh"));

    let secrets: Vec<Vec<u8>> = sqlx::query_scalar(
        "SELECT client_secret_encrypted FROM workspace_apps WHERE workspace_id = $1 ORDER BY provider",
    )
    .bind(s.ws_id)
    .fetch_all(&s.state.pool)
    .await
    .expect("secrets");
    assert_eq!(secrets.len(), 2);
    assert_eq!(
        secrets[0], secrets[1],
        "the sealed secret is copied as it is"
    );

    let (status, _) = send(
        &s.app,
        "PUT",
        &format!("/api/workspaces/{}/apps/github", s.ws_id),
        Some(&s.owner_token),
        Some(json!({ "client_id": "", "enabled": true, "reuse_from": "jira" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = configure_confluence(
        &s,
        &s.owner_token,
        json!({ "client_id": "", "enabled": true, "reuse_from": "github" }),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_confluence_page_needs_its_own_connection_then_gets_a_card(pool: PgPool) {
    let s = setup(pool).await;
    configure(
        &s,
        &s.owner_token,
        json!({ "client_id": "client-1", "client_secret": "shh", "enabled": true }),
    )
    .await;
    assert!(matches!(
        service::unfurl(&s.state, s.ws_id, s.owner, PAGE)
            .await
            .expect("unfurl"),
        AppUnfurl::Unclaimed
    ));
    configure_confluence(
        &s,
        &s.owner_token,
        json!({ "client_id": "", "enabled": true, "reuse_from": "jira" }),
    )
    .await;

    let jira_state = connect(&s, &s.owner_token).await;
    assert_eq!(
        complete(&s, &s.owner_token, &jira_state).await.0,
        StatusCode::OK
    );
    assert!(
        matches!(
            service::unfurl(&s.state, s.ws_id, s.owner, PAGE)
                .await
                .expect("unfurl"),
            AppUnfurl::Connect(_)
        ),
        "a Jira connection does not read Confluence"
    );

    let (status, body) = send(
        &s.app,
        "POST",
        &format!("/api/workspaces/{}/apps/confluence/connect", s.ws_id),
        Some(&s.owner_token),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let url = reqwest::Url::parse(body["authorize_url"].as_str().expect("url")).expect("parse");
    let scope = url
        .query_pairs()
        .find(|(k, _)| k == "scope")
        .map(|(_, v)| v.into_owned())
        .expect("scope");
    assert!(scope.contains("read:page:confluence"), "{scope}");
    assert!(!scope.contains("read:jira-work"), "{scope}");
    let oauth_state = url
        .query_pairs()
        .find(|(k, _)| k == "state")
        .map(|(_, v)| v.into_owned())
        .expect("state");
    assert_eq!(
        complete(&s, &s.owner_token, &oauth_state).await.0,
        StatusCode::OK
    );

    match service::unfurl(&s.state, s.ws_id, s.owner, PAGE)
        .await
        .expect("unfurl")
    {
        AppUnfurl::Card(card) => {
            assert_eq!(card.title, "Onboarding guide");
            assert_eq!(card.provider.as_deref(), Some("confluence"));
            assert_eq!(card.subtitle.as_deref(), Some("Page in Engineering"));
            let fields: Vec<(String, String)> = card
                .fields
                .iter()
                .map(|f| (f.label.clone(), f.value.clone()))
                .collect();
            assert_eq!(
                fields,
                vec![
                    ("Owner".into(), "Ana".into()),
                    ("Last updated".into(), "Oct 21, 2026 by Lazar".into())
                ]
            );
        }
        _ => panic!("the connected poster's page is carded"),
    }
    match service::unfurl(
        &s.state,
        s.ws_id,
        s.owner,
        "https://acme.atlassian.net/wiki/spaces/ENG/blog/2026/10/21/999/Release+notes",
    )
    .await
    .expect("unfurl")
    {
        AppUnfurl::Card(card) => {
            assert_eq!(card.subtitle.as_deref(), Some("Blog post in Engineering"))
        }
        _ => panic!("a blog post is carded"),
    }
    for unknown in [
        "https://acme.atlassian.net/wiki/spaces/ENG/pages/55555",
        "https://other.atlassian.net/wiki/spaces/ENG/pages/12345",
    ] {
        assert!(
            matches!(
                service::unfurl(&s.state, s.ws_id, s.owner, unknown)
                    .await
                    .expect("unfurl"),
                AppUnfurl::Unclaimed
            ),
            "{unknown}"
        );
    }
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_new_key_reseals_every_app_secret_and_the_old_one_can_go(pool: PgPool) {
    let s = google_connected(pool).await;
    let report = reseal::run(
        &s.state.pool,
        &rotated_keyring(),
        &s.state.config.jwt_secret,
    )
    .await
    .expect("reseal");
    assert_eq!(
        report,
        Report {
            resealed: 3,
            unreadable: vec![]
        },
        "the client secret, the access token and the refresh token"
    );
    let again = reseal::run(
        &s.state.pool,
        &rotated_keyring(),
        &s.state.config.jwt_secret,
    )
    .await
    .expect("reseal");
    assert_eq!(again.resealed, 0, "a second start has nothing to do");

    let keys: (Option<String>, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT a.client_secret_key_id, c.access_token_key_id, c.refresh_token_key_id
           FROM workspace_apps a JOIN app_connections c USING (workspace_id, provider)
          WHERE a.workspace_id = $1",
    )
    .bind(s.ws_id)
    .fetch_one(&s.state.pool)
    .await
    .expect("keys");
    let next = Some("next".to_string());
    assert_eq!(keys, (next.clone(), next.clone(), next));

    let (_, state) = app_and_state_with(
        s.state.pool.clone(),
        crate::config::AppConfig {
            secrets_keys: Keyring::parse(NEXT_SECRETS_KEY).expect("ring"),
            ..s.state.config.clone()
        },
    )
    .await;
    sqlx::query(
        "UPDATE app_connections SET expires_at = NOW() - INTERVAL '1 hour' WHERE user_id = $1",
    )
    .bind(s.owner)
    .execute(&state.pool)
    .await
    .expect("expire");
    assert!(
        matches!(
            service::unfurl(&state, s.ws_id, s.owner, DOC)
                .await
                .expect("unfurl"),
            AppUnfurl::Card(_)
        ),
        "with only the new key the token still refreshes"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn secrets_sealed_by_3_7_are_resealed_when_the_api_starts(pool: PgPool) {
    let s = google_connected(pool).await;
    let jwt = s.state.config.jwt_secret.clone();
    let (secret, secret_nonce) = sealed_the_3_7_way(&jwt, "workspace-app-secrets", "shh");
    let (access, access_nonce) = sealed_the_3_7_way(&jwt, "workspace-app-secrets", "g-1");
    let (refresh, refresh_nonce) = sealed_the_3_7_way(&jwt, "workspace-app-secrets", "gr-1");
    sqlx::query(
        "UPDATE workspace_apps
            SET client_secret_encrypted = $2, client_secret_nonce = $3, client_secret_key_id = NULL
          WHERE workspace_id = $1",
    )
    .bind(s.ws_id)
    .bind(secret)
    .bind(secret_nonce)
    .execute(&s.state.pool)
    .await
    .expect("legacy app");
    sqlx::query(
        "UPDATE app_connections
            SET access_token_encrypted = $2, access_token_nonce = $3, access_token_key_id = NULL,
                refresh_token_encrypted = $4, refresh_token_nonce = $5, refresh_token_key_id = NULL,
                expires_at = NOW() - INTERVAL '1 hour'
          WHERE workspace_id = $1",
    )
    .bind(s.ws_id)
    .bind(access)
    .bind(access_nonce)
    .bind(refresh)
    .bind(refresh_nonce)
    .execute(&s.state.pool)
    .await
    .expect("legacy connection");

    let report = reseal::run(&s.state.pool, &s.state.config.secrets_keys, &jwt)
        .await
        .expect("reseal");
    assert_eq!(
        report,
        Report {
            resealed: 3,
            unreadable: vec![]
        }
    );
    assert!(
        matches!(
            service::unfurl(&s.state, s.ws_id, s.owner, DOC)
                .await
                .expect("unfurl"),
            AppUnfurl::Card(_)
        ),
        "the re-sealed refresh token still refreshes"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_secret_on_a_key_no_longer_listed_is_reported_and_left_alone(pool: PgPool) {
    let s = google_connected(pool).await;
    sqlx::query("UPDATE workspace_apps SET client_secret_key_id = 'gone' WHERE workspace_id = $1")
        .bind(s.ws_id)
        .execute(&s.state.pool)
        .await
        .expect("retire");

    let report = reseal::run(
        &s.state.pool,
        &s.state.config.secrets_keys,
        &s.state.config.jwt_secret,
    )
    .await
    .expect("reseal");
    assert_eq!(report.resealed, 0);
    assert_eq!(report.unreadable.len(), 1, "{report:?}");
    assert!(report.unreadable[0].contains("google_drive"), "{report:?}");

    let key: Option<String> = sqlx::query_scalar(
        "SELECT client_secret_key_id FROM workspace_apps WHERE workspace_id = $1",
    )
    .bind(s.ws_id)
    .fetch_one(&s.state.pool)
    .await
    .expect("key");
    assert_eq!(key.as_deref(), Some("gone"));
}
