pub mod models;
pub mod repo;
pub mod routes;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::time::Duration;

use serde_json::{json, Map, Value};
use tracing::warn;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use crate::app_events::sender;
use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::authz;
use crate::blocks::models::{Block, BlockElement, BlockOption, Layout, View};
use crate::blocks::parse;
use crate::bots::models::BotApp;
use crate::hooks::{executor, ssrf};
use crate::messaging::models::Message;
use crate::sealed;
use crate::state::AppState;
use models::{AppView, StoredView, Values};
use repo::Delivered;

pub const ANSWER_WITHIN: Duration = Duration::from_secs(3);
pub const TRIGGER_TTL_SECS: f64 = 3.0;
pub const RESPONSE_URL_USES: i32 = 5;
pub const RESPONSE_URL_TTL_SECS: f64 = 30.0 * 60.0;
pub const NOT_INTERACTIVE: &str = "This app has not set up interactivity";
pub const NO_RESPONSE: &str = "The app did not respond";
const POLL: Duration = Duration::from_millis(50);
const REQUIRED: &str = "This field is required";

pub enum Answer {
    Acked(Option<Value>),
    Silent,
}

pub fn is_interactive(app: &BotApp) -> bool {
    app.interactivity && app.disabled_at.is_none() && (app.socket_mode || app.request_url.is_some())
}

pub async fn interactive_app(state: &AppState, app_id: Uuid) -> AppResult<BotApp> {
    state
        .bot_repo
        .find(app_id)
        .await?
        .filter(is_interactive)
        .ok_or_else(|| AppError::Conflict(NOT_INTERACTIVE.into()))
}

pub fn response_url(state: &AppState, id: Uuid) -> String {
    shared_common::paths::api_url(
        state.config.public_url.trim_end_matches('/'),
        &format!(
            "/interactions/responses/{}",
            sealed::sign_id(
                &state.config.secrets_keys,
                sealed::INTERACTION_RESPONSES,
                id
            )
        ),
    )
}

pub fn interaction_of(state: &AppState, token: &str) -> Option<Uuid> {
    sealed::verify_id(
        &state.config.secrets_keys,
        sealed::INTERACTION_RESPONSES,
        token,
    )
}

pub async fn exchange(state: &AppState, app: &BotApp, payload: &Value) -> AppResult<Answer> {
    if app.socket_mode {
        over_socket(state, app, payload).await
    } else {
        over_http(state, app, payload).await
    }
}

async fn over_socket(state: &AppState, app: &BotApp, payload: &Value) -> AppResult<Answer> {
    if app.connected_sockets == 0 {
        return Ok(Answer::Silent);
    }
    let id = state
        .interaction_repo
        .enqueue_for_socket(app.id, payload, ANSWER_WITHIN.as_secs_f64())
        .await?;
    let deadline = tokio::time::Instant::now() + ANSWER_WITHIN;
    let mut answer = Answer::Silent;
    while tokio::time::Instant::now() < deadline {
        tokio::time::sleep(POLL).await;
        match state.interaction_repo.delivered(id).await? {
            Some(Delivered {
                done: true,
                response,
            }) => {
                answer = Answer::Acked(response);
                break;
            }
            Some(_) => {}
            None => break,
        }
    }
    state.interaction_repo.forget_delivery(id).await?;
    Ok(answer)
}

async fn over_http(state: &AppState, app: &BotApp, payload: &Value) -> AppResult<Answer> {
    let Some(url) = app.request_url.as_deref() else {
        return Ok(Answer::Silent);
    };
    let Ok(target) =
        ssrf::validate_outbound_url_with(url, state.config.webhook_allow_private_targets).await
    else {
        return Ok(Answer::Silent);
    };
    let secret = sender::signing_secret(state, app.id).await?;
    let body = serde_json::to_vec(payload).map_err(|e| AppError::Internal(e.to_string()))?;
    let sent = sender::client()?
        .post(target)
        .header("Content-Type", "application/json")
        .header(
            executor::SIGNATURE_HEADER,
            executor::sign_body(&secret, &body),
        )
        .timeout(ANSWER_WITHIN)
        .body(body)
        .send()
        .await;
    let Ok(response) = sent else {
        return Ok(Answer::Silent);
    };
    if !response.status().is_success() {
        return Ok(Answer::Silent);
    }
    let text = response.text().await.unwrap_or_default();
    Ok(Answer::Acked(serde_json::from_str(&text).ok()))
}

