use std::sync::Arc;
use std::time::Duration;

use tracing::{info, warn};
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::repo::Invocation;
use super::routes::CommandResponse;
use crate::authz;
use crate::blocks::{self, models::Composed};
use crate::hooks::models::Hook;
use crate::hooks::{executor, ssrf};
use crate::messaging::routes::thread_root;
use crate::sealed;
use crate::state::AppState;
use crate::threads::repo::ReplyFollows;
use crate::threads::routes::announce_reply;

pub const RESPONSE_URL_TTL_SECS: f64 = 30.0 * 60.0;
pub const RESPONSE_URL_USES: i32 = 5;
pub const REQUEST_ID_HEADER: &str = "X-Chat-Request-Id";
pub const FIRST_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(3);
const RETRY_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_ATTEMPTS: i32 = 3;
const RETRY_DELAYS_SECS: [f64; 2] = [2.0, 8.0];
const RETRY_LEASE_SECS: f64 = 60.0;
const TICK: Duration = Duration::from_secs(1);
const PURGE_EVERY_TICKS: u64 = 300;

#[derive(Debug, Default, serde::Deserialize)]
pub struct Reply {
    pub text: Option<String>,
    pub response_type: Option<String>,
    pub blocks: Option<serde_json::Value>,
    pub attachments: Option<serde_json::Value>,
}

impl Reply {
    pub async fn compose(self, state: &AppState, workspace_id: Uuid) -> AppResult<CommandResponse> {
        let composed = blocks::compose(
            state,
            workspace_id,
            blocks::Input {
                text: self.text.as_deref(),
                blocks: self.blocks.as_ref(),
                attachments: self.attachments.as_ref(),
            },
        )
        .await?;
        Ok(CommandResponse::answer(
            self.response_type.as_deref(),
            composed,
        ))
    }
}

pub enum Outcome {
    Answered(Reply),
    Acknowledged,
    Unreachable,
    Refused(String),
}

pub fn first_retry_delay() -> f64 {
    RETRY_DELAYS_SECS[0]
}

pub fn response_url(state: &AppState, id: Uuid) -> String {
    shared_common::paths::api_url(
        state.config.public_url.trim_end_matches('/'),
        &format!(
            "/commands/responses/{}",
            sealed::sign_id(&state.config.secrets_keys, sealed::COMMAND_RESPONSES, id)
        ),
    )
}

pub fn invocation_of(state: &AppState, token: &str) -> Option<Uuid> {
    sealed::verify_id(&state.config.secrets_keys, sealed::COMMAND_RESPONSES, token)
}

pub fn outgoing(state: &AppState, id: Uuid, payload: &serde_json::Value) -> serde_json::Value {
    let mut outgoing = payload.clone();
    if let Some(fields) = outgoing.as_object_mut() {
        fields.insert("request_id".into(), serde_json::json!(id));
        fields.insert(
            "response_url".into(),
            serde_json::json!(response_url(state, id)),
        );
    }
    outgoing
}

pub async fn call(
    hook: &Hook,
    payload: &serde_json::Value,
    request_id: Uuid,
    timeout: Duration,
    allow_private: bool,
) -> AppResult<Outcome> {
    let url_str = hook
        .config
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest(format!("/{} has no URL to call", hook.name)))?;

    let url = ssrf::validate_outbound_url_with(url_str, allow_private)
        .await
        .map_err(|e| AppError::BadRequest(format!("Command URL is not reachable: {e}")))?;

    let secret = hook
        .config
        .get("secret")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let body = serde_json::to_vec(payload).map_err(|e| AppError::Internal(e.to_string()))?;

    let Ok(response) = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| AppError::Internal(e.to_string()))?
        .post(url)
        .header("Content-Type", "application/json")
        .header(
            executor::SIGNATURE_HEADER,
            executor::sign_body(secret, &body),
        )
        .header(REQUEST_ID_HEADER, request_id.to_string())
        .timeout(timeout)
        .body(body)
        .send()
        .await
    else {
        return Ok(Outcome::Unreachable);
    };

    let status = response.status();
    if matches!(status.as_u16(), 502..=504) {
        return Ok(Outcome::Unreachable);
    }
    if !status.is_success() {
        return Ok(Outcome::Refused(format!(
            "/{} answered {status}",
            hook.name
        )));
    }

    let text = response.text().await.unwrap_or_default();
    if text.trim().is_empty() {
        return Ok(Outcome::Acknowledged);
    }
    let mut reply: Reply = serde_json::from_str(&text).unwrap_or_default();
    if reply.text.is_none() && reply.blocks.is_none() && reply.attachments.is_none() {
        reply.text = Some(text.chars().take(1000).collect());
    }
    Ok(Outcome::Answered(reply))
}

