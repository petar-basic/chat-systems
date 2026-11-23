use axum::http::StatusCode;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use super::common::*;

struct Hooked {
    app: axum::Router,
    owner_id: Uuid,
    token: String,
    ws: Uuid,
    ch: Uuid,
    hook_token: String,
}

async fn hooked(pool: PgPool) -> Hooked {
    let (app, state) = app_and_state(pool).await;
    let (owner_id, _, token) = seed_and_login(&app, &state, "blocks-owner", false).await;
    let ws = seed_workspace(&state, owner_id, "Blocks").await;
    let ch = seed_channel(&state, ws, owner_id, "builds", false).await;
    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/v1/workspaces/{ws}/hooks"),
        Some(&token),
        Some(json!({
            "hook_type": "incoming_webhook",
            "name": "ci",
            "config": { "channel_id": ch }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    Hooked {
        app,
        owner_id,
        token,
        ws,
        ch,
        hook_token: body["config"]["token"].as_str().expect("token").to_string(),
    }
}

async fn post_hook(h: &Hooked, payload: Value) -> (StatusCode, Value) {
    send(
        &h.app,
        "POST",
        &format!("/api/v1/hooks/incoming/{}", h.hook_token),
        None,
        Some(payload),
    )
    .await
}

async fn latest(h: &Hooked) -> Value {
    let (status, body) = send(
        &h.app,
        "GET",
        &format!("/api/v1/channels/{}/messages", h.ch),
        Some(&h.token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    body["data"][0].clone()
}

fn fixture(name: &str) -> Value {
    let text = match name {
        "github_actions" => include_str!("../blocks/fixtures/github_actions.json"),
        "grafana" => include_str!("../blocks/fixtures/grafana.json"),
        "sentry" => include_str!("../blocks/fixtures/sentry.json"),
        "jenkins" => include_str!("../blocks/fixtures/jenkins.json"),
        other => panic!("no fixture {other}"),
    };
    serde_json::from_str(text).expect("fixture json")
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_github_actions_payload_posted_unchanged_shows_its_header_fields_and_button(
    pool: PgPool,
) {
    let h = hooked(pool).await;
    let (status, body) = post_hook(&h, fixture("github_actions")).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");

    let message = latest(&h).await;
    assert_eq!(message["content"], "GitHub Action build result: success");
    let blocks = message["metadata"]["blocks"].as_array().expect("blocks");
    assert_eq!(blocks[0]["type"], "header");
    assert_eq!(blocks[0]["text"]["text"], "✅ Build passed");
    assert_eq!(blocks[1]["fields"].as_array().map(Vec::len), Some(4));
    assert_eq!(blocks[3]["elements"][0]["type"], "button");
    assert_eq!(
        blocks[3]["elements"][0]["url"],
        "https://github.com/acme/api/actions/runs/9201"
    );
    assert!(message["metadata"].get("flattened").is_none());
    assert_eq!(message["metadata"]["bot"]["name"], "ci");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn real_alerting_and_ci_payloads_land_as_attachments(pool: PgPool) {
    let h = hooked(pool).await;
    for name in ["grafana", "sentry", "jenkins"] {
        let (status, body) = post_hook(&h, fixture(name)).await;
        assert_eq!(status, StatusCode::OK, "{name}: {body:?}");
        let message = latest(&h).await;
        assert_eq!(message["metadata"]["flattened"], true, "{name}");
        assert!(
            message["metadata"]["legacy_attachments"][0]["color"]
                .as_str()
                .is_some_and(|c| c.starts_with('#')),
            "{name}: {message:?}"
        );
        assert!(!message["content"].as_str().unwrap_or_default().is_empty());
    }
    let message = latest(&h).await;
    assert_eq!(
        message["content"],
        "api-deploy - #42 Success after 3 min 12 sec ([Open](https://ci.example.com/job/api-deploy/42/))",
        "what notifications, email and search read"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn text_and_attachments_together_keep_the_text(pool: PgPool) {
    let h = hooked(pool).await;
    let (status, body) = post_hook(
        &h,
        json!({
            "text": "Nightly finished",
            "attachments": [{ "color": "danger", "fields": [{ "title": "Failed", "value": 3, "short": true }] }]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let message = latest(&h).await;
    assert_eq!(message["content"], "Nightly finished");
    assert!(message["metadata"].get("flattened").is_none());
    let attachment = &message["metadata"]["legacy_attachments"][0];
    assert_eq!(attachment["color"], "#a30200");
    assert_eq!(
        attachment["fields"][0]["value"], "3",
        "a number reads as its digits"
    );
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_unknown_block_and_the_fifty_first_are_refused_with_the_path(pool: PgPool) {
    let h = hooked(pool).await;
    let (status, body) = post_hook(
        &h,
        json!({ "blocks": [{ "type": "divider" }, { "type": "rich_text", "elements": [] }] }),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        body["error"],
        "blocks[1].type: unknown block type `rich_text`"
    );

    let fifty_one: Vec<Value> = (0..51)
        .map(|i| json!({ "type": "section", "text": { "type": "plain_text", "text": format!("line {i}") } }))
        .collect();
    let (status, body) = post_hook(&h, json!({ "blocks": fifty_one })).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"], "blocks: at most 50 blocks");

    let (status, body) = post_hook(
        &h,
        json!({ "blocks": [{ "type": "section", "text": { "type": "mrkdwn", "text": "x".repeat(3001) } }] }),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        body["error"],
        "blocks[0].text: too long, at most 3000 characters"
    );

    let (status, body) = post_hook(&h, json!({ "username": "nobody" })).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body:?}");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn an_image_on_a_private_address_is_refused(pool: PgPool) {
    let h = hooked(pool).await;
    for url in [
        "http://127.0.0.1:8080/admin.png",
        "http://169.254.169.254/latest/meta-data/",
        "http://10.0.0.7/chart.png",
    ] {
        let (status, body) = post_hook(
            &h,
            json!({ "blocks": [
                { "type": "section", "text": { "type": "mrkdwn", "text": "chart" },
                  "accessory": { "type": "image", "image_url": url, "alt_text": "chart" } }
            ] }),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{url}: {body:?}");
        assert!(
            body["error"]
                .as_str()
                .is_some_and(|e| e.starts_with("blocks[0].accessory.image_url: ")),
            "{url}: {body:?}"
        );
    }
    let (status, body) = post_hook(
        &h,
        json!({ "attachments": [{ "image_url": "http://192.168.1.1/x.png" }] }),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body:?}");
    assert!(body["error"]
        .as_str()
        .is_some_and(|e| e.starts_with("attachments[0].image_url: ")));
}

#[test_macros::db_test(migrations = "../migrations")]
async fn search_finds_a_word_that_is_only_in_a_block(pool: PgPool) {
    let h = hooked(pool).await;
    let (status, body) = post_hook(
        &h,
        json!({ "blocks": [
            { "type": "header", "text": { "type": "plain_text", "text": "Deploy" } },
            { "type": "section", "fields": [
                { "type": "mrkdwn", "text": "*Service:*\nquokkagateway" }
            ] }
        ] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");

    let (status, body) = send(
        &h.app,
        "GET",
        &format!("/api/v1/search?q=quokkagateway&workspace_id={}", h.ws),
        Some(&h.token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let hits = body["data"].as_array().expect("hits");
    assert_eq!(hits.len(), 1, "{body:?}");
    assert_eq!(hits[0]["content"], "Deploy\n**Service:**\nquokkagateway");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn people_send_markdown_not_blocks(pool: PgPool) {
    let h = hooked(pool).await;
    let (status, body) = send(
        &h.app,
        "POST",
        &format!("/api/v1/channels/{}/messages", h.ch),
        Some(&h.token),
        Some(json!({
            "content": "hello",
            "blocks": [{ "type": "divider" }]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body:?}");
    assert_eq!(body["error"], "Blocks and attachments are sent by apps");
}

#[test_macros::db_test(migrations = "../migrations")]
async fn a_mention_in_a_block_names_a_member_and_leaves_a_stranger_as_text(pool: PgPool) {
    let h = hooked(pool).await;
    let owner = h.owner_id;
    let (status, body) = post_hook(
        &h,
        json!({ "blocks": [{ "type": "section", "text": { "type": "mrkdwn", "text": format!("hi <@{owner}> and <@U0STRANGER>") } }] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let content = latest(&h).await["content"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(content.starts_with("hi @["), "{content}");
    assert!(content.contains(&format!("]({owner})")), "{content}");
    assert!(content.ends_with(" and @U0STRANGER"), "{content}");
}
