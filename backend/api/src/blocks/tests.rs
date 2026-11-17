use std::collections::HashMap;

use serde_json::{json, Value};

use super::models::{Block, BlockElement, Composed, TextKind};
use super::mrkdwn::Names;
use super::{assign_ids, check_images, convert, flatten, parse};

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

fn view_refusal(view: Value) -> String {
    match parse::view(&view) {
        Err(shared_common::errors::AppError::Validation(why)) => why,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

fn input(block_id: &str, element: Value) -> Value {
    json!({
        "type": "input",
        "block_id": block_id,
        "label": { "type": "plain_text", "text": "Label" },
        "element": element,
    })
}

fn modal(blocks: Value) -> Value {
    json!({
        "type": "modal",
        "title": { "type": "plain_text", "text": "Request" },
        "submit": { "type": "plain_text", "text": "Send" },
        "blocks": blocks,
    })
}

#[test]
fn a_view_takes_inputs_that_a_message_refuses() {
    let text = input(
        "why",
        json!({ "type": "plain_text_input", "action_id": "text" }),
    );
    assert_eq!(
        refusal(json!([text.clone()])),
        "blocks[0].type: unknown block type `input`"
    );
    let (view, _) = parse::view(&modal(json!([
        text,
        input("pick", json!({ "type": "checkboxes", "action_id": "boxes",
            "options": [{ "text": { "type": "mrkdwn", "text": "*Yes*" }, "value": "yes" }],
            "initial_options": [{ "text": { "type": "mrkdwn", "text": "*Yes*" }, "value": "yes" }] })),
        input("one", json!({ "type": "radio_buttons", "action_id": "radio",
            "options": [{ "text": { "type": "plain_text", "text": "A" }, "value": "a" }] })),
    ])))
    .expect("a view");
    assert_eq!(view.blocks.len(), 3);
    assert!(matches!(
        &view.blocks[1],
        Block::Input {
            element: BlockElement::Checkboxes { .. },
            ..
        }
    ));
}

#[test]
fn a_view_is_checked_the_way_slack_checks_it() {
    let text = || {
        input(
            "why",
            json!({ "type": "plain_text_input", "action_id": "text" }),
        )
    };
    let mut no_submit = modal(json!([text()]));
    no_submit.as_object_mut().expect("object").remove("submit");
    assert_eq!(
        view_refusal(no_submit),
        "view.submit: a view with inputs needs a submit label"
    );
    let mut long_title = modal(json!([]));
    long_title["title"]["text"] = json!("A title well past what fits");
    assert_eq!(
        view_refusal(long_title),
        "view.title: too long, at most 24 characters"
    );
    assert_eq!(
        view_refusal(modal(json!([input(
            "why",
            json!({ "type": "button",
            "text": { "type": "plain_text", "text": "No" } })
        )]))),
        "view.blocks[0].element.type: unknown element type `button`"
    );
    assert_eq!(
        view_refusal(modal(json!([text(), text()]))),
        "view.blocks[1].block_id: `why` is used by another block"
    );
    assert_eq!(
        view_refusal(modal(json!([input(
            "why",
            json!({ "type": "radio_buttons",
            "action_id": "radio",
            "options": [{ "text": { "type": "plain_text", "text": "A" }, "value": "a" }],
            "initial_option": { "text": { "type": "plain_text", "text": "B" }, "value": "b" } })
        )]))),
        "view.blocks[0].element.initial_option: must be one of the options"
    );
    assert_eq!(
        view_refusal(modal(json!([input(
            "why",
            json!({ "type": "plain_text_input",
            "action_id": "text", "min_length": 10, "max_length": 5 })
        )]))),
        "view.blocks[0].element.min_length: cannot be more than max_length"
    );
    let many: Vec<Value> = (0..101).map(|_| json!({ "type": "divider" })).collect();
    assert_eq!(
        view_refusal(modal(json!(many))),
        "view.blocks: at most 100 blocks"
    );
    assert_eq!(
        view_refusal(json!({ "type": "home", "blocks": [] })),
        "view.type: unknown view type `home`"
    );
}

#[test]
fn an_action_id_is_used_once_in_a_block() {
    let button = |id: &str| json!({ "type": "button", "action_id": id, "text": { "type": "plain_text", "text": id } });
    assert_eq!(
        refusal(json!([{ "type": "actions", "elements": [button("go"), button("go")] }])),
        "blocks[0]: action_id `go` is used twice in this block"
    );
}

#[test]
fn every_element_that_reaches_the_app_gets_ids() {
    let (mut layout, _) = parse::layout(
        Some(&json!([
            { "type": "actions", "elements": [
                { "type": "button", "text": { "type": "plain_text", "text": "Go" } },
                { "type": "button", "text": { "type": "plain_text", "text": "Docs" },
                  "url": "https://example.com/docs" }
            ] },
            { "type": "section", "text": { "type": "mrkdwn", "text": "Just words" } },
            { "type": "section", "text": { "type": "mrkdwn", "text": "With a link" },
              "accessory": { "type": "button", "url": "https://example.com",
                "text": { "type": "plain_text", "text": "Open" } } }
        ])),
        None,
    )
    .expect("parses");
    assign_ids(&mut layout);
    let Block::Actions { elements, block_id } = &layout.blocks[0] else {
        panic!("actions");
    };
    assert!(block_id.is_some());
    assert!(parse::action_id(&elements[0]).is_some());
    assert!(
        parse::action_id(&elements[1]).is_none(),
        "a link reaches no app"
    );
    assert!(parse::block_id(&layout.blocks[1]).is_none());
    assert!(parse::block_id(&layout.blocks[2]).is_none());
}