pub async fn record(state: &AppState, ip: &ClientIp, user_id: Uuid, app: &BotApp, details: Value) {
    audit::record(
        state,
        AuditEntry::new(AuditAction::BotAppInteraction, user_id)
            .workspace(app.workspace_id)
            .resource(app.id)
            .ip(ip)
            .details(details),
    )
    .await;
}

pub fn answered(answer: Answer) -> AppResult<Option<Value>> {
    match answer {
        Answer::Acked(body) => Ok(body),
        Answer::Silent => Err(AppError::ServiceUnavailable(NO_RESPONSE.into())),
    }
}

pub fn message_body(message: &Message, layout: &Layout) -> Value {
    json!({
        "id": message.id,
        "user_id": message.user_id,
        "text": message.content,
        "blocks": layout.blocks,
        "attachments": layout.legacy_attachments,
        "thread_parent_id": message.thread_parent_id,
        "created_at": message.created_at,
    })
}

pub fn person(user_id: Uuid, name: Option<String>) -> Value {
    json!({ "id": user_id, "name": name })
}

fn actionable(block: &Block) -> Vec<&BlockElement> {
    match block {
        Block::Input { .. } => Vec::new(),
        other => parse::elements(other),
    }
}

pub fn find_element<'a>(
    blocks: impl IntoIterator<Item = &'a Block>,
    block_id: &str,
    action_id: &str,
) -> Option<&'a BlockElement> {
    blocks
        .into_iter()
        .filter(|block| parse::block_id(block).is_some_and(|id| id == block_id))
        .flat_map(actionable)
        .find(|element| parse::action_id(element).is_some_and(|id| id == action_id))
}

pub fn message_blocks(layout: &Layout) -> Vec<&Block> {
    layout
        .blocks
        .iter()
        .chain(
            layout
                .legacy_attachments
                .iter()
                .flat_map(|attachment| attachment.blocks.iter()),
        )
        .collect()
}

fn option_of<'a>(options: &'a [BlockOption], value: Option<&str>) -> AppResult<&'a BlockOption> {
    options
        .iter()
        .find(|option| Some(option.value.as_str()) == value)
        .ok_or_else(|| AppError::Validation("Pick one of the options".into()))
}

async fn member(state: &AppState, workspace_id: Uuid, value: Option<&str>) -> AppResult<Uuid> {
    let refused = || AppError::Validation("Pick a person in this workspace".into());
    let id: Uuid = value.and_then(|v| v.parse().ok()).ok_or_else(refused)?;
    authz::require_workspace_member(state, workspace_id, id)
        .await
        .map_err(|_| refused())?;
    Ok(id)
}

async fn conversation(state: &AppState, user_id: Uuid, value: Option<&str>) -> AppResult<Uuid> {
    let refused = || AppError::Validation("Pick a conversation you can see".into());
    let id: Uuid = value.and_then(|v| v.parse().ok()).ok_or_else(refused)?;
    authz::require_channel_access(state, id, user_id)
        .await
        .map_err(|_| refused())?;
    Ok(id)
}

fn date(value: Option<&str>) -> AppResult<String> {
    value
        .filter(|v| chrono::NaiveDate::parse_from_str(v, "%Y-%m-%d").is_ok())
        .map(str::to_string)
        .ok_or_else(|| AppError::Validation("Pick a date as YYYY-MM-DD".into()))
}

