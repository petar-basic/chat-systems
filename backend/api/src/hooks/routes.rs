use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use rand::RngCore;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::*;
use super::repo::NewReminder;
use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::authz;
use crate::middleware::AuthUser;
use crate::state::AppState;
use crate::workspace::models::{ChannelType, WorkspaceRole};

pub fn router(state: Arc<AppState>) -> Router {
    let protected = Router::new()
        .route(
            "/workspaces/:ws_id/hooks/channels",
            get(list_hooked_channels),
        )
        .route("/workspaces/:ws_id/hooks", get(list_hooks))
        .route("/workspaces/:ws_id/hooks", post(create_hook))
        .route("/hooks/:hook_id", get(get_hook))
        .route("/hooks/:hook_id", delete(delete_hook))
        .route("/hooks/:hook_id/reveal", post(reveal_hook))
        .route("/hooks/:hook_id/rotate", post(rotate_hook))
        .route("/workspaces/:ws_id/reminders", get(list_reminders))
        .route("/workspaces/:ws_id/reminders", post(create_reminder));

    // The incoming webhook is authenticated by its URL token, not a session, so
    // it must NOT sit behind auth_middleware.
    let public = Router::new().route("/hooks/incoming/:token", post(incoming_webhook));

    Router::new()
        .merge(crate::protected(state.clone(), protected))
        .merge(public.with_state(state))
}

fn generate_token() -> String {
    use base64::Engine;
    let mut bytes = [0u8; 24];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

async fn list_hooks(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
    let hooks = state.hook_repo.list_hooks(ws_id).await?;
    let mut data = serde_json::to_value(&hooks).map_err(|e| AppError::Internal(e.to_string()))?;
    if let Some(arr) = data.as_array_mut() {
        for hook in arr.iter_mut() {
            redact_secrets(hook.get_mut("config"));
        }
    }
    Ok(Json(serde_json::json!({ "data": data })))
}

async fn create_hook(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<CreateHookRequest>,
) -> AppResult<Json<Hook>> {
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
    shared_common::validation::validate_hook_name(&req.name)?;
    if let Some(description) = &req.description {
        shared_common::validation::validate_description(description)?;
    }
    let mut config = req.config.unwrap_or(serde_json::json!({}));

    if req.hook_type == HookType::IncomingWebhook {
        let channel_id = config
            .get("channel_id")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<Uuid>().ok())
            .ok_or_else(|| {
                AppError::Validation("incoming_webhook requires a channel_id in config".into())
            })?;
        require_attachable_channel(&state, ws_id, channel_id, auth.user_id).await?;
        if let Some(obj) = config.as_object_mut() {
            obj.insert("token".to_string(), serde_json::json!(generate_token()));
        }
    }

    if req.hook_type == HookType::OutgoingWebhook {
        let url = config
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                AppError::Validation("outgoing_webhook requires a url in config".into())
            })?
            .to_string();
        let parsed = reqwest::Url::parse(&url)
            .map_err(|e| AppError::Validation(format!("Invalid webhook URL: {e}")))?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(AppError::Validation(
                "Webhook URL must be http or https".into(),
            ));
        }

        let channel_ids = parse_channel_ids(&config)?;
        for channel_id in &channel_ids {
            require_attachable_channel(&state, ws_id, *channel_id, auth.user_id).await?;
        }

        if let Some(obj) = config.as_object_mut() {
            if !obj.contains_key("secret") {
                obj.insert("secret".to_string(), serde_json::json!(generate_token()));
            }
        }
    }

    let hook = state
        .hook_repo
        .create_hook(
            ws_id,
            auth.user_id,
            &req.hook_type,
            &req.name,
            req.description.as_deref(),
            &config,
        )
        .await?;

    audit::record(
        &state,
        AuditEntry::new(AuditAction::HookCreated, auth.user_id)
            .workspace(ws_id)
            .resource(hook.id)
            .ip(&ip)
            .details(serde_json::json!({
                "name": hook.name,
                "hook_type": hook.hook_type,
                "channel_ids": config.get("channel_ids"),
            })),
    )
    .await;

    Ok(Json(hook))
}

