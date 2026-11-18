use std::fmt::Display;

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;

use shared_common::errors::{AppError, AppResult};

use super::models::{
    Block, BlockElement, BlockOption, BlockText, ContextElement, HomeView, Layout,
    LegacyAttachment, LegacyField, TextKind, View,
};

pub const MAX_BLOCKS: usize = 50;
pub const MAX_VIEW_BLOCKS: usize = 100;
pub const MAX_ATTACHMENTS: usize = 20;
const MAX_FIELDS: usize = 10;
const MAX_ATTACHMENT_FIELDS: usize = 20;
const MAX_CONTEXT_ELEMENTS: usize = 10;
const MAX_ACTION_ELEMENTS: usize = 25;
const MAX_SELECT_OPTIONS: usize = 100;
const MAX_OVERFLOW_OPTIONS: usize = 5;
const MAX_URL: usize = 3000;
const MAX_ID: usize = 255;
const MAX_ATTACHMENT_TEXT: usize = 8000;
const MAX_ATTACHMENT_LINE: usize = 3000;
const MAX_CHOICES: usize = 10;
const MAX_INPUT: u32 = 3000;
const MAX_VIEW_TITLE: usize = 24;
const MAX_PRIVATE_METADATA: usize = 3000;

const BLOCK_TYPES: [&str; 6] = [
    "header", "section", "divider", "image", "context", "actions",
];
const VIEW_BLOCK_TYPES: [&str; 7] = [
    "header", "section", "divider", "image", "context", "actions", "input",
];
const INPUT_TYPES: [&str; 7] = [
    "plain_text_input",
    "checkboxes",
    "radio_buttons",
    "static_select",
    "users_select",
    "conversations_select",
    "datepicker",
];
const ACTION_TYPES: [&str; 6] = [
    "button",
    "static_select",
    "users_select",
    "conversations_select",
    "overflow",
    "datepicker",
];
const ACCESSORY_TYPES: [&str; 7] = [
    "button",
    "static_select",
    "users_select",
    "conversations_select",
    "overflow",
    "datepicker",
    "image",
];
const CONTEXT_TYPES: [&str; 3] = ["plain_text", "mrkdwn", "image"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    Message,
    View,
    Home,
}