pub struct Answering<'a> {
    pub channel_id: Uuid,
    pub thread_parent_id: Option<Uuid>,
    pub user_id: Uuid,
    pub command: &'a str,
    pub hook_id: Option<Uuid>,
}

pub async fn post_in_channel(
    state: &AppState,
    answering: Answering<'_>,
    answer: &Composed,
    replacing: Option<Uuid>,
) -> AppResult<Uuid> {
    let Answering {
        channel_id,
        thread_parent_id,
        user_id,
        command,
        hook_id,
    } = answering;
    let channel = authz::find_channel(state, channel_id).await?;
    let root = match (thread_parent_id, replacing) {
        (Some(parent), None) => Some(thread_root(state, channel_id, parent).await?),
        _ => None,
    };
    let mut tx = state.pool.begin().await?;
    let (message_id, staged) = match replacing {
        Some(message_id) => {
            state
                .message_repo
                .update_message_in(&mut tx, message_id, &answer.content, user_id)
                .await?;
            let msg = state
                .message_repo
                .merge_layout_in(&mut tx, message_id, &answer.metadata().into())
                .await?;
            let staged = state
                .publisher
                .stage_message_updated(&mut tx, &msg, channel.workspace_id)
                .await?;
            (msg.id, staged)
        }
        None => {
            let metadata = answer.merged_into(serde_json::json!({ "bot": {
                "name": format!("/{command}"),
                "icon_url": null,
                "hook_id": hook_id,
            } }));
            let msg = state
                .message_repo
                .create_bot_reply_in(
                    &mut tx,
                    channel_id,
                    root.as_ref().map(|root| root.id),
                    user_id,
                    &answer.content,
                    &metadata,
                    &[],
                )
                .await?;
            if let Some(root) = &root {
                state
                    .thread_repo
                    .follow_on_reply_in(
                        &mut tx,
                        ReplyFollows {
                            root_message_id: root.id,
                            workspace_id: channel.workspace_id,
                            replier_id: user_id,
                            root_author_id: root.user_id,
                            mentioned: &[],
                            replied_at: msg.created_at,
                        },
                    )
                    .await?;
            }
            let staged = state
                .publisher
                .stage_message_created(&mut tx, &msg, channel.workspace_id, &[])
                .await?;
            (msg.id, staged)
        }
    };
    tx.commit().await?;
    state.publisher.dispatch(staged).await;
    if let Some(root) = &root {
        announce_reply(state, root.id, channel.workspace_id).await;
    }
    Ok(message_id)
}

pub async fn tell(state: &AppState, invocation: &Invocation, response_type: &str, text: &str) {
    let payload = serde_json::json!({
        "user_id": invocation.user_id,
        "channel_id": invocation.channel_id,
        "thread_parent_id": invocation.thread_parent_id,
        "command": invocation.command,
        "response_type": response_type,
        "text": text,
    });
    if let Err(e) = state.publisher.publish("command.response", payload).await {
        warn!(invocation = %invocation.id, "failed to send a command's answer: {e}");
    }
}

