use std::fmt::Display;

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;

use shared_common::errors::{AppError, AppResult};

use super::models::{
    Block, BlockElement, BlockOption, BlockText, ContextElement, Layout, LegacyAttachment,
    LegacyField, TextKind,
};

pub const MAX_BLOCKS: usize = 50;
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

const BLOCK_TYPES: [&str; 6] = [
    "header", "section", "divider", "image", "context", "actions",
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
        Some(value) => blocks_at(value, "blocks", &mut images)?,
    };
    let legacy_attachments = match attachments {
        None | Some(Value::Null) => Vec::new(),
        Some(value) => attachments_at(value, &mut images)?,
    };
    Ok((
        Layout {
            blocks,
            legacy_attachments,
        },
        images,
    ))
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

fn blocks_at(value: &Value, path: &str, images: &mut Vec<ImageRef>) -> AppResult<Vec<Block>> {
    let items = list(value, path)?;
    if items.len() > MAX_BLOCKS {
        return Err(refuse(path, format!("at most {MAX_BLOCKS} blocks")));
    }
    items
        .iter()
        .enumerate()
        .map(|(i, item)| block_at(item, &format!("{path}[{i}]"), images))
        .collect()
}

fn block_at(value: &Value, path: &str, images: &mut Vec<ImageRef>) -> AppResult<Block> {
    match type_at(value, path, &BLOCK_TYPES, "block")? {
        "section" => {
            if let Some(accessory) = value.get("accessory") {
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
        Some(blocks) => blocks_at(blocks, &format!("{path}.blocks"), images)?,
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