impl Surface {
    fn block_types(self) -> &'static [&'static str] {
        match self {
            Surface::Message | Surface::Home => &BLOCK_TYPES,
            Surface::View => &VIEW_BLOCK_TYPES,
        }
    }

    fn max_blocks(self) -> usize {
        match self {
            Surface::Message => MAX_BLOCKS,
            Surface::View | Surface::Home => MAX_VIEW_BLOCKS,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImageRef {
    pub path: String,
    pub url: String,
}

pub fn refuse(path: &str, why: impl Display) -> AppError {
    AppError::Validation(format!("{path}: {why}"))
}

pub fn layout(
    blocks: Option<&Value>,
    attachments: Option<&Value>,
) -> AppResult<(Layout, Vec<ImageRef>)> {
    let mut images = Vec::new();
    let blocks = match blocks {
        None | Some(Value::Null) => Vec::new(),
        Some(value) => blocks_at(value, "blocks", Surface::Message, &mut images)?,
    };
    let legacy_attachments = match attachments {
        None | Some(Value::Null) => Vec::new(),
        Some(value) => attachments_at(value, &mut images)?,
    };
    let layout = Layout {
        blocks,
        legacy_attachments,
    };
    let mut seen = Vec::new();
    unique_ids(&layout.blocks, "blocks", &mut seen)?;
    for (i, attachment) in layout.legacy_attachments.iter().enumerate() {
        unique_ids(
            &attachment.blocks,
            &format!("attachments[{i}].blocks"),
            &mut seen,
        )?;
    }
    Ok((layout, images))
}

pub fn view(value: &Value) -> AppResult<(View, Vec<ImageRef>)> {
    if !value.is_object() {
        return Err(refuse("view", "must be an object"));
    }
    type_at(value, "view", &["modal"], "view")?;
    let mut images = Vec::new();
    let blocks = blocks_at(
        value
            .get("blocks")
            .ok_or_else(|| refuse("view.blocks", "missing"))?,
        "view.blocks",
        Surface::View,
        &mut images,
    )?;
    let mut view: View = typed(value, "view")?;
    view.blocks = blocks;
    plain(&view.title, "view.title", MAX_VIEW_TITLE)?;
    for (name, label) in [("submit", &view.submit), ("close", &view.close)] {
        if let Some(label) = label {
            plain(label, &format!("view.{name}"), MAX_VIEW_TITLE)?;
        }
    }
    let has_inputs = view
        .blocks
        .iter()
        .any(|block| matches!(block, Block::Input { .. }));
    if has_inputs && view.submit.is_none() {
        return Err(refuse(
            "view.submit",
            "a view with inputs needs a submit label",
        ));
    }
    optional(
        view.private_metadata.as_ref(),
        "view.private_metadata",
        MAX_PRIVATE_METADATA,
    )?;
    optional(view.callback_id.as_ref(), "view.callback_id", MAX_ID)?;
    optional(view.external_id.as_ref(), "view.external_id", MAX_ID)?;
    unique_ids(&view.blocks, "view.blocks", &mut Vec::new())?;
    Ok((view, images))
}

pub fn home(value: &Value) -> AppResult<(HomeView, Vec<ImageRef>)> {
    if !value.is_object() {
        return Err(refuse("view", "must be an object"));
    }
    type_at(value, "view", &["home"], "view")?;
    let mut images = Vec::new();
    let blocks = blocks_at(
        value
            .get("blocks")
            .ok_or_else(|| refuse("view.blocks", "missing"))?,
        "view.blocks",
        Surface::Home,
        &mut images,
    )?;
    let mut view: HomeView = typed(value, "view")?;
    view.blocks = blocks;
    optional(
        view.private_metadata.as_ref(),
        "view.private_metadata",
        MAX_PRIVATE_METADATA,
    )?;
    optional(view.callback_id.as_ref(), "view.callback_id", MAX_ID)?;
    optional(view.external_id.as_ref(), "view.external_id", MAX_ID)?;
    unique_ids(&view.blocks, "view.blocks", &mut Vec::new())?;
    Ok((view, images))
}

pub fn block_id(block: &Block) -> Option<&String> {
    match block {
        Block::Header { block_id, .. }
        | Block::Section { block_id, .. }
        | Block::Divider { block_id }
        | Block::Image { block_id, .. }
        | Block::Context { block_id, .. }
        | Block::Actions { block_id, .. }
        | Block::Input { block_id, .. } => block_id.as_ref(),
    }
}

pub fn action_id(element: &BlockElement) -> Option<&String> {
    match element {
        BlockElement::Button { action_id, .. }
        | BlockElement::StaticSelect { action_id, .. }
        | BlockElement::UsersSelect { action_id, .. }
        | BlockElement::ConversationsSelect { action_id, .. }
        | BlockElement::Overflow { action_id, .. }
        | BlockElement::Datepicker { action_id, .. }
        | BlockElement::PlainTextInput { action_id, .. }
        | BlockElement::Checkboxes { action_id, .. }
        | BlockElement::RadioButtons { action_id, .. } => action_id.as_ref(),
        BlockElement::Image { .. } => None,
    }
}

pub fn elements(block: &Block) -> Vec<&BlockElement> {
    match block {
        Block::Section {
            accessory: Some(element),
            ..
        }
        | Block::Input { element, .. } => vec![element],
        Block::Actions { elements, .. } => elements.iter().collect(),
        _ => Vec::new(),
    }
}

fn unique_ids(blocks: &[Block], path: &str, seen: &mut Vec<String>) -> AppResult<()> {
    for (i, block) in blocks.iter().enumerate() {
        if let Some(id) = block_id(block) {
            if seen.contains(id) {
                return Err(refuse(
                    &format!("{path}[{i}].block_id"),
                    format!("`{id}` is used by another block"),
                ));
            }
            seen.push(id.clone());
        }
        let mut actions: Vec<&String> = Vec::new();
        for element in elements(block) {
            if let Some(id) = action_id(element) {
                if actions.contains(&id) {
                    return Err(refuse(
                        &format!("{path}[{i}]"),
                        format!("action_id `{id}` is used twice in this block"),
                    ));
                }
                actions.push(id);
            }
        }
    }
    Ok(())
}

fn list<'a>(value: &'a Value, path: &str) -> AppResult<&'a Vec<Value>> {
    value
        .as_array()
        .ok_or_else(|| refuse(path, "must be a list"))
}