/// An outgoing webhook forwards a channel's traffic off the instance, so
/// attaching one to a channel takes the same rights as moderating it. Without
/// this a workspace admin who is not in a private channel can read it through a
/// webhook they point at themselves.
async fn require_attachable_channel(
    state: &AppState,
    ws_id: Uuid,
    channel_id: Uuid,
    user_id: Uuid,
) -> AppResult<()> {
    let channel = state
        .workspace_service
        .repo
        .find_channel_by_id(channel_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Channel not found".into()))?;
    if channel.workspace_id != ws_id {
        return Err(AppError::Validation(
            "channel does not belong to this workspace".into(),
        ));
    }
    if channel.channel_type != ChannelType::Public {
        state
            .workspace_service
            .repo
            .get_channel_member(channel_id, user_id)
            .await?
            .ok_or_else(|| {
                AppError::Forbidden(
                    "Only members of a private channel can attach an integration to it".into(),
                )
            })?;
        authz::require_channel_moderator(state, &channel, user_id).await?;
    }
    Ok(())
}

fn parse_channel_ids(config: &serde_json::Value) -> AppResult<Vec<Uuid>> {
    let raw = config
        .get("channel_ids")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            AppError::Validation(
                "outgoing_webhook requires a non-empty channel_ids array in config".into(),
            )
        })?;
    if raw.is_empty() {
        return Err(AppError::Validation(
            "outgoing_webhook requires a non-empty channel_ids array in config".into(),
        ));
    }
    raw.iter()
        .map(|v| {
            v.as_str()
                .and_then(|s| s.parse::<Uuid>().ok())
                .ok_or_else(|| {
                    AppError::Validation("channel_ids must be an array of channel UUIDs".into())
                })
        })
        .collect()
}

async fn list_hooked_channels(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    let channel_ids = state
        .hook_repo
        .channel_ids_with_outgoing_hooks(ws_id)
        .await?;
    Ok(Json(serde_json::json!({ "channel_ids": channel_ids })))
}

