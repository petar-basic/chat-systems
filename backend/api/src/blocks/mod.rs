pub mod models;
pub mod mrkdwn;
pub mod parse;
pub mod repo;
pub mod shortcodes;

#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};

use serde_json::{Map, Value};
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use crate::hooks::ssrf;
use crate::state::AppState;
use models::{
    Block, BlockElement, BlockText, Composed, ContextElement, HomeView, Layout, TextKind, View,
};
use mrkdwn::Names;
use parse::{refuse, ImageRef};

const MAX_CONTENT_BYTES: usize = 4000;
const LAYOUT_KEYS: [&str; 3] = ["blocks", "legacy_attachments", "flattened"];

#[derive(Default, Clone, Copy)]
pub struct Input<'a> {
    pub text: Option<&'a str>,
    pub blocks: Option<&'a Value>,
    pub attachments: Option<&'a Value>,
}

impl Input<'_> {
    pub fn lays_out(&self) -> bool {
        [self.blocks, self.attachments]
            .into_iter()
            .any(|value| value.is_some_and(|value| !value.is_null()))
    }
}

impl Layout {
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty() && self.legacy_attachments.is_empty()
    }

    pub fn of(metadata: &Value) -> Layout {
        let read = |key: &str| metadata.get(key).cloned().unwrap_or(Value::Null);
        Layout {
            blocks: serde_json::from_value(read("blocks")).unwrap_or_default(),
            legacy_attachments: serde_json::from_value(read("legacy_attachments"))
                .unwrap_or_default(),
        }
    }
}

impl Composed {
    pub fn plain(text: &str) -> Composed {
        Composed {
            content: text.to_string(),
            layout: None,
            flattened: false,
        }
    }

    pub fn finish(text: Option<&str>, layout: Layout) -> Composed {
        if layout.is_empty() {
            return Composed::plain(text.unwrap_or_default());
        }
        let given = text.unwrap_or_default();
        let flattened = given.trim().is_empty();
        Composed {
            content: if flattened {
                flatten(&layout)
            } else {
                given.to_string()
            },
            layout: Some(layout),
            flattened,
        }
    }

    pub fn metadata(&self) -> Map<String, Value> {
        let mut out = Map::new();
        let Some(layout) = &self.layout else {
            return out;
        };
        if !layout.blocks.is_empty() {
            out.insert("blocks".into(), serde_json::json!(layout.blocks));
        }
        if !layout.legacy_attachments.is_empty() {
            out.insert(
                "legacy_attachments".into(),
                serde_json::json!(layout.legacy_attachments),
            );
        }
        if self.flattened {
            out.insert("flattened".into(), Value::Bool(true));
        }
        out
    }

    pub fn merged_into(&self, mut metadata: Value) -> Value {
        if let Some(object) = metadata.as_object_mut() {
            for key in LAYOUT_KEYS {
                object.remove(key);
            }
            object.extend(self.metadata());
        }
        metadata
    }
}

pub fn has_layout(metadata: &Value) -> bool {
    LAYOUT_KEYS[..2]
        .iter()
        .any(|key| metadata.get(key).is_some())
}

pub async fn compose(
    state: &AppState,
    workspace_id: Uuid,
    input: Input<'_>,
) -> AppResult<Composed> {
    let (mut layout, images) = parse::layout(input.blocks, input.attachments)?;
    if layout.is_empty() {
        return Ok(Composed::plain(input.text.unwrap_or_default()));
    }
    check_images(&images).await?;
    assign_ids(&mut layout);
    localize(state, workspace_id, &mut layout).await?;
    Ok(Composed::finish(input.text, layout))
}

pub async fn compose_view(state: &AppState, workspace_id: Uuid, value: &Value) -> AppResult<View> {
    let (view, images) = parse::view(value)?;
    check_images(&images).await?;
    let mut layout = Layout {
        blocks: view.blocks,
        legacy_attachments: Vec::new(),
    };
    assign_ids(&mut layout);
    localize(state, workspace_id, &mut layout).await?;
    Ok(View {
        blocks: layout.blocks,
        ..view
    })
}