fn typed<T: DeserializeOwned>(value: &Value, path: &str) -> AppResult<T> {
    serde_path_to_error::deserialize(value).map_err(|e| {
        let inner = e.path().to_string();
        let at = match inner.as_str() {
            "" | "." => path.to_string(),
            nested if nested.starts_with('[') => format!("{path}{nested}"),
            nested => format!("{path}.{nested}"),
        };
        refuse(&at, e.into_inner())
    })
}

fn type_at<'a>(value: &'a Value, path: &str, known: &[&str], what: &str) -> AppResult<&'a str> {
    let at = format!("{path}.type");
    let kind = value
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| refuse(&at, "missing"))?;
    if !known.contains(&kind) {
        return Err(refuse(&at, format!("unknown {what} type `{kind}`")));
    }
    Ok(kind)
}

fn blocks_at(
    value: &Value,
    path: &str,
    surface: Surface,
    images: &mut Vec<ImageRef>,
) -> AppResult<Vec<Block>> {
    let items = list(value, path)?;
    let max = surface.max_blocks();
    if items.len() > max {
        return Err(refuse(path, format!("at most {max} blocks")));
    }
    items
        .iter()
        .enumerate()
        .map(|(i, item)| block_at(item, &format!("{path}[{i}]"), surface, images))
        .collect()
}

fn block_at(
    value: &Value,
    path: &str,
    surface: Surface,
    images: &mut Vec<ImageRef>,
) -> AppResult<Block> {
    match type_at(value, path, surface.block_types(), "block")? {
        "section" => {
            if let Some(accessory) = value.get("accessory").filter(|a| !a.is_null()) {
                type_at(
                    accessory,
                    &format!("{path}.accessory"),
                    &ACCESSORY_TYPES,
                    "element",
                )?;
            }
        }
        "actions" => nested_types(value, path, &ACTION_TYPES)?,
        "context" => nested_types(value, path, &CONTEXT_TYPES)?,
        "input" => {
            let element = value
                .get("element")
                .ok_or_else(|| refuse(&format!("{path}.element"), "missing"))?;
            type_at(element, &format!("{path}.element"), &INPUT_TYPES, "element")?;
        }
        _ => {}
    }
    let block: Block = typed(value, path)?;
    check_block(&block, path, images)?;
    Ok(block)
}

fn nested_types(value: &Value, path: &str, known: &[&str]) -> AppResult<()> {
    if let Some(elements) = value.get("elements").and_then(Value::as_array) {
        for (j, element) in elements.iter().enumerate() {
            type_at(element, &format!("{path}.elements[{j}]"), known, "element")?;
        }
    }
    Ok(())
}

fn length(text: &str, path: &str, max: usize, required: bool) -> AppResult<()> {
    if required && text.trim().is_empty() {
        return Err(refuse(path, "cannot be empty"));
    }
    if text.chars().count() > max {
        return Err(refuse(path, format!("too long, at most {max} characters")));
    }
    Ok(())
}

fn any_text(text: &BlockText, path: &str, max: usize) -> AppResult<()> {
    length(&text.text, path, max, true)
}

fn plain(text: &BlockText, path: &str, max: usize) -> AppResult<()> {
    if text.kind != TextKind::PlainText {
        return Err(refuse(path, "must be plain_text"));
    }
    length(&text.text, path, max, true)
}

fn optional(text: Option<&String>, path: &str, max: usize) -> AppResult<()> {
    text.map_or(Ok(()), |text| length(text, path, max, false))
}

fn web_address(url: &str, path: &str) -> AppResult<()> {
    if url.chars().count() > MAX_URL {
        return Err(refuse(
            path,
            format!("too long, at most {MAX_URL} characters"),
        ));
    }
    match reqwest::Url::parse(url) {
        Ok(parsed) if matches!(parsed.scheme(), "http" | "https") && parsed.has_host() => Ok(()),
        _ => Err(refuse(path, "must be an http or https address")),
    }
}