pub async fn action(
    state: &AppState,
    workspace_id: Uuid,
    user_id: Uuid,
    block_id: &str,
    element: &BlockElement,
    value: Option<&str>,
) -> AppResult<Value> {
    let (kind, chosen) = match element {
        BlockElement::Button {
            text,
            url: None,
            style,
            value,
            ..
        } => (
            "button",
            json!({ "text": text, "value": value, "style": style }),
        ),
        BlockElement::StaticSelect { options, .. } => (
            "static_select",
            json!({ "selected_option": option_of(options, value)? }),
        ),
        BlockElement::Overflow { options, .. } => (
            "overflow",
            json!({ "selected_option": option_of(options, value)? }),
        ),
        BlockElement::UsersSelect { .. } => (
            "users_select",
            json!({ "selected_user": member(state, workspace_id, value).await? }),
        ),
        BlockElement::ConversationsSelect { .. } => (
            "conversations_select",
            json!({ "selected_conversation": conversation(state, user_id, value).await? }),
        ),
        BlockElement::Datepicker { .. } => ("datepicker", json!({ "selected_date": date(value)? })),
        _ => {
            return Err(AppError::Validation(
                "This element sends nothing to the app".into(),
            ))
        }
    };
    let mut action = json!({
        "type": kind,
        "block_id": block_id,
        "action_id": parse::action_id(element),
        "action_ts": chrono::Utc::now(),
    });
    if let (Some(action), Value::Object(chosen)) = (action.as_object_mut(), chosen) {
        action.extend(chosen);
    }
    Ok(action)
}