pub async fn compose_home(
    state: &AppState,
    workspace_id: Uuid,
    value: &Value,
) -> AppResult<HomeView> {
    let (view, images) = parse::home(value)?;
    check_images(&images).await?;
    let mut layout = Layout {
        blocks: view.blocks,
        legacy_attachments: Vec::new(),
    };
    assign_ids(&mut layout);
    localize(state, workspace_id, &mut layout).await?;
    Ok(HomeView {
        blocks: layout.blocks,
        ..view
    })
}

pub async fn compose_unfurl(
    state: &AppState,
    workspace_id: Uuid,
    value: &Value,
    path: &str,
) -> AppResult<Vec<Block>> {
    let (blocks, images) = parse::unfurl(value, path)?;
    check_images(&images).await?;
    let mut layout = Layout {
        blocks,
        legacy_attachments: Vec::new(),
    };
    localize(state, workspace_id, &mut layout).await?;
    Ok(layout.blocks)
}

fn short_id() -> String {
    Uuid::new_v4().simple().to_string()[..8].to_string()
}

fn action_id_slot(element: &mut BlockElement) -> Option<&mut Option<String>> {
    match element {
        BlockElement::Image { .. } | BlockElement::Button { url: Some(_), .. } => None,
        BlockElement::Button { action_id, .. }
        | BlockElement::StaticSelect { action_id, .. }
        | BlockElement::UsersSelect { action_id, .. }
        | BlockElement::ConversationsSelect { action_id, .. }
        | BlockElement::Overflow { action_id, .. }
        | BlockElement::Datepicker { action_id, .. }
        | BlockElement::PlainTextInput { action_id, .. }
        | BlockElement::Checkboxes { action_id, .. }
        | BlockElement::RadioButtons { action_id, .. } => Some(action_id),
    }
}

fn assign_block_ids(block: &mut Block) {
    let (block_id, elements): (&mut Option<String>, Vec<&mut BlockElement>) = match block {
        Block::Section {
            accessory: Some(element),
            block_id,
            ..
        }
        | Block::Input {
            element, block_id, ..
        } => (block_id, vec![element]),
        Block::Actions { elements, block_id } => (block_id, elements.iter_mut().collect()),
        _ => return,
    };
    let mut interactive = false;
    for element in elements {
        if let Some(slot) = action_id_slot(element) {
            interactive = true;
            slot.get_or_insert_with(short_id);
        }
    }
    if interactive {
        block_id.get_or_insert_with(short_id);
    }
}

pub fn assign_ids(layout: &mut Layout) {
    let attached = layout
        .legacy_attachments
        .iter_mut()
        .flat_map(|attachment| attachment.blocks.iter_mut());
    for block in layout.blocks.iter_mut().chain(attached) {
        assign_block_ids(block);
    }
}

async fn localize(state: &AppState, workspace_id: Uuid, layout: &mut Layout) -> AppResult<()> {
    let (user_ids, channel_ids) = referenced(layout);
    let users: HashMap<String, (Uuid, String)> = if user_ids.is_empty() {
        HashMap::new()
    } else {
        state
            .block_repo
            .member_names(workspace_id, &user_ids)
            .await?
            .into_iter()
            .map(|(id, name)| (id.to_string(), (id, name)))
            .collect()
    };
    let channels: HashMap<String, String> = if channel_ids.is_empty() {
        HashMap::new()
    } else {
        state
            .block_repo
            .public_channel_names(workspace_id, &channel_ids)
            .await?
            .into_iter()
            .map(|(id, name)| (id.to_string(), name))
            .collect()
    };
    convert(
        layout,
        &Names {
            users: &users,
            channels: &channels,
        },
    );
    Ok(())
}

