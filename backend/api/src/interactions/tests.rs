use serde_json::json;
use uuid::Uuid;

use super::models::StoredView;
use super::{app_views, find_element, message_blocks};
use crate::blocks::models::{Block, BlockElement, Layout};
use crate::blocks::parse;

fn layout(value: serde_json::Value) -> Layout {
    let (layout, _) = parse::layout(Some(&value), None).expect("valid blocks");
    layout
}

#[test]
fn an_element_is_found_by_its_block_and_action_ids() {
    let layout = layout(json!([
        { "type": "section", "block_id": "pick",
          "text": { "type": "mrkdwn", "text": "On call" },
          "accessory": { "type": "users_select", "action_id": "person" } },
        { "type": "actions", "block_id": "deploy", "elements": [
            { "type": "button", "action_id": "approve", "value": "v2",
              "text": { "type": "plain_text", "text": "Approve" } },
            { "type": "button", "action_id": "reject",
              "text": { "type": "plain_text", "text": "Reject" } }
        ] }
    ]));
    let blocks = message_blocks(&layout);
    assert!(matches!(
        find_element(blocks.iter().copied(), "deploy", "reject"),
        Some(BlockElement::Button { .. })
    ));
    assert!(matches!(
        find_element(blocks.iter().copied(), "pick", "person"),
        Some(BlockElement::UsersSelect { .. })
    ));
    assert!(find_element(blocks.iter().copied(), "pick", "approve").is_none());
    assert!(find_element(blocks.iter().copied(), "deploy", "missing").is_none());
}

#[test]
fn an_input_is_read_on_submission_and_is_not_an_action() {
    let (view, _) = parse::view(&json!({
        "type": "modal",
        "title": { "type": "plain_text", "text": "Request" },
        "submit": { "type": "plain_text", "text": "Send" },
        "blocks": [
            { "type": "input", "block_id": "why",
              "label": { "type": "plain_text", "text": "Why" },
              "element": { "type": "plain_text_input", "action_id": "text" } }
        ]
    }))
    .expect("a valid view");
    assert!(matches!(view.blocks[0], Block::Input { .. }));
    assert!(find_element(&view.blocks, "why", "text").is_none());
}

fn stored(stack_id: Uuid, position: i16) -> StoredView {
    StoredView {
        id: Uuid::new_v4(),
        stack_id,
        position,
        app_id: Uuid::new_v4(),
        workspace_id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        view: json!({
            "type": "modal",
            "title": { "type": "plain_text", "text": format!("View {position}") },
            "submit": null,
            "close": null,
            "blocks": [],
            "private_metadata": null,
            "callback_id": null,
            "external_id": null,
        }),
        hash: format!("hash-{position}"),
    }
}

#[test]
fn a_stack_names_its_root_and_each_view_below() {
    let stack_id = Uuid::new_v4();
    let stack = vec![
        stored(stack_id, 0),
        stored(stack_id, 1),
        stored(stack_id, 2),
    ];
    let views = app_views(&stack).expect("views");
    assert_eq!(views.len(), 3);
    assert!(views.iter().all(|view| view.root_view_id == stack[0].id));
    assert_eq!(views[0].previous_view_id, None);
    assert_eq!(views[1].previous_view_id, Some(stack[0].id));
    assert_eq!(views[2].previous_view_id, Some(stack[1].id));
    assert_eq!(views[2].view.title.text, "View 2");
}