async fn get_hook(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(hook_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let hook = state
        .hook_repo
        .find_hook_by_id(hook_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Hook not found".into()))?;
    authz::require_workspace_role(
        &state,
        hook.workspace_id,
        auth.user_id,
        &WorkspaceRole::Admin,
    )
    .await?;
    let mut value = serde_json::to_value(&hook).map_err(|e| AppError::Internal(e.to_string()))?;
    redact_secrets(value.get_mut("config"));
    Ok(Json(value))
}

fn secrets_response(state: &AppState, hook: &Hook) -> serde_json::Value {
    let incoming_url = hook
        .config
        .get("token")
        .and_then(|v| v.as_str())
        .map(|token| format!("{}/api/hooks/incoming/{}", state.config.public_url, token));
    serde_json::json!({
        "hook_id": hook.id,
        "hook_type": hook.hook_type,
        "config": hook.config,
        "incoming_url": incoming_url,
    })
}

async fn require_hook_admin(state: &AppState, hook_id: Uuid, user_id: Uuid) -> AppResult<Hook> {
    let hook = state
        .hook_repo
        .find_hook_by_id(hook_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Hook not found".into()))?;
    authz::require_workspace_role(state, hook.workspace_id, user_id, &WorkspaceRole::Admin).await?;
    Ok(hook)
}

async fn reveal_hook(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(hook_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let hook = require_hook_admin(&state, hook_id, auth.user_id).await?;
    audit::record(
        &state,
        AuditEntry::new(AuditAction::HookRevealed, auth.user_id)
            .workspace(hook.workspace_id)
            .resource(hook.id)
            .ip(&ip),
    )
    .await;
    Ok(Json(secrets_response(&state, &hook)))
}

async fn rotate_hook(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(hook_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let hook = require_hook_admin(&state, hook_id, auth.user_id).await?;

    let rotated_key = match hook.hook_type {
        HookType::IncomingWebhook => "token",
        HookType::OutgoingWebhook => "secret",
        _ => {
            return Err(AppError::BadRequest(
                "Only incoming and outgoing webhooks carry a rotatable credential".into(),
            ))
        }
    };

    let mut config = hook.config.clone();
    let obj = config
        .as_object_mut()
        .ok_or_else(|| AppError::Internal("hook config is not an object".into()))?;
    obj.insert(rotated_key.to_string(), serde_json::json!(generate_token()));

    let updated = state.hook_repo.update_hook_config(hook.id, &config).await?;
    audit::record(
        &state,
        AuditEntry::new(AuditAction::HookRotated, auth.user_id)
            .workspace(hook.workspace_id)
            .resource(hook.id)
            .ip(&ip)
            .details(serde_json::json!({ "credential": rotated_key })),
    )
    .await;
    Ok(Json(secrets_response(&state, &updated)))
}

async fn delete_hook(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(hook_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let hook = state
        .hook_repo
        .find_hook_by_id(hook_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Hook not found".into()))?;
    authz::require_workspace_role(
        &state,
        hook.workspace_id,
        auth.user_id,
        &WorkspaceRole::Admin,
    )
    .await?;
    state.hook_repo.delete_hook(hook_id).await?;

    audit::record(
        &state,
        AuditEntry::new(AuditAction::HookDeleted, auth.user_id)
            .workspace(hook.workspace_id)
            .resource(hook.id)
            .ip(&ip)
            .details(serde_json::json!({
                "name": hook.name,
                "hook_type": hook.hook_type,
            })),
    )
    .await;

    Ok(Json(serde_json::json!({ "status": "deleted" })))
}

async fn list_reminders(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Member).await?;
    let reminders = state.hook_repo.list_reminders(ws_id, auth.user_id).await?;
    Ok(Json(serde_json::json!({ "data": reminders })))
}

async fn create_reminder(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<CreateReminderRequest>,
) -> AppResult<Json<Reminder>> {
    shared_common::validation::validate_reminder_content(&req.content)?;
    let member =
        authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Member).await?;
    if req.target_user_id != auth.user_id {
        if !member.role.has_at_least(&WorkspaceRole::Admin) {
            return Err(AppError::Forbidden(
                "Cannot create reminders for other users".into(),
            ));
        }
        state
            .workspace_service
            .repo
            .get_member(ws_id, req.target_user_id)
            .await?
            .ok_or_else(|| {
                AppError::Forbidden("Target user is not a member of this workspace".into())
            })?;
    }
    let reminder = state
        .hook_repo
        .create_reminder(NewReminder {
            workspace_id: ws_id,
            created_by: auth.user_id,
            target_user_id: req.target_user_id,
            channel_id: req.channel_id,
            message_id: req.message_id,
            content: &req.content,
            remind_at: req.remind_at,
        })
        .await?;
    Ok(Json(reminder))
}

async fn incoming_webhook(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
    peer: Option<axum::extract::ConnectInfo<std::net::SocketAddr>>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<IncomingWebhookPayload>,
) -> AppResult<Json<serde_json::Value>> {
    let mut conn = state.redis.clone();

    // Bound the source before the database is touched. Keying only on the token
    // means a caller who varies the token gets a fresh bucket every request, and
    // each one still costs a lookup.
    if let Some(ip) = crate::net::client_ip(
        &headers,
        peer.map(|p| p.0),
        &crate::net::parse_trusted_proxies(&state.config.trusted_proxies),
    ) {
        crate::rate_limit::enforce(
            &mut conn,
            &format!("rate_limit:hook_ip:{ip}"),
            120,
            60,
            crate::rate_limit::LimiterFailure::Open,
        )
        .await?;
    }

    crate::rate_limit::enforce(
        &mut conn,
        &format!("rate_limit:hook_incoming:{token}"),
        60,
        60,
        crate::rate_limit::LimiterFailure::Open,
    )
    .await?;

    let hook = state
        .hook_repo
        .find_active_incoming_hook_by_token(&token)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Invalid webhook token".into()))?;

    shared_common::validation::validate_message_content(&payload.text)?;

    let channel_id = hook
        .config
        .get("channel_id")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<Uuid>().ok())
        .ok_or_else(|| AppError::Internal("hook is missing a channel_id".into()))?;

    let msg = state
        .message_repo
        .create_message(channel_id, hook.created_by, &payload.text, None)
        .await?;

    let msg_json = serde_json::to_value(&msg).map_err(|e| AppError::Internal(e.to_string()))?;
    if let Err(e) = state
        .publisher
        .publish_message_created(&msg_json, hook.workspace_id, &[])
        .await
    {
        tracing::warn!(
            "incoming webhook publish failed for hook {}: {}",
            hook.id,
            e
        );
    }

    let _ = state
        .hook_repo
        .log_execution(
            hook.id,
            "incoming.message",
            &serde_json::json!({ "text": payload.text }),
            Some(200),
            None,
        )
        .await;

    Ok(Json(
        serde_json::json!({ "status": "ok", "message_id": msg.id }),
    ))
}

fn redact_secrets(value: Option<&mut serde_json::Value>) {
    if let Some(v) = value {
        redact_value(v);
    }
}

fn redact_value(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::Object(map) => {
            for (key, val) in map.iter_mut() {
                if is_secret_key(key) {
                    *val = serde_json::Value::String("***".to_string());
                } else {
                    redact_value(val);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr.iter_mut() {
                redact_value(item);
            }
        }
        _ => {}
    }
}

fn is_secret_key(key: &str) -> bool {
    let lk = key.to_lowercase();
    [
        "secret",
        "token",
        "password",
        "apikey",
        "api_key",
        "credential",
        "bearer",
        "authorization",
    ]
    .iter()
    .any(|needle| lk.contains(needle))
        || lk == "key"
}
