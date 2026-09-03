use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use base64::Engine;
use hmac::{Hmac, KeyInit, Mac};
use redis::AsyncCommands;
use sha1::Sha1;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::{IceServer, IceServersResponse, InviteRequest, StartHuddleRequest};
use crate::authz;
use crate::dto::{DataList, StatusResponse};
use crate::huddle::models::{ActiveHuddle, HuddleStarted};
use crate::middleware::AuthUser;
use crate::state::AppState;

type HmacSha1 = Hmac<Sha1>;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(ice_servers))
        .routes(routes!(active_huddles))
        .routes(routes!(start_huddle))
        .routes(routes!(invite_to_huddle))
}

#[utoipa::path(get, path = "/workspaces/{ws_id}/active-huddles", tag = "huddles", responses((status = 200, body = DataList<ActiveHuddle>)))]
async fn active_huddles(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<DataList<ActiveHuddle>>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;

    let sessions = state.huddle_repo.list_open_channel_sessions(ws_id).await?;

    let mut conn = state.redis.clone();
    let mut active = Vec::new();
    for session in sessions {
        let key = format!("huddle:{}:members", session.id);
        let count: i64 = conn.scard(&key).await.unwrap_or(0);
        if count > 0 {
            active.push(ActiveHuddle {
                huddle_id: session.id,
                channel_id: session.channel_id,
                initiator_id: session.initiated_by,
            });
        }
    }

    Ok(Json(DataList { data: active }))
}

#[utoipa::path(post, path = "/workspaces/{ws_id}/huddles/{huddle_id}/invite", tag = "huddles", request_body = InviteRequest, responses((status = 200, body = StatusResponse)))]
async fn invite_to_huddle(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((ws_id, huddle_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<InviteRequest>,
) -> AppResult<Json<StatusResponse>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;

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
            .publish_scoped(
                "huddle.ring",
                ws_id,
                serde_json::json!({
                    "huddle_id": huddle_id,
                    "workspace_id": ws_id,
                    "from_user_id": auth.user_id,
                    "to_user_id": invitee,
                }),
            )
            .await;
    }

    Ok(Json(StatusResponse::new("ok")))
}

#[utoipa::path(post, path = "/workspaces/{ws_id}/huddles", tag = "huddles", request_body = StartHuddleRequest, responses((status = 200, body = HuddleStarted)))]
async fn start_huddle(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<StartHuddleRequest>,
) -> AppResult<Json<HuddleStarted>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;

    let huddle_id = Uuid::new_v4();

    match (req.channel_id, req.dm_partner_id) {
        (Some(channel_id), None) => {
            let channel = authz::require_channel_access(&state, channel_id, auth.user_id)
                .await?
                .channel;
            if channel.workspace_id != ws_id {
                return Err(AppError::BadRequest(
                    "Channel does not belong to workspace".into(),
                ));
            }
            state
                .huddle_repo
                .start_session(huddle_id, ws_id, Some(channel_id), None, auth.user_id)
                .await?;

            let mut tx = state.pool.begin().await?;
            let msg = state
                .message_repo
                .create_system_message_in(
                    &mut tx,
                    channel_id,
                    auth.user_id,
                    "started a huddle",
                    serde_json::json!({
                        "kind": "huddle_started",
                        "huddle_id": huddle_id,
                        "initiator_id": auth.user_id,
                    }),
                )
                .await?;
            let staged = state
                .publisher
                .stage_message_created(&mut tx, &msg, ws_id, &[])
                .await?;
            tx.commit().await?;
            state.publisher.dispatch(staged).await;

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
            authz::require_workspace_member(&state, ws_id, partner_id).await?;
            state
                .huddle_repo
                .start_session(huddle_id, ws_id, None, Some(partner_id), auth.user_id)
                .await?;
            let _ = state
                .publisher
                .publish_scoped(
                    "huddle.ring",
                    ws_id,
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

    Ok(Json(HuddleStarted { huddle_id }))
}

#[utoipa::path(get, path = "/workspaces/{ws_id}/ice-servers", tag = "huddles", responses((status = 200, body = IceServersResponse)))]
async fn ice_servers(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<IceServersResponse>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;

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
        .map_err(|e| AppError::Internal(format!("TURN HMAC init failed: {e}")))?;
    mac.update(username.as_bytes());
    let digest = mac.finalize().into_bytes();
    Ok(base64::engine::general_purpose::STANDARD.encode(digest))
}