fn image(url: &str, path: &str, images: &mut Vec<ImageRef>) -> AppResult<()> {
    web_address(url, path)?;
    images.push(ImageRef {
        path: path.to_string(),
        url: url.to_string(),
    });
    Ok(())
}

fn count(len: usize, path: &str, min: usize, max: usize, what: &str) -> AppResult<()> {
    if len < min {
        return Err(refuse(path, format!("needs at least {min} {what}")));
    }
    if len > max {
        return Err(refuse(path, format!("at most {max} {what}")));
    }
    Ok(())
}

fn check_block(block: &Block, path: &str, images: &mut Vec<ImageRef>) -> AppResult<()> {
    let block_id = match block {
        Block::Header { text, block_id } => {
            plain(text, &format!("{path}.text"), 150)?;
            block_id
        }
        Block::Section {
            text,
            fields,
            accessory,
            block_id,
        } => {
            if text.is_none() && fields.is_empty() {
                return Err(refuse(path, "a section needs text or fields"));
            }
            if let Some(text) = text {
                any_text(text, &format!("{path}.text"), 3000)?;
            }
            count(
                fields.len(),
                &format!("{path}.fields"),
                0,
                MAX_FIELDS,
                "fields",
            )?;
            for (i, field) in fields.iter().enumerate() {
                any_text(field, &format!("{path}.fields[{i}]"), 2000)?;
            }
            if let Some(accessory) = accessory {
                check_element(accessory, &format!("{path}.accessory"), images)?;
            }
            block_id
        }
        Block::Divider { block_id } => block_id,
        Block::Image {
            image_url,
            alt_text,
            title,
            block_id,
        } => {
            image(image_url, &format!("{path}.image_url"), images)?;
            length(alt_text, &format!("{path}.alt_text"), 2000, true)?;
            if let Some(title) = title {
                plain(title, &format!("{path}.title"), 2000)?;
            }
            block_id
        }
        Block::Context { elements, block_id } => {
            count(
                elements.len(),
                &format!("{path}.elements"),
                1,
                MAX_CONTEXT_ELEMENTS,
                "elements",
            )?;
            for (j, element) in elements.iter().enumerate() {
                let at = format!("{path}.elements[{j}]");
                match element {
                    ContextElement::PlainText { text } | ContextElement::Mrkdwn { text } => {
                        length(text, &at, 3000, true)?;
                    }
                    ContextElement::Image {
                        image_url,
                        alt_text,
                    } => {
                        image(image_url, &format!("{at}.image_url"), images)?;
                        length(alt_text, &format!("{at}.alt_text"), 2000, true)?;
                    }
                }
            }
            block_id
        }
        Block::Actions { elements, block_id } => {
            count(
                elements.len(),
                &format!("{path}.elements"),
                1,
                MAX_ACTION_ELEMENTS,
                "elements",
            )?;
            for (j, element) in elements.iter().enumerate() {
                check_element(element, &format!("{path}.elements[{j}]"), images)?;
            }
            block_id
        }
        Block::Input {
            label,
            element,
            hint,
            block_id,
            ..
        } => {
            plain(label, &format!("{path}.label"), 2000)?;
            if let Some(hint) = hint {
                plain(hint, &format!("{path}.hint"), 2000)?;
            }
            check_element(element, &format!("{path}.element"), images)?;
            block_id
        }
    };
    optional(block_id.as_ref(), &format!("{path}.block_id"), MAX_ID)
}

fn check_placeholder(placeholder: Option<&BlockText>, path: &str) -> AppResult<()> {
    placeholder.map_or(Ok(()), |placeholder| {
        plain(placeholder, &format!("{path}.placeholder"), 150)
    })
}

fn check_option(option: &BlockOption, path: &str) -> AppResult<()> {
    plain(&option.text, &format!("{path}.text"), 75)?;
    length(&option.value, &format!("{path}.value"), 150, false)?;
    if let Some(url) = &option.url {
        web_address(url, &format!("{path}.url"))?;
    }
    Ok(())
}

fn check_options(options: &[BlockOption], path: &str, min: usize, max: usize) -> AppResult<()> {
    let at = format!("{path}.options");
    count(options.len(), &at, min, max, "options")?;
    for (i, option) in options.iter().enumerate() {
        check_option(option, &format!("{at}[{i}]"))?;
    }
    Ok(())
}

