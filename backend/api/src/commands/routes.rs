use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use garde::Validate;
use serde::{Deserialize, Serialize};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::builtin;
use super::delivery::{self, Outcome};
use super::repo::NewInvocation;
use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::authz;
use crate::dto::{DataList, StatusResponse};
use crate::middleware::AuthUser;
use crate::state::AppState;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(invoke))
        .routes(routes!(list_commands))
}

pub fn public_router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new().routes(routes!(respond))
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct CommandInfo {
    pub command: String,
    pub hint: Option<String>,
    pub builtin: bool,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct CommandResponse {
    /// `ephemeral` is seen only by whoever ran it; `in_channel` is posted as a
    /// message from the command; `pending` means the answer arrives later as a
    /// `command.response` frame.
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

    pub fn pending(text: impl Into<String>) -> Self {
        Self {
            response_type: "pending",
            text: text.into(),
            at: None,
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

#[derive(Debug, Deserialize, garde::Validate, utoipa::ToSchema)]
pub struct CommandAnswer {
    #[garde(custom(shared_common::validation::rules::message_content))]
    pub text: String,
    #[garde(skip)]
    #[serde(default)]
    pub response_type: Option<String>,
    #[garde(skip)]
    #[serde(default)]
    pub replace_original: bool,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct InvokeRequest {
    pub command: String,
    #[serde(default)]
    pub text: String,
}

#[utoipa::path(get, path = "/workspaces/{ws_id}/commands", tag = "commands", responses((status = 200, body = DataList<CommandInfo>)))]
async fn list_commands(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<DataList<CommandInfo>>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;

    let mut commands: Vec<CommandInfo> = builtin::BUILTIN_COMMANDS
        .iter()
        .map(|(name, hint)| CommandInfo {
            command: name.to_string(),
            hint: Some(hint.to_string()),
            builtin: true,
        })
        .collect();

    for hook in state.hook_repo.list_slash_commands(ws_id).await? {
        if let Some(command) = hook.config.get("command").and_then(|v| v.as_str()) {
            commands.push(CommandInfo {
                command: command.to_string(),
                hint: hook.description.clone(),
                builtin: false,
            });
        }
    }

    Ok(Json(commands.into()))
}

/// Built-ins first, then anything registered. An unknown command is a 404 by
/// design: the client falls back to sending what was typed as an ordinary
/// message, so a typo does not vanish into an error dialog.
#[utoipa::path(post, path = "/channels/{ch_id}/commands", tag = "commands", request_body = InvokeRequest, responses((status = 200, body = CommandResponse)))]
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
        return finish(
            &state,
            ch_id,
            auth.user_id,
            &command,
            response,
            may_post,
            None,
        )
        .await;
    }

    let channel = authz::find_channel(&state, ch_id).await?;

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

    let request_id = Uuid::new_v4();
    let payload = serde_json::json!({
        "command": command,
        "text": req.text,
        "channel_id": ch_id,
        "workspace_id": channel.workspace_id,
        "user_id": auth.user_id,
    });
    state
        .command_repo
        .create(NewInvocation {
            id: request_id,
            hook_id: hook.id,
            workspace_id: channel.workspace_id,
            channel_id: ch_id,
            user_id: auth.user_id,
            command: &command,
            payload: &payload,
            uses: delivery::RESPONSE_URL_USES,
            ttl_secs: delivery::RESPONSE_URL_TTL_SECS,
        })
        .await?;

    let outgoing = delivery::outgoing(&state, request_id, &payload);
    let over_socket = match hook.app_id {
        Some(app_id) => {
            crate::app_events::deliver_command(
                &state, app_id, request_id, ch_id, &command, &outgoing,
            )
            .await?
        }
        None => None,
    };
    let response = match over_socket {
        Some(response) => response,
        None => {
            let outcome = delivery::call(
                &hook,
                &outgoing,
                request_id,
                delivery::FIRST_ATTEMPT_TIMEOUT,
                state.config.webhook_allow_private_targets,
            )
            .await?;
            match outcome {
                Outcome::Refused(why) => return Err(AppError::BadRequest(why)),
                Outcome::Unreachable => {
                    state
                        .command_repo
                        .schedule_retry(request_id, 1, delivery::first_retry_delay())
                        .await?;
                    CommandResponse::pending(format!("/{command} is taking a moment…"))
                }
                Outcome::Acknowledged => {
                    CommandResponse::pending(format!("/{command} is working on it…"))
                }
                Outcome::Answered(response) => response,
            }
        }
    };

    audit::record(
        &state,
        AuditEntry::new(AuditAction::CommandInvoked, auth.user_id)
            .workspace(channel.workspace_id)
            .resource(hook.id)
            .ip(&ip)
            .details(serde_json::json!({ "command": command })),
    )
    .await;

    finish(
        &state,
        ch_id,
        auth.user_id,
        &command,
        response,
        may_post,
        Some(request_id),
    )
    .await
}

#[utoipa::path(post, path = "/commands/responses/{token}", tag = "commands", request_body = CommandAnswer, responses((status = 200, body = StatusResponse)), security(()))]
async fn respond(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
    Json(req): Json<CommandAnswer>,
) -> AppResult<Json<StatusResponse>> {
    req.validate()?;
    let id = delivery::invocation_of(&state, &token)
        .ok_or_else(|| AppError::NotFound("Unknown response URL".into()))?;
    let Some(invocation) = state.command_repo.use_response_url(id).await? else {
        return Err(AppError::NotFound(
            if state.command_repo.exists(id).await? {
                "This response URL has expired or been used up"
            } else {
                "Unknown response URL"
            }
            .into(),
        ));
    };
    state
        .hook_repo
        .find_hook_by_id(invocation.hook_id)
        .await?
        .filter(|hook| hook.is_active)
        .ok_or_else(|| AppError::Forbidden("The command has been turned off".into()))?;

    let response = match req.response_type.as_deref() {
        Some("in_channel") => CommandResponse::in_channel(req.text),
        _ => CommandResponse::ephemeral(req.text),
    };
    delivery::answer(&state, &invocation, &response, req.replace_original).await?;
    Ok(Json(StatusResponse::new("ok")))
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
    invocation: Option<Uuid>,
) -> AppResult<Json<CommandResponse>> {
    if response.response_type != "in_channel" {
        return Ok(Json(response));
    }
    // An answer nobody may post becomes an answer only the person who asked
    // sees, rather than an error for a command that already did its work.
    if !may_post {
        return Ok(Json(CommandResponse::ephemeral(response.text)));
    }
    if !response.text.trim().is_empty() {
        let message_id =
            delivery::post_in_channel(state, ch_id, user_id, command, &response.text, None).await?;
        if let Some(invocation) = invocation {
            state
                .command_repo
                .record_answer(invocation, message_id)
                .await?;
        }
    }
    Ok(Json(response))
}