fn given<'a>(values: &'a Values, block_id: &str, action_id: &str) -> Vec<&'a str> {
    values
        .get(block_id)
        .and_then(|actions| actions.get(action_id))
        .map(|items| {
            items
                .iter()
                .map(String::as_str)
                .filter(|item| !item.trim().is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn chosen_options(options: &[BlockOption], picked: &[&str]) -> Vec<BlockOption> {
    options
        .iter()
        .filter(|option| picked.contains(&option.value.as_str()))
        .cloned()
        .collect()
}

async fn input_value(
    state: &AppState,
    workspace_id: Uuid,
    user_id: Uuid,
    element: &BlockElement,
    picked: &[&str],
) -> Result<Option<Value>, String> {
    let first = picked.first().copied();
    let entry = match element {
        BlockElement::PlainTextInput {
            min_length,
            max_length,
            ..
        } => {
            let first = first.filter(|text| !text.trim().is_empty());
            if let Some(text) = first {
                let length = text.chars().count();
                let length = u32::try_from(length).unwrap_or(u32::MAX);
                if let Some(max) = max_length.filter(|max| length > *max) {
                    return Err(format!("At most {max} characters"));
                }
                if let Some(min) = min_length.filter(|min| length < *min) {
                    return Err(format!("At least {min} characters"));
                }
            }
            first.map(|text| json!({ "type": "plain_text_input", "value": text }))
        }
        BlockElement::Checkboxes { options, .. } => {
            let selected = chosen_options(options, picked);
            (!selected.is_empty())
                .then(|| json!({ "type": "checkboxes", "selected_options": selected }))
        }
        BlockElement::RadioButtons { options, .. } => {
            chosen_options(options, &picked[..picked.len().min(1)])
                .into_iter()
                .next()
                .map(|selected| json!({ "type": "radio_buttons", "selected_option": selected }))
        }
        BlockElement::StaticSelect { options, .. } => {
            chosen_options(options, &picked[..picked.len().min(1)])
                .into_iter()
                .next()
                .map(|selected| json!({ "type": "static_select", "selected_option": selected }))
        }
        BlockElement::UsersSelect { .. } => match first {
            Some(_) => {
                let id = member(state, workspace_id, first)
                    .await
                    .map_err(|e| e.to_string())?;
                Some(json!({ "type": "users_select", "selected_user": id }))
            }
            None => None,
        },
        BlockElement::ConversationsSelect { .. } => match first {
            Some(_) => {
                let id = conversation(state, user_id, first)
                    .await
                    .map_err(|e| e.to_string())?;
                Some(json!({ "type": "conversations_select", "selected_conversation": id }))
            }
            None => None,
        },
        BlockElement::Datepicker { .. } => match first {
            Some(_) => Some(
                json!({ "type": "datepicker", "selected_date": date(first).map_err(|e| e.to_string())? }),
            ),
            None => None,
        },
        _ => None,
    };
    Ok(entry)
}

pub async fn state_values(
    state: &AppState,
    view: &View,
    workspace_id: Uuid,
    user_id: Uuid,
    values: &Values,
) -> (Value, HashMap<String, String>) {
    let mut out = Map::new();
    let mut errors = HashMap::new();
    for block in &view.blocks {
        let Block::Input {
            element,
            optional,
            block_id: Some(block_id),
            ..
        } = block
        else {
            continue;
        };
        let Some(action_id) = parse::action_id(element) else {
            continue;
        };
        let picked = given(values, block_id, action_id);
        let entry = match input_value(state, workspace_id, user_id, element, &picked).await {
            Ok(entry) => entry,
            Err(why) => {
                errors.insert(block_id.clone(), why);
                continue;
            }
        };
        if entry.is_none() && !optional {
            errors.insert(block_id.clone(), REQUIRED.to_string());
        }
        let entry = entry.unwrap_or_else(|| json!({ "type": element_kind(element) }));
        out.insert(block_id.clone(), json!({ action_id: entry }));
    }
    (json!({ "values": out }), errors)
}

fn element_kind(element: &BlockElement) -> &'static str {
    match element {
        BlockElement::Button { .. } => "button",
        BlockElement::StaticSelect { .. } => "static_select",
        BlockElement::UsersSelect { .. } => "users_select",
        BlockElement::ConversationsSelect { .. } => "conversations_select",
        BlockElement::Overflow { .. } => "overflow",
        BlockElement::Datepicker { .. } => "datepicker",
        BlockElement::Image { .. } => "image",
        BlockElement::PlainTextInput { .. } => "plain_text_input",
        BlockElement::Checkboxes { .. } => "checkboxes",
        BlockElement::RadioButtons { .. } => "radio_buttons",
    }
}

pub fn new_hash() -> String {
    Uuid::new_v4().simple().to_string()
}

pub fn read_view(stored: &StoredView) -> AppResult<View> {
    serde_json::from_value(stored.view.clone()).map_err(|e| AppError::Internal(e.to_string()))
}

pub fn app_views(stack: &[StoredView]) -> AppResult<Vec<AppView>> {
    let Some(root) = stack.first() else {
        return Ok(Vec::new());
    };
    stack
        .iter()
        .enumerate()
        .map(|(i, stored)| {
            Ok(AppView {
                id: stored.id,
                app_id: stored.app_id,
                root_view_id: root.id,
                previous_view_id: i.checked_sub(1).map(|j| stack[j].id),
                hash: stored.hash.clone(),
                view: read_view(stored)?,
            })
        })
        .collect()
}

pub async fn app_view(state: &AppState, stored: &StoredView) -> AppResult<AppView> {
    let stack = state.interaction_repo.stack(stored.stack_id).await?;
    app_views(&stack)?
        .into_iter()
        .find(|view| view.id == stored.id)
        .ok_or_else(|| AppError::NotFound("View not found".into()))
}

pub fn view_payload(view: &AppView, view_state: Value) -> Value {
    let mut payload = serde_json::to_value(view).unwrap_or_else(|_| json!({}));
    if let Some(fields) = payload.as_object_mut() {
        fields.insert("state".into(), view_state);
    }
    payload
}

pub async fn announce(state: &AppState, user_id: Uuid, workspace_id: Uuid, stack_id: Uuid) {
    let views = match state.interaction_repo.stack(stack_id).await {
        Ok(stack) => app_views(&stack).unwrap_or_default(),
        Err(e) => {
            warn!(%stack_id, "failed to read a view stack: {e}");
            return;
        }
    };
    let payload = json!({
        "user_id": user_id,
        "workspace_id": workspace_id,
        "views": views,
    });
    if let Err(e) = state.publisher.publish("views.changed", payload).await {
        warn!(%user_id, "failed to send a view stack: {e}");
    }
}