fn check_choices(options: &[BlockOption], path: &str) -> AppResult<()> {
    let at = format!("{path}.options");
    count(options.len(), &at, 1, MAX_CHOICES, "options")?;
    for (i, option) in options.iter().enumerate() {
        any_text(&option.text, &format!("{at}[{i}].text"), 75)?;
        length(&option.value, &format!("{at}[{i}].value"), 150, false)?;
    }
    Ok(())
}

fn one_of(chosen: &BlockOption, options: &[BlockOption], path: &str) -> AppResult<()> {
    if options.iter().any(|option| option.value == chosen.value) {
        Ok(())
    } else {
        Err(refuse(path, "must be one of the options"))
    }
}

fn check_element(element: &BlockElement, path: &str, images: &mut Vec<ImageRef>) -> AppResult<()> {
    let action_id = match element {
        BlockElement::Button {
            text,
            url,
            value,
            action_id,
            ..
        } => {
            plain(text, &format!("{path}.text"), 75)?;
            if let Some(url) = url {
                web_address(url, &format!("{path}.url"))?;
            }
            optional(value.as_ref(), &format!("{path}.value"), 2000)?;
            action_id
        }
        BlockElement::StaticSelect {
            placeholder,
            options,
            initial_option,
            action_id,
        } => {
            check_placeholder(placeholder.as_ref(), path)?;
            check_options(options, path, 1, MAX_SELECT_OPTIONS)?;
            if let Some(initial) = initial_option {
                check_option(initial, &format!("{path}.initial_option"))?;
            }
            action_id
        }
        BlockElement::UsersSelect {
            placeholder,
            action_id,
            ..
        }
        | BlockElement::ConversationsSelect {
            placeholder,
            action_id,
            ..
        } => {
            check_placeholder(placeholder.as_ref(), path)?;
            action_id
        }
        BlockElement::Overflow { options, action_id } => {
            check_options(options, path, 1, MAX_OVERFLOW_OPTIONS)?;
            action_id
        }
        BlockElement::Datepicker {
            placeholder,
            initial_date,
            action_id,
        } => {
            check_placeholder(placeholder.as_ref(), path)?;
            if let Some(date) = initial_date {
                if chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err() {
                    return Err(refuse(
                        &format!("{path}.initial_date"),
                        "must be a date as YYYY-MM-DD",
                    ));
                }
            }
            action_id
        }
        BlockElement::Image {
            image_url,
            alt_text,
        } => {
            image(image_url, &format!("{path}.image_url"), images)?;
            length(alt_text, &format!("{path}.alt_text"), 2000, true)?;
            return Ok(());
        }
        BlockElement::PlainTextInput {
            placeholder,
            initial_value,
            min_length,
            max_length,
            action_id,
            ..
        } => {
            check_placeholder(placeholder.as_ref(), path)?;
            let max = max_length.unwrap_or(MAX_INPUT);
            if max == 0 || max > MAX_INPUT {
                return Err(refuse(
                    &format!("{path}.max_length"),
                    format!("must be 1 to {MAX_INPUT}"),
                ));
            }
            if min_length.is_some_and(|min| min > max) {
                return Err(refuse(
                    &format!("{path}.min_length"),
                    "cannot be more than max_length",
                ));
            }
            optional(
                initial_value.as_ref(),
                &format!("{path}.initial_value"),
                max as usize,
            )?;
            action_id
        }
        BlockElement::Checkboxes {
            options,
            initial_options,
            action_id,
        } => {
            check_choices(options, path)?;
            for (i, chosen) in initial_options.iter().enumerate() {
                one_of(chosen, options, &format!("{path}.initial_options[{i}]"))?;
            }
            action_id
        }
        BlockElement::RadioButtons {
            options,
            initial_option,
            action_id,
        } => {
            check_choices(options, path)?;
            if let Some(chosen) = initial_option {
                one_of(chosen, options, &format!("{path}.initial_option"))?;
            }
            action_id
        }
    };
    optional(action_id.as_ref(), &format!("{path}.action_id"), MAX_ID)
}

#[derive(Deserialize)]
struct RawField {
    title: Option<Value>,
    value: Option<Value>,
    #[serde(default)]
    short: bool,
}