pub async fn check_images(images: &[ImageRef]) -> AppResult<()> {
    let mut seen = HashSet::new();
    let unique: Vec<&ImageRef> = images
        .iter()
        .filter(|image| seen.insert(image.url.as_str()))
        .collect();
    futures_util::future::try_join_all(unique.into_iter().map(|image| async move {
        ssrf::resolve_outbound(&image.url, false)
            .await
            .map(|_| ())
            .map_err(|e| match e {
                AppError::BadRequest(why) => refuse(&image.path, why),
                other => other,
            })
    }))
    .await?;
    Ok(())
}

fn each_text_object(text: &mut BlockText, f: &mut dyn FnMut(&TextKind, &mut String)) {
    f(&text.kind, &mut text.text);
}

fn each_element_text(element: &mut BlockElement, f: &mut dyn FnMut(&TextKind, &mut String)) {
    match element {
        BlockElement::Button { text, .. } => each_text_object(text, f),
        BlockElement::StaticSelect {
            placeholder,
            options,
            initial_option,
            ..
        } => {
            if let Some(placeholder) = placeholder {
                each_text_object(placeholder, f);
            }
            for option in options.iter_mut().chain(initial_option.iter_mut()) {
                each_text_object(&mut option.text, f);
            }
        }
        BlockElement::UsersSelect { placeholder, .. }
        | BlockElement::ConversationsSelect { placeholder, .. }
        | BlockElement::Datepicker { placeholder, .. } => {
            if let Some(placeholder) = placeholder {
                each_text_object(placeholder, f);
            }
        }
        BlockElement::Overflow { options, .. } => {
            for option in options {
                each_text_object(&mut option.text, f);
            }
        }
        BlockElement::PlainTextInput { placeholder, .. } => {
            if let Some(placeholder) = placeholder {
                each_text_object(placeholder, f);
            }
        }
        BlockElement::Checkboxes {
            options,
            initial_options,
            ..
        } => {
            for option in options.iter_mut().chain(initial_options.iter_mut()) {
                each_text_object(&mut option.text, f);
            }
        }
        BlockElement::RadioButtons {
            options,
            initial_option,
            ..
        } => {
            for option in options.iter_mut().chain(initial_option.iter_mut()) {
                each_text_object(&mut option.text, f);
            }
        }
        BlockElement::Image { .. } => {}
    }
}

fn each_block_text(block: &mut Block, f: &mut dyn FnMut(&TextKind, &mut String)) {
    match block {
        Block::Header { text, .. } => each_text_object(text, f),
        Block::Section {
            text,
            fields,
            accessory,
            ..
        } => {
            for text in text.iter_mut().chain(fields.iter_mut()) {
                each_text_object(text, f);
            }
            if let Some(accessory) = accessory {
                each_element_text(accessory, f);
            }
        }
        Block::Divider { .. } => {}
        Block::Image { title, .. } => {
            if let Some(title) = title {
                each_text_object(title, f);
            }
        }
        Block::Context { elements, .. } => {
            for element in elements {
                match element {
                    ContextElement::PlainText { text } => f(&TextKind::PlainText, text),
                    ContextElement::Mrkdwn { text } => f(&TextKind::Mrkdwn, text),
                    ContextElement::Image { .. } => {}
                }
            }
        }
        Block::Actions { elements, .. } => {
            for element in elements {
                each_element_text(element, f);
            }
        }
        Block::Input {
            label,
            element,
            hint,
            ..
        } => {
            for text in std::iter::once(label).chain(hint.iter_mut()) {
                each_text_object(text, f);
            }
            each_element_text(element, f);
        }
    }
}

