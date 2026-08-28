use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::builtin;
use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::authz;
use crate::hooks::{executor, ssrf};
use crate::middleware::AuthUser;
use crate::state::AppState;

/// Somebody is watching a spinner while this runs. Slack allows three seconds
/// before it gives up on the same interaction, and a command that needs longer
/// should be posting back through an incoming webhook instead.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(3);

pub fn router(state: Arc<AppState>) -> Router {
    let routes = Router::new()
        .route("/channels/{ch_id}/commands", post(invoke))
        .route("/workspaces/{ws_id}/commands", get(list_commands));

    crate::protected(state, routes)
}

#[derive(Debug, Serialize)]
pub struct CommandResponse {
    /// `ephemeral` is seen only by whoever ran it; `in_channel` is posted as a
    /// message from the command.
    pub response_type: &'static str,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<chrono::DateTime<chrono::Utc>>,
}

impl CommandResponse {
    pub fn ephemeral(text: impl Into<String>) -> Self {
        Self {
            response_type: "ephemeral",
            text: text.into(),
            at: None,
        }
    }

    pub fn ephemeral_at(text: impl Into<String>, at: chrono::DateTime<chrono::Utc>) -> Self {
        Self {
            response_type: "ephemeral",
            text: text.into(),
            at: Some(at),
        }
    }

