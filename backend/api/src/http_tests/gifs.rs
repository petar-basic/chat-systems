use axum::http::StatusCode;
use axum::Router;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;
use crate::state::AppState;

const KEY: &str = "giphyTestKey123";

struct Team {
    ws: Uuid,
    admin: String,
    owner: String,
    ana: String,
    general: Uuid,
}

async fn team(app: &Router, state: &AppState) -> Team {
    let (_, _, admin) = seed_and_login(app, state, "gif-admin", true).await;
    let (owner_id, _, owner) = seed_and_login(app, state, "gif-owner", false).await;
    let ws = seed_workspace(state, owner_id, "Gifs").await;
    let (ana_id, _, ana) = seed_and_login(app, state, "gif-ana", false).await;
    add_ws_member(state, ws, ana_id, "member").await;
    let general = seed_channel(state, ws, owner_id, "gif-general", false).await;
    state
        .workspace_service
        .repo
        .add_channel_member(
            general,
            ana_id,
            &crate::workspace::models::ChannelRole::Member,
        )
        .await
        .expect("join");
    Team {
        ws,
        admin,
        owner,
        ana,
        general,
    }
}

async fn set_key(app: &Router, admin: &str, key: &str) -> Value {
    let (status, settings) = send(
        app,
        "PATCH",
        "/api/v1/admin/settings",
        Some(admin),
        Some(json!({ "giphy_api_key": key })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{settings:?}");
    settings
}

async fn gif_settings(app: &Router, token: &str, ws: Uuid) -> (StatusCode, Value) {
    send(
        app,
        "GET",
        &format!("/api/v1/workspaces/{ws}/gifs"),
        Some(token),
        None,
    )
    .await
}

async fn set_workspace(app: &Router, token: &str, ws: Uuid, body: Value) -> StatusCode {
    let (status, _) = send(
        app,
        "PATCH",
        &format!("/api/v1/workspaces/{ws}"),
        Some(token),
        Some(body),
    )
    .await;
    status
}

async fn listed_commands(app: &Router, token: &str, ws: Uuid) -> Vec<String> {
    let (status, body) = send(
        app,
        "GET",
        &format!("/api/v1/workspaces/{ws}/commands"),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    body["data"]
        .as_array()
        .expect("data")
        .iter()
        .map(|c| c["command"].as_str().expect("command").to_string())
        .collect()
}

async fn giphy(app: &Router, token: &str, channel: Uuid) -> Value {
    let (status, body) = send(
        app,
        "POST",
        &format!("/api/v1/channels/{channel}/commands"),
        Some(token),
        Some(json!({ "command": "giphy", "text": "cats" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    body
}

fn gif(url: &str) -> Value {
    json!({
        "provider": "giphy",
        "id": "3o7TKSjRrfIPjeiVyM",
        "title": "  Cat typing  ",
        "url": url,
        "still": "https://media.giphy.com/media/3o7TKSjRrfIPjeiVyM/giphy_s.gif",
        "width": 480,
        "height": 270,
    })
}

const MP4: &str =
    "https://media2.giphy.com/media/3o7TKSjRrfIPjeiVyM/giphy.mp4?cid=a1&rid=giphy.mp4&ct=g";

async fn post(app: &Router, token: &str, channel: Uuid, body: Value) -> (StatusCode, Value) {
    send(
        app,
        "POST",
        &format!("/api/v1/channels/{channel}/messages"),
        Some(token),
        Some(body),
    )
    .await
}

#[test_macros::db_test(migrations = "../migrations")]
async fn gifs_need_an_instance_key_and_a_workspace_that_wants_them(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let t = team(&app, &state).await;

    let (status, _) = gif_settings(&app, &t.ana, t.ws).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "no key, no GIFs");
    assert!(!listed_commands(&app, &t.ana, t.ws)
        .await
        .contains(&"giphy".to_string()));
    assert_eq!(
        giphy(&app, &t.ana, t.general).await["text"],
        "GIFs are off in this workspace."
    );

    let settings = set_key(&app, &t.admin, &format!("  {KEY}  ")).await;
    assert_eq!(settings["giphy_api_key"], KEY);
    let details: Value = sqlx::query_scalar(
        "SELECT details FROM audit_log WHERE action = 'instance.settings_changed'",
    )
    .fetch_one(&state.pool)
    .await
    .expect("audited");
    assert_eq!(
        details,
        json!({ "giphy_api_key": "set" }),
        "the key itself is not audited"
    );

    let (status, body) = gif_settings(&app, &t.ana, t.ws).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(
        body,
        json!({ "provider": "giphy", "api_key": KEY, "rating": "pg" })
    );
    assert!(listed_commands(&app, &t.ana, t.ws)
        .await
        .contains(&"giphy".to_string()));
    assert_eq!(
        giphy(&app, &t.ana, t.general).await["text"],
        "GIFs are picked in the app's message box."
    );

    let (_, _, outsider) = seed_and_login(&app, &state, "gif-outsider", false).await;
    let (status, _) = gif_settings(&app, &outsider, t.ws).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    assert_eq!(
        set_workspace(&app, &t.ana, t.ws, json!({ "gifs": false })).await,
        StatusCode::FORBIDDEN,
        "a member cannot turn them off"
    );
    assert_eq!(
        set_workspace(&app, &t.owner, t.ws, json!({ "gif_rating": "g" })).await,
        StatusCode::OK
    );
    assert_eq!(gif_settings(&app, &t.ana, t.ws).await.1["rating"], "g");
    assert_eq!(
        set_workspace(&app, &t.owner, t.ws, json!({ "gif_rating": "nc-17" })).await,
        StatusCode::UNPROCESSABLE_ENTITY
    );

    assert_eq!(
        set_workspace(&app, &t.owner, t.ws, json!({ "gifs": false })).await,
        StatusCode::OK
    );
    let (status, _) = gif_settings(&app, &t.ana, t.ws).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "the workspace turned them off"
    );
    assert!(!listed_commands(&app, &t.ana, t.ws)
        .await
        .contains(&"giphy".to_string()));
    assert_eq!(
        giphy(&app, &t.ana, t.general).await["text"],
        "GIFs are off in this workspace."
    );

    assert_eq!(
        set_workspace(&app, &t.owner, t.ws, json!({ "gifs": true })).await,
        StatusCode::OK
    );
    assert_eq!(gif_settings(&app, &t.ana, t.ws).await.0, StatusCode::OK);
    let cleared = set_key(&app, &t.admin, "").await;
    assert_eq!(cleared["giphy_api_key"], Value::Null);
    assert_eq!(
        gif_settings(&app, &t.ana, t.ws).await.0,
        StatusCode::NOT_FOUND
    );

    let (status, _) = send(
        &app,
        "PATCH",
        "/api/v1/admin/settings",
        Some(&t.admin),
        Some(json!({ "giphy_api_key": "not a key" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = send(
        &app,
        "PATCH",
        "/api/v1/admin/settings",
        Some(&t.owner),
        Some(json!({ "giphy_api_key": KEY })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "only an instance admin sets the key"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_gif_from_giphy_is_sent_with_its_message(pool: PgPool) {
    let (app, state) = app_and_state(pool).await;
    let t = team(&app, &state).await;

    let (status, _) = post(
        &app,
        &t.ana,
        t.general,
        json!({ "content": "", "gif": gif(MP4) }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "no key, no GIFs");

    set_key(&app, &t.admin, KEY).await;
    let (status, sent) = post(
        &app,
        &t.ana,
        t.general,
        json!({ "content": "look", "gif": gif(MP4) }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{sent:?}");
    assert_eq!(sent["content"], "look");
    assert_eq!(sent["metadata"]["gif"]["title"], "Cat typing");
    assert_eq!(
        sent["metadata"]["gif"]["url"], MP4,
        "GIPHY's URL is kept as it came"
    );

    let (status, alone) = post(
        &app,
        &t.ana,
        t.general,
        json!({ "content": " ", "gif": gif(MP4) }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "a GIF needs no words: {alone:?}");
    let (status, edited) = send(
        &app,
        "PATCH",
        &format!("/api/v1/messages/{}", sent["id"].as_str().expect("id")),
        Some(&t.ana),
        Some(json!({ "content": "" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "its words can go: {edited:?}");
    assert_eq!(edited["metadata"]["gif"]["id"], "3o7TKSjRrfIPjeiVyM");

    let (status, listed) = send(
        &app,
        "GET",
        &format!("/api/v1/channels/{}/messages", t.general),
        Some(&t.owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let listed = listed["data"].as_array().expect("data");
    assert_eq!(listed.len(), 2);
    assert!(listed
        .iter()
        .all(|m| m["metadata"]["gif"]["provider"] == "giphy"));

    for url in [
        "https://media.giphy.com.evil.example/x.mp4",
        "http://media.giphy.com/media/x/giphy.mp4",
        "https://evil.example/giphy.mp4",
    ] {
        let (status, _) = post(
            &app,
            &t.ana,
            t.general,
            json!({ "content": "", "gif": gif(url) }),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{url}");
    }
    let (status, _) = post(
        &app,
        &t.ana,
        t.general,
        json!({ "content": "", "gif": gif(MP4), "attachment_ids": [Uuid::new_v4()] }),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "a GIF goes alone");

    assert_eq!(
        set_workspace(&app, &t.owner, t.ws, json!({ "gifs": false })).await,
        StatusCode::OK
    );
    let (status, _) = post(
        &app,
        &t.ana,
        t.general,
        json!({ "content": "", "gif": gif(MP4) }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "a workspace with GIFs off refuses them"
    );
    let (status, _) = post(
        &app,
        &t.ana,
        t.general,
        json!({ "content": "words still go" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}