fn each_text(layout: &mut Layout, f: &mut dyn FnMut(&TextKind, &mut String)) {
    for block in &mut layout.blocks {
        each_block_text(block, f);
    }
    for attachment in &mut layout.legacy_attachments {
        for text in [&mut attachment.author_name, &mut attachment.title]
            .into_iter()
            .flatten()
        {
            f(&TextKind::PlainText, text);
        }
        for text in [
            &mut attachment.pretext,
            &mut attachment.text,
            &mut attachment.footer,
        ]
        .into_iter()
        .flatten()
        {
            f(&TextKind::Mrkdwn, text);
        }
        for field in &mut attachment.fields {
            f(&TextKind::PlainText, &mut field.title);
            f(&TextKind::Mrkdwn, &mut field.value);
        }
        for block in &mut attachment.blocks {
            each_block_text(block, f);
        }
    }
}

fn entity_ids(text: &str, sigil: &str, into: &mut Vec<Uuid>) {
    let opener = format!("<{sigil}");
    let mut rest = text;
    while let Some(start) = rest.find(&opener) {
        let after = &rest[start + opener.len()..];
        let end = after.find(['>', '|']).unwrap_or(after.len());
        if let Ok(id) = Uuid::parse_str(&after[..end]) {
            if !into.contains(&id) {
                into.push(id);
            }
        }
        rest = &after[end..];
    }
}

pub fn referenced(layout: &mut Layout) -> (Vec<Uuid>, Vec<Uuid>) {
    let mut users = Vec::new();
    let mut channels = Vec::new();
    each_text(layout, &mut |kind, text| {
        if *kind == TextKind::Mrkdwn {
            entity_ids(text, "@", &mut users);
            entity_ids(text, "#", &mut channels);
        }
    });
    (users, channels)
}

pub fn convert(layout: &mut Layout, names: &Names) {
    each_text(layout, &mut |kind, text| {
        *text = match kind {
            TextKind::Mrkdwn => mrkdwn::convert_block_text(text, names),
            TextKind::PlainText => mrkdwn::emojify(text),
        };
    });
}

fn block_lines(block: &Block, lines: &mut Vec<String>) {
    match block {
        Block::Header { text, .. } => lines.push(text.text.clone()),
        Block::Section { text, fields, .. } => {
            lines.extend(text.iter().chain(fields.iter()).map(|t| t.text.clone()));
        }
        Block::Image { title, .. } => lines.extend(title.iter().map(|t| t.text.clone())),
        Block::Context { elements, .. } => {
            let words: Vec<&str> = elements
                .iter()
                .filter_map(|element| match element {
                    ContextElement::PlainText { text } | ContextElement::Mrkdwn { text } => {
                        Some(text.as_str())
                    }
                    ContextElement::Image { .. } => None,
                })
                .collect();
            lines.push(words.join(" "));
        }
        Block::Divider { .. } | Block::Actions { .. } | Block::Input { .. } => {}
    }
}

pub fn flatten(layout: &Layout) -> String {
    let mut lines = Vec::new();
    for block in &layout.blocks {
        block_lines(block, &mut lines);
    }
    for attachment in &layout.legacy_attachments {
        lines.extend(
            [
                &attachment.pretext,
                &attachment.author_name,
                &attachment.title,
                &attachment.text,
            ]
            .into_iter()
            .flatten()
            .cloned(),
        );
        for field in &attachment.fields {
            lines.push(match (field.title.is_empty(), field.value.is_empty()) {
                (false, false) => format!("{}: {}", field.title, field.value),
                (true, _) => field.value.clone(),
                (_, true) => field.title.clone(),
            });
        }
        for block in &attachment.blocks {
            block_lines(block, &mut lines);
        }
        lines.extend(attachment.footer.iter().cloned());
    }
    let text = lines
        .iter()
        .map(|line| line.replace("\\\n", "\n").trim().to_string())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    shorten(text)
}

fn shorten(text: String) -> String {
    if text.len() <= MAX_CONTENT_BYTES {
        return text;
    }
    let ellipsis = "…";
    let mut end = MAX_CONTENT_BYTES - ellipsis.len();
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{ellipsis}", &text[..end])
}
