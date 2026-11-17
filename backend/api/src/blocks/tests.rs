use std::collections::HashMap;

use serde_json::{json, Value};

use super::models::{Block, BlockElement, Composed, TextKind};
use super::mrkdwn::Names;
use super::{check_images, convert, flatten, parse};

fn fixture(name: &str) -> Value {
    let text = match name {
        "github_actions" => include_str!("fixtures/github_actions.json"),
        "grafana" => include_str!("fixtures/grafana.json"),
        "sentry" => include_str!("fixtures/sentry.json"),
        "jenkins" => include_str!("fixtures/jenkins.json"),
        other => panic!("no fixture {other}"),
    };
    serde_json::from_str(text).expect("fixture json")
}

fn composed(payload: &Value) -> Composed {
    let (mut layout, _) =
        parse::layout(payload.get("blocks"), payload.get("attachments")).expect("parses");
    let users = HashMap::new();
    let channels = HashMap::new();
    convert(
        &mut layout,
        &Names {
            users: &users,
            channels: &channels,
        },
    );
    Composed::finish(payload.get("text").and_then(Value::as_str), layout)
}

fn refusal(blocks: Value) -> String {
    match parse::layout(Some(&blocks), None) {
        Err(shared_common::errors::AppError::Validation(why)) => why,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn a_github_actions_payload_keeps_its_header_fields_and_link_button() {
    let composed = composed(&fixture("github_actions"));
    assert!(!composed.flattened, "the payload brought its own text");
    assert_eq!(composed.content, "GitHub Action build result: success");
    let blocks = composed.layout.expect("laid out").blocks;
    assert!(matches!(&blocks[0], Block::Header { text, .. } if text.text == "✅ Build passed"));
    let Block::Section { fields, .. } = &blocks[1] else {
        panic!("a section of fields");
    };
    assert_eq!(fields.len(), 4);
    assert_eq!(
        fields[0].text,
        "**Repository:**\\\n[acme/api](https://github.com/acme/api)"
    );
    let Block::Actions { elements, .. } = &blocks[3] else {
        panic!("the actions");
    };
    assert!(matches!(
        &elements[0],
        BlockElement::Button { url: Some(url), .. } if url == "https://github.com/acme/api/actions/runs/9201"
    ));
}

#[test]
fn attachments_alone_are_flattened_for_notifications_and_search() {
    let grafana = composed(&fixture("grafana"));
    assert!(grafana.flattened);
    let attachment = &grafana
        .layout
        .as_ref()
        .expect("laid out")
        .legacy_attachments[0];
    assert_eq!(attachment.color.as_deref(), Some("#d63232"));
    assert_eq!(attachment.ts, Some(1_700_000_000));
    assert!(grafana
        .content
        .starts_with("[FIRING:1] High CPU (api-1)\n**Firing**"));
    assert!(grafana.content.ends_with("Grafana v10.4.1"));

    let sentry = composed(&fixture("sentry"));
    let attachment = &sentry.layout.as_ref().expect("laid out").legacy_attachments[0];
    assert_eq!(attachment.ts, Some(1_700_000_123), "a ts sent as a string");
    assert_eq!(
        attachment.footer.as_deref(),
        Some("[BACKEND-3QZ](https://sentry.io/organizations/acme/issues/4512/)")
    );
    assert!(sentry.content.contains("Environment: production"));

    let jenkins = composed(&fixture("jenkins"));
    let attachment = &jenkins
        .layout
        .as_ref()
        .expect("laid out")
        .legacy_attachments[0];
    assert_eq!(
        attachment.color.as_deref(),
        Some("#2eb886"),
        "Slack's named good"
    );
    assert_eq!(
        jenkins.content,
        "api-deploy - #42 Success after 3 min 12 sec ([Open](https://ci.example.com/job/api-deploy/42/))"
    );
}

#[test]
fn blocks_without_text_are_flattened_in_order() {
    let composed = composed(&json!({
        "blocks": [
            { "type": "header", "text": { "type": "plain_text", "text": "Release 4.8.0" } },
            { "type": "section", "text": { "type": "mrkdwn", "text": "Shipped *today*" } },
            { "type": "context", "elements": [
                { "type": "image", "image_url": "https://1.1.1.1/a.png", "alt_text": "logo" },
                { "type": "mrkdwn", "text": "by" },
                { "type": "plain_text", "text": "Releases" }
            ] },
            { "type": "actions", "elements": [
                { "type": "button", "text": { "type": "plain_text", "text": "Approve" }, "action_id": "approve" }
            ] }
        ]
    }));
    assert!(composed.flattened);
    assert_eq!(
        composed.content,
        "Release 4.8.0\nShipped **today**\nby Releases"
    );
}

#[test]
fn an_unknown_block_type_is_refused_with_its_path() {
    let why = refusal(json!([
        { "type": "divider" },
        { "type": "table", "rows": [] }
    ]));
    assert_eq!(why, "blocks[1].type: unknown block type `table`");
    let why = refusal(json!([
        { "type": "actions", "elements": [
            { "type": "button", "text": { "type": "plain_text", "text": "Go" } },
            { "type": "checkboxes", "options": [] }
        ] }
    ]));
    assert_eq!(
        why,
        "blocks[0].elements[1].type: unknown element type `checkboxes`"
    );
}

#[test]
fn the_fifty_first_block_is_refused() {
    let fifty: Vec<Value> = (0..50).map(|_| json!({ "type": "divider" })).collect();
    assert!(parse::layout(Some(&json!(fifty)), None).is_ok());
    let mut fifty_one = fifty;
    fifty_one.push(json!({ "type": "divider" }));
    assert_eq!(refusal(json!(fifty_one)), "blocks: at most 50 blocks");
}

#[test]
fn slacks_lengths_and_shapes_are_checked() {
    let long = "x".repeat(151);
    assert_eq!(
        refusal(json!([{ "type": "header", "text": { "type": "plain_text", "text": long } }])),
        "blocks[0].text: too long, at most 150 characters"
    );
    assert_eq!(
        refusal(json!([{ "type": "header", "text": { "type": "mrkdwn", "text": "*hi*" } }])),
        "blocks[0].text: must be plain_text"
    );
    assert_eq!(
        refusal(json!([{ "type": "section" }])),
        "blocks[0]: a section needs text or fields"
    );
    let eleven: Vec<Value> = (0..11)
        .map(|i| json!({ "type": "mrkdwn", "text": format!("f{i}") }))
        .collect();
    assert_eq!(
        refusal(json!([{ "type": "section", "fields": eleven }])),
        "blocks[0].fields: at most 10 fields"
    );
    assert_eq!(
        refusal(json!([{ "type": "actions", "elements": [
            { "type": "button", "text": { "type": "plain_text", "text": "Run" }, "url": "javascript:alert(1)" }
        ] }])),
        "blocks[0].elements[0].url: must be an http or https address"
    );
    assert!(refusal(json!([{ "type": "image", "alt_text": "x" }]))
        .starts_with("blocks[0]: missing field `image_url`"));
}

#[test]
fn a_mrkdwn_text_keeps_its_kind() {
    let composed = composed(&json!({
        "blocks": [{ "type": "section", "text": { "type": "plain_text", "text": "*not bold* :tada:" } }]
    }));
    let Block::Section {
        text: Some(text), ..
    } = &composed.layout.expect("laid out").blocks[0]
    else {
        panic!("a section");
    };
    assert_eq!(text.kind, TextKind::PlainText);
    assert_eq!(
        text.text, "*not bold* 🎉",
        "plain text is not markup, but its emoji are named"
    );
}

#[tokio::test]
async fn an_image_on_a_private_address_is_refused() {
    let (_, images) = parse::layout(
        Some(&json!([
            { "type": "image", "image_url": "https://1.1.1.1/chart.png", "alt_text": "chart" },
            { "type": "image", "image_url": "http://169.254.169.254/latest/meta-data", "alt_text": "metadata" }
        ])),
        None,
    )
    .expect("parses");
    match check_images(&images).await {
        Err(shared_common::errors::AppError::Validation(why)) => {
            assert!(why.starts_with("blocks[1].image_url: "), "{why}");
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
    let (_, public) = parse::layout(
        Some(&json!([{ "type": "image", "image_url": "https://1.1.1.1/chart.png", "alt_text": "chart" }])),
        None,
    )
    .expect("parses");
    assert!(check_images(&public).await.is_ok());
}

#[test]
fn a_long_flattening_is_cut_to_what_a_message_holds() {
    let blocks: Vec<Value> = (0..5)
        .map(|_| json!({ "type": "section", "text": { "type": "plain_text", "text": "é".repeat(1500) } }))
        .collect();
    let (layout, _) = parse::layout(Some(&json!(blocks)), None).expect("parses");
    let flat = flatten(&layout);
    assert!(flat.len() <= 4000);
    assert!(flat.ends_with('…'));
}