pub async fn answer(
    state: &AppState,
    invocation: &Invocation,
    response: &CommandResponse,
    replace_original: bool,
) -> AppResult<()> {
    let access =
        authz::require_channel_access(state, invocation.channel_id, invocation.user_id).await?;
    let posts = response.response_type == "in_channel"
        && authz::posting_blocked(state, &access).await?.is_none()
        && (!response.text.trim().is_empty() || response.layout.is_some());
    if !posts {
        tell(state, invocation, "ephemeral", &response.text).await;
        return Ok(());
    }
    let replacing = invocation.answer_message_id.filter(|_| replace_original);
    let message_id = post_in_channel(
        state,
        Answering {
            channel_id: invocation.channel_id,
            thread_parent_id: invocation.thread_parent_id,
            user_id: invocation.user_id,
            command: &invocation.command,
            hook_id: Some(invocation.hook_id),
        },
        &response.posted(),
        replacing,
    )
    .await?;
    if replacing.is_none() {
        state
            .command_repo
            .record_answer(invocation.id, message_id)
            .await?;
    }
    tell(state, invocation, "in_channel", "").await;
    Ok(())
}

pub async fn retry_due(state: &AppState) {
    let due = match state.command_repo.claim_due_retries(RETRY_LEASE_SECS).await {
        Ok(due) => due,
        Err(e) => {
            warn!("Failed to claim command retries: {e}");
            return;
        }
    };
    for invocation in due {
        retry(state, &invocation).await;
    }
}

async fn retry(state: &AppState, invocation: &Invocation) {
    let attempt = invocation.attempts + 1;
    let hook = match state.hook_repo.find_hook_by_id(invocation.hook_id).await {
        Ok(Some(hook)) if hook.is_active => hook,
        Ok(_) => {
            settle(state, invocation, attempt).await;
            let gone = format!("/{} is no longer available", invocation.command);
            tell(state, invocation, "error", &gone).await;
            return;
        }
        Err(e) => {
            warn!(invocation = %invocation.id, "Failed to load a command for its retry: {e}");
            return;
        }
    };

    let outcome = call(
        &hook,
        &outgoing(state, invocation.id, &invocation.payload),
        invocation.id,
        RETRY_TIMEOUT,
        state.config.webhook_allow_private_targets,
    )
    .await;
    match outcome {
        Ok(Outcome::Unreachable) if attempt < MAX_ATTEMPTS => {
            let delay = RETRY_DELAYS_SECS[(attempt - 1) as usize];
            if let Err(e) = state
                .command_repo
                .schedule_retry(invocation.id, attempt, delay)
                .await
            {
                warn!(invocation = %invocation.id, "Failed to schedule a command retry: {e}");
            }
        }
        Ok(Outcome::Unreachable) => {
            settle(state, invocation, attempt).await;
            let unreachable = format!("/{} could not be reached", invocation.command);
            tell(state, invocation, "error", &unreachable).await;
        }
        Ok(Outcome::Acknowledged) => settle(state, invocation, attempt).await,
        Ok(Outcome::Answered(reply)) => {
            settle(state, invocation, attempt).await;
            let delivered = match reply.compose(state, invocation.workspace_id).await {
                Ok(response) => answer(state, invocation, &response, false).await,
                Err(e) => {
                    tell(state, invocation, "error", &e.to_string()).await;
                    return;
                }
            };
            if let Err(e) = delivered {
                warn!(invocation = %invocation.id, "Failed to deliver a command's answer: {e}");
            }
        }
        Ok(Outcome::Refused(why)) => {
            settle(state, invocation, attempt).await;
            tell(state, invocation, "error", &why).await;
        }
        Err(e) => {
            settle(state, invocation, attempt).await;
            tell(state, invocation, "error", &e.to_string()).await;
        }
    }
}

async fn settle(state: &AppState, invocation: &Invocation, attempts: i32) {
    if let Err(e) = state.command_repo.settle(invocation.id, attempts).await {
        warn!(invocation = %invocation.id, "Failed to settle a command: {e}");
    }
}

pub async fn start_retrier(state: Arc<AppState>) {
    info!("Command retrier started");
    let mut ticks: u64 = 0;
    loop {
        tokio::time::sleep(TICK).await;
        retry_due(&state).await;
        ticks += 1;
        if ticks.is_multiple_of(PURGE_EVERY_TICKS) {
            if let Err(e) = state.command_repo.purge_expired().await {
                warn!("Failed to purge expired command invocations: {e}");
            }
        }
    }
}