    pub fn in_channel(text: impl Into<String>) -> Self {
        Self {
            response_type: "in_channel",
            text: text.into(),
            at: None,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct InvokeRequest {
    pub command: String,
    #[serde(default)]
    pub text: String,
}

async fn list_commands(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;

    let mut commands: Vec<serde_json::Value> = builtin::BUILTIN_COMMANDS
        .iter()
        .map(|(name, hint)| serde_json::json!({ "command": name, "hint": hint, "builtin": true }))
        .collect();

    for hook in state.hook_repo.list_slash_commands(ws_id).await? {
        if let Some(command) = hook.config.get("command").and_then(|v| v.as_str()) {
            commands.push(serde_json::json!({
                "command": command,
                "hint": hook.description,
                "builtin": false,
            }));
        }
    }

    Ok(Json(serde_json::json!({ "data": commands })))
}

/// Built-ins first, then anything registered. An unknown command is a 404 by
/// design: the client falls back to sending what was typed as an ordinary
/// message, so a typo does not vanish into an error dialog.
async fn invoke(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(ch_id): Path<Uuid>,
    Json(req): Json<InvokeRequest>,
) -> AppResult<Json<CommandResponse>> {
    // Running a command is reading; answering `in_channel` is posting. The
    // strict check happens up front so an announcement channel cannot be
    // written to through `/topic` or a registered command either.
    let access = authz::require_channel_access(&state, ch_id, auth.user_id).await?;
    let may_post = access.can_post();

    let command = req.command.trim().trim_start_matches('/').to_lowercase();
    if command.is_empty() {
        return Err(AppError::NotFound("unknown_command".into()));
    }

    if let Some(response) = builtin::run(&state, ch_id, auth.user_id, &command, &req.text).await? {
        return finish(&state, ch_id, auth.user_id, &command, response, may_post).await;
    }

    let channel = state
        .workspace_service
        .repo
        .find_channel_by_id(ch_id)
        .await?
        .ok_or_else(|| AppError::NotFound("No such channel".into()))?;

    let hook = state
        .hook_repo
        .find_slash_command(channel.workspace_id, &command)
        .await?
        .ok_or_else(|| AppError::NotFound("unknown_command".into()))?;

    // Registered in some channels, not all: the scoping from CS-019 applies
    // here for the same reason it applies there -- invoking it sends what
    // somebody typed to a third-party URL.
    let scoped_to: Vec<String> = hook
        .config
        .get("channel_ids")
        .and_then(|v| v.as_array())
        .map(|ids| {
            ids.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    if !scoped_to.is_empty() && !scoped_to.contains(&ch_id.to_string()) {
        return Err(AppError::Forbidden(format!(
            "/{command} is not enabled in this channel"
        )));
    }

    let payload = serde_json::json!({
        "command": command,
        "text": req.text,
        "channel_id": ch_id,
        "workspace_id": channel.workspace_id,
        "user_id": auth.user_id,
    });

    let response = dispatch(&hook, &payload, state.config.webhook_allow_private_targets).await?;

    audit::record(
        &state,
        AuditEntry::new(AuditAction::CommandInvoked, auth.user_id)
            .workspace(channel.workspace_id)
            .resource(hook.id)
            .ip(&ip)
            .details(serde_json::json!({ "command": command })),
    )
    .await;

    finish(&state, ch_id, auth.user_id, &command, response, may_post).await
}

/// An `in_channel` answer becomes a real message from the command; an ephemeral
/// one is handed straight back and stored nowhere.
async fn finish(
    state: &AppState,
    ch_id: Uuid,
    user_id: Uuid,
    command: &str,
    response: CommandResponse,
    may_post: bool,
) -> AppResult<Json<CommandResponse>> {
    // An answer nobody may post becomes an answer only the person who asked
    // sees, rather than an error for a command that already did its work.
    if response.response_type == "in_channel" && !may_post {
        return Ok(Json(CommandResponse::ephemeral(response.text)));
    }

    if response.response_type == "in_channel" && !response.text.trim().is_empty() {
        let bot = serde_json::json!({ "name": format!("/{command}"), "icon_url": null });
        let msg = state
            .message_repo
            .create_bot_message(ch_id, user_id, &response.text, &bot)
            .await?;

        if let Some(channel) = state
            .workspace_service
            .repo
            .find_channel_by_id(ch_id)
            .await?
        {
            let msg_json =
                serde_json::to_value(&msg).map_err(|e| AppError::Internal(e.to_string()))?;
            let _ = state
                .publisher
                .publish_message_created(&msg_json, channel.workspace_id, &[])
                .await;
        }
    }

    Ok(Json(response))
}

async fn dispatch(
    hook: &crate::hooks::models::Hook,
    payload: &serde_json::Value,
    allow_private: bool,
) -> AppResult<CommandResponse> {
    let url_str = hook
        .config
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::Internal("command hook is missing a url".into()))?;

    // The same validation the outgoing hooks use. A command URL is operator
    // input, and an operator who can point one at 169.254.169.254 has a
    // credential exfiltration primitive.
    let url = ssrf::validate_outbound_url_with(url_str, allow_private)
        .await
        .map_err(|e| AppError::BadRequest(format!("Command URL is not reachable: {e}")))?;

    let secret = hook
        .config
        .get("secret")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let body = serde_json::to_vec(payload).map_err(|e| AppError::Internal(e.to_string()))?;

    let response = reqwest::Client::new()
        .post(url)
        .header("Content-Type", "application/json")
        .header(
            executor::SIGNATURE_HEADER,
            executor::sign_body(secret, &body),
        )
        .timeout(COMMAND_TIMEOUT)
        .body(body)
        .send()
        .await
        .map_err(|_| {
            AppError::ServiceUnavailable(format!("/{} did not answer in time", hook.name))
        })?;

    // No retries: unlike an event, a command has somebody waiting for it, and a
    // second attempt would run whatever it does twice.
    if !response.status().is_success() {
        return Err(AppError::BadRequest(format!(
            "/{} answered {}",
            hook.name,
            response.status()
        )));
    }

    let text = response.text().await.unwrap_or_default();
    let parsed: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);

    let answer = parsed
        .get("text")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| text.chars().take(1000).collect());

    Ok(match parsed.get("response_type").and_then(|v| v.as_str()) {
        Some("in_channel") => CommandResponse::in_channel(answer),
        _ => CommandResponse::ephemeral(answer),
    })
}