#[derive(Deserialize)]
struct RawAttachment {
    color: Option<String>,
    pretext: Option<String>,
    author_name: Option<String>,
    author_link: Option<String>,
    title: Option<String>,
    title_link: Option<String>,
    text: Option<String>,
    #[serde(default)]
    fields: Vec<RawField>,
    image_url: Option<String>,
    thumb_url: Option<String>,
    footer: Option<String>,
    ts: Option<Value>,
    blocks: Option<Value>,
}

fn words(value: Option<Value>) -> String {
    match value {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(text)) => text,
        Some(other) => other.to_string(),
    }
}

fn colour(color: Option<&str>) -> Option<String> {
    let color = color?.trim();
    let named = match color {
        "good" => Some("#2eb886"),
        "warning" => Some("#daa038"),
        "danger" => Some("#a30200"),
        _ => None,
    };
    if let Some(named) = named {
        return Some(named.to_string());
    }
    let hex = color.strip_prefix('#').unwrap_or(color);
    let valid = matches!(hex.len(), 3 | 6) && hex.chars().all(|c| c.is_ascii_hexdigit());
    valid.then(|| format!("#{}", hex.to_ascii_lowercase()))
}

fn seconds(ts: Option<Value>) -> Option<i64> {
    match ts? {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        Value::String(s) => s.split('.').next()?.parse().ok(),
        _ => None,
    }
}

fn attachments_at(value: &Value, images: &mut Vec<ImageRef>) -> AppResult<Vec<LegacyAttachment>> {
    let items = list(value, "attachments")?;
    if items.len() > MAX_ATTACHMENTS {
        return Err(refuse(
            "attachments",
            format!("at most {MAX_ATTACHMENTS} attachments"),
        ));
    }
    items
        .iter()
        .enumerate()
        .map(|(i, item)| attachment_at(item, &format!("attachments[{i}]"), images))
        .collect()
}

fn attachment_at(
    value: &Value,
    path: &str,
    images: &mut Vec<ImageRef>,
) -> AppResult<LegacyAttachment> {
    let raw: RawAttachment = typed(value, path)?;
    for (name, text, max) in [
        ("pretext", &raw.pretext, MAX_ATTACHMENT_LINE),
        ("author_name", &raw.author_name, MAX_ATTACHMENT_LINE),
        ("title", &raw.title, MAX_ATTACHMENT_LINE),
        ("text", &raw.text, MAX_ATTACHMENT_TEXT),
        ("footer", &raw.footer, MAX_ATTACHMENT_LINE),
    ] {
        optional(text.as_ref(), &format!("{path}.{name}"), max)?;
    }
    for (name, link) in [
        ("author_link", &raw.author_link),
        ("title_link", &raw.title_link),
    ] {
        if let Some(link) = link {
            web_address(link, &format!("{path}.{name}"))?;
        }
    }
    for (name, url) in [("image_url", &raw.image_url), ("thumb_url", &raw.thumb_url)] {
        if let Some(url) = url {
            image(url, &format!("{path}.{name}"), images)?;
        }
    }
    count(
        raw.fields.len(),
        &format!("{path}.fields"),
        0,
        MAX_ATTACHMENT_FIELDS,
        "fields",
    )?;
    let mut fields = Vec::with_capacity(raw.fields.len());
    for (i, field) in raw.fields.into_iter().enumerate() {
        let title = words(field.title);
        let value = words(field.value);
        length(&title, &format!("{path}.fields[{i}].title"), 2000, false)?;
        length(&value, &format!("{path}.fields[{i}].value"), 2000, false)?;
        fields.push(LegacyField {
            title,
            value,
            short: field.short,
        });
    }
    let blocks = match &raw.blocks {
        None | Some(Value::Null) => Vec::new(),
        Some(blocks) => blocks_at(blocks, &format!("{path}.blocks"), Surface::Message, images)?,
    };
    Ok(LegacyAttachment {
        color: colour(raw.color.as_deref()),
        pretext: raw.pretext,
        author_name: raw.author_name,
        author_link: raw.author_link,
        title: raw.title,
        title_link: raw.title_link,
        text: raw.text,
        fields,
        image_url: raw.image_url,
        thumb_url: raw.thumb_url,
        footer: raw.footer,
        ts: seconds(raw.ts),
        blocks,
    })
}
