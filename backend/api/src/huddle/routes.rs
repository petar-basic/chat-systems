use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{middleware, Json, Router};
use base64::Engine;
use hmac::{Hmac, Mac};
use redis::AsyncCommands;
use sha1::Sha1;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::{IceServer, IceServersResponse, InviteRequest, StartHuddleRequest};
use crate::middleware::{auth_middleware, AuthUser};
use crate::state::AppState;
use crate::workspace::models::{Channel, ChannelType};

type HmacSha1 = Hmac<Sha1>;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/workspaces/:ws_id/ice-servers", get(ice_servers))
        .route("/workspaces/:ws_id/active-huddles", get(active_huddles))
        .route("/workspaces/:ws_id/huddles", post(start_huddle))
        .route(
            "/workspaces/:ws_id/huddles/:huddle_id/invite",
            post(invite_to_huddle),
        )
        .layer(middleware::from_fn(auth_middleware))
        .with_state(state)
}

async fn active_huddles(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    require_workspace_member(&state, ws_id, auth.user_id).await?;

    let sessions = state
        .huddle_repo
        .list_open_channel_sessions(ws_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    let mut conn = state.redis.clone();
    let mut active = Vec::new();
    for session in sessions {
        let key = format!("huddle:{}:members", session.id);
        let count: i64 = conn.scard(&key).await.unwrap_or(0);
        if count > 0 {
            active.push(serde_json::json!({
                "huddle_id": session.id,
                "channel_id": session.channel_id,
                "initiator_id": session.initiated_by,
            }));
        }
    }

    Ok(Json(serde_json::json!({ "data": active })))
}

async fn invite_to_huddle(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((ws_id, huddle_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<InviteRequest>,
) -> AppResult<Json<serde_json::Value>> {
    require_workspace_member(&state, ws_id, auth.user_id).await?;

    for invitee in req.user_ids {
        if invitee == auth.user_id {
            continue;
        }
        if !state
            .workspace_service
            .is_workspace_member(ws_id, invitee)
            .await?
        {
            continue;
        }
        let _ = state
            .publisher
            .publish(
                "huddle.ring",
                serde_json::json!({
                    "huddle_id": huddle_id,
                    "workspace_id": ws_id,
                    "from_user_id": auth.user_id,
                    "to_user_id": invitee,
                }),
            )
            .await;
    }

    Ok(Json(serde_json::json!({ "status": "ok" })))
}

async fn start_huddle(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<StartHuddleRequest>,
) -> AppResult<Json<serde_json::Value>> {
    require_workspace_member(&state, ws_id, auth.user_id).await?;

    let huddle_id = Uuid::new_v4();

    match (req.channel_id, req.dm_partner_id) {
        (Some(channel_id), None) => {
            let channel = require_channel_access(&state, channel_id, auth.user_id).await?;
            if channel.workspace_id != ws_id {
                return Err(AppError::BadRequest(
                    "Channel does not belong to workspace".into(),
                ));
            }
            state
                .huddle_repo
                .start_session(huddle_id, ws_id, Some(channel_id), None, auth.user_id)
                .await
                .map_err(|e| AppError::Database(e.to_string()))?;

            if let Ok(msg) = state
                .message_repo
                .create_system_message(
                    channel_id,
                    auth.user_id,
                    "started a huddle",
                    serde_json::json!({
                        "kind": "huddle_started",
                        "huddle_id": huddle_id,
                        "initiator_id": auth.user_id,
                    }),
                )
                .await
            {
                if let Ok(msg_json) = serde_json::to_value(&msg) {
                    let _ = state
                        .publisher
                        .publish_message_created(&msg_json, ws_id, &[])
                        .await;
                }
            }

            let _ = state
                .publisher
                .publish(
                    "huddle.started",
                    serde_json::json!({
                        "huddle_id": huddle_id,
                        "workspace_id": ws_id,
                        "channel_id": channel_id,
                        "initiator_id": auth.user_id,
                    }),
                )
                .await;
        }
        (None, Some(partner_id)) => {
            require_workspace_member(&state, ws_id, partner_id).await?;
            state
                .huddle_repo
                .start_session(huddle_id, ws_id, None, Some(partner_id), auth.user_id)
                .await
                .map_err(|e| AppError::Database(e.to_string()))?;
            let _ = state
                .publisher
                .publish(
                    "huddle.ring",
                    serde_json::json!({
                        "huddle_id": huddle_id,
                        "workspace_id": ws_id,
                        "from_user_id": auth.user_id,
                        "to_user_id": partner_id,
                    }),
                )
                .await;
        }
        _ => {
            return Err(AppError::BadRequest(
                "Provide exactly one of channel_id or dm_partner_id".into(),
            ))
        }
    }

    Ok(Json(serde_json::json!({ "huddle_id": huddle_id })))
}

async fn require_channel_access(
    state: &AppState,
    channel_id: Uuid,
    user_id: Uuid,
) -> AppResult<Channel> {
    let channel = state
        .workspace_service
        .repo
        .find_channel_by_id(channel_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("Channel not found".into()))?;

    state
        .workspace_service
        .repo
        .get_member(channel.workspace_id, user_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| AppError::Forbidden("Not a member of this workspace".into()))?;

    if channel.channel_type == ChannelType::Private || channel.channel_type == ChannelType::GroupDm
    {
        state
            .workspace_service
            .repo
            .get_channel_member(channel_id, user_id)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
            .ok_or_else(|| AppError::Forbidden("Not a member of this channel".into()))?;
    }

    Ok(channel)
}

async fn require_workspace_member(
    state: &AppState,
    workspace_id: Uuid,
    user_id: Uuid,
) -> AppResult<()> {
    let is_member = state
        .workspace_service
        .is_workspace_member(workspace_id, user_id)
        .await?;
    if !is_member {
        return Err(AppError::Forbidden("Not a workspace member".into()));
    }
    Ok(())
}

async fn ice_servers(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<IceServersResponse>> {
    require_workspace_member(&state, ws_id, auth.user_id).await?;

    let cfg = &state.config;
    let mut servers = Vec::new();

    let stun_urls = split_urls(&cfg.stun_urls);
    if !stun_urls.is_empty() {
        servers.push(IceServer {
            urls: stun_urls,
            username: None,
            credential: None,
        });
    }

    let turn_urls = split_urls(&cfg.turn_urls);
    if !turn_urls.is_empty() && !cfg.turn_secret.is_empty() {
        let expiry = chrono::Utc::now().timestamp() + cfg.turn_ttl_secs;
        let username = format!("{}:{}", expiry, auth.user_id);
        let credential = turn_credential(&cfg.turn_secret, &username)?;
        servers.push(IceServer {
            urls: turn_urls,
            username: Some(username),
            credential: Some(credential),
        });
    }

    Ok(Json(IceServersResponse {
        ice_servers: servers,
        ttl: cfg.turn_ttl_secs,
    }))
}

fn split_urls(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

fn turn_credential(secret: &str, username: &str) -> AppResult<String> {
    let mut mac = HmacSha1::new_from_slice(secret.as_bytes())
        .map_err(|e| AppError::Internal(format!("TURN HMAC init failed: {}", e)))?;
    mac.update(username.as_bytes());
    let digest = mac.finalize().into_bytes();
    Ok(base64::engine::general_purpose::STANDARD.encode(digest))
}
