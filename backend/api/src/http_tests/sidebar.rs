use super::common::*;
use crate::workspace::models::ChannelRole;
use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

struct Setup {
    app: axum::Router,
    state: std::sync::Arc<crate::state::AppState>,
    owner: uuid::Uuid,
    token: String,
    ws_id: uuid::Uuid,
    ch_id: uuid::Uuid,
}

async fn setup(pool: PgPool) -> Setup {
    let (app, state) = app_and_state(pool).await;
    let (owner, _email, token) = seed_and_login(&app, &state, "owner", false).await;
    let ws_id = seed_workspace(&state, owner, &format!("ws-{}", uuid::Uuid::new_v4())).await;
    let ch_id = seed_channel(&state, ws_id, owner, "projects-x", false).await;
    Setup {
        app,
        state,
        owner,
        token,
        ws_id,
        ch_id,
    }
}

async fn create_section(s: &Setup, token: &str, name: &str) -> (StatusCode, serde_json::Value) {
    send(
        &s.app,
        "POST",
        &format!("/api/workspaces/{}/sidebar/sections", s.ws_id),
        Some(token),
        Some(json!({ "name": name })),
    )
    .await
}

async fn layout(s: &Setup, token: &str) -> serde_json::Value {
    let (status, body) = send(
        &s.app,
        "GET",
        &format!("/api/workspaces/{}/sidebar", s.ws_id),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    body
}

async fn place(s: &Setup, token: &str, section: serde_json::Value) -> StatusCode {
    send(
        &s.app,
        "PUT",
        &format!("/api/channels/{}/section", s.ch_id),
        Some(token),
        Some(json!({ "section_id": section })),
    )
    .await
    .0
}

fn section_names(body: &serde_json::Value) -> Vec<String> {
    body["sections"]
        .as_array()
        .expect("sections")
        .iter()
        .map(|s| s["name"].as_str().expect("name").to_string())
        .collect()
}

#[test_macros::db_test(migrations = "../migrations")]
async fn sections_can_be_created_renamed_reordered_and_collapsed(pool: PgPool) {
    let s = setup(pool).await;
    let (status, projects) = create_section(&s, &s.token, "  Projects ").await;
    assert_eq!(status, StatusCode::OK, "{projects:?}");
    assert_eq!(projects["name"], json!("Projects"));
    let (_, clients) = create_section(&s, &s.token, "Clients").await;
    assert_eq!(
        section_names(&layout(&s, &s.token).await),
        vec!["Projects", "Clients"]
    );

    let (status, renamed) = send(
        &s.app,
        "PATCH",
        &format!(
            "/api/workspaces/{}/sidebar/sections/{}",
            s.ws_id,
            projects["id"].as_str().unwrap()
        ),
        Some(&s.token),
        Some(json!({ "name": "Active projects", "collapsed": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{renamed:?}");
    assert_eq!(renamed["collapsed"], json!(true));

    let (status, _) = send(
        &s.app,
        "PUT",
        &format!("/api/workspaces/{}/sidebar/order", s.ws_id),
        Some(&s.token),
        Some(json!({ "section_ids": [clients["id"], projects["id"]] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        section_names(&layout(&s, &s.token).await),
        vec!["Clients", "Active projects"]
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_section_name_is_unique_and_not_empty(pool: PgPool) {
    let s = setup(pool).await;
    assert_eq!(
        create_section(&s, &s.token, "Social").await.0,
        StatusCode::OK
    );
    assert_eq!(
        create_section(&s, &s.token, "social").await.0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        create_section(&s, &s.token, "   ").await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_channel_moves_into_a_section_and_back_when_it_is_deleted(pool: PgPool) {
    let s = setup(pool).await;
    let (_, section) = create_section(&s, &s.token, "Projects").await;
    assert_eq!(
        place(&s, &s.token, section["id"].clone()).await,
        StatusCode::OK
    );
    let body = layout(&s, &s.token).await;
    assert_eq!(
        body["placements"],
        json!([{ "channel_id": s.ch_id, "section_id": section["id"] }])
    );

    let (status, _) = send(
        &s.app,
        "DELETE",
        &format!(
            "/api/workspaces/{}/sidebar/sections/{}",
            s.ws_id,
            section["id"].as_str().unwrap()
        ),
        Some(&s.token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(layout(&s, &s.token).await["placements"], json!([]));
}

#[test_macros::db_test(migrations = "../migrations")]
async fn leaving_a_channel_forgets_its_section(pool: PgPool) {
    let s = setup(pool).await;
    let (_, section) = create_section(&s, &s.token, "Projects").await;
    assert_eq!(
        place(&s, &s.token, section["id"].clone()).await,
        StatusCode::OK
    );

    let repo = &s.state.workspace_service.repo;
    repo.remove_channel_member(s.ch_id, s.owner)
        .await
        .expect("leave");
    repo.add_channel_member(s.ch_id, s.owner, &ChannelRole::Member)
        .await
        .expect("rejoin");
    assert_eq!(layout(&s, &s.token).await["placements"], json!([]));
}

#[test_macros::db_test(migrations = "../migrations")]
async fn nobody_else_can_touch_your_sections(pool: PgPool) {
    let s = setup(pool).await;
    let (_, section) = create_section(&s, &s.token, "Mine").await;
    let (other, _email, other_token) = seed_and_login(&s.app, &s.state, "other", false).await;
    add_ws_member(&s.state, s.ws_id, other, "member").await;
    s.state
        .workspace_service
        .repo
        .add_channel_member(s.ch_id, other, &ChannelRole::Member)
        .await
        .expect("join");

    let section_url = format!(
        "/api/workspaces/{}/sidebar/sections/{}",
        s.ws_id,
        section["id"].as_str().unwrap()
    );
    let (status, _) = send(
        &s.app,
        "PATCH",
        &section_url,
        Some(&other_token),
        Some(json!({ "name": "Theirs" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = send(&s.app, "DELETE", &section_url, Some(&other_token), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        place(&s, &other_token, section["id"].clone()).await,
        StatusCode::NOT_FOUND
    );
    assert!(section_names(&layout(&s, &other_token).await).is_empty());
    assert_eq!(section_names(&layout(&s, &s.token).await), vec!["Mine"]);
}

async fn collapse_dms(s: &Setup, collapsed: bool) -> StatusCode {
    send(
        &s.app,
        "PUT",
        &format!("/api/workspaces/{}/sidebar/builtins", s.ws_id),
        Some(&s.token),
        Some(json!({ "section": "dms", "collapsed": collapsed })),
    )
    .await
    .0
}

#[test_macros::db_test(migrations = "../migrations")]
async fn built_in_sections_remember_being_collapsed(pool: PgPool) {
    let s = setup(pool).await;
    assert_eq!(collapse_dms(&s, true).await, StatusCode::OK);
    assert_eq!(
        layout(&s, &s.token).await["collapsed_builtins"],
        json!(["dms"])
    );
    assert_eq!(collapse_dms(&s, false).await, StatusCode::OK);
    assert_eq!(layout(&s, &s.token).await["collapsed_builtins"], json!([]));
}

#[test_macros::db_test(migrations = "../migrations")]
async fn sidebar_outsider_forbidden(pool: PgPool) {
    let s = setup(pool).await;
    let (_outsider, _email, outsider_token) =
        seed_and_login(&s.app, &s.state, "outsider", false).await;
    let (status, _) = send(
        &s.app,
        "GET",
        &format!("/api/workspaces/{}/sidebar", s.ws_id),
        Some(&outsider_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
