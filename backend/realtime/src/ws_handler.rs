use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::ws::{Message, WebSocket};
use futures_util::stream::{SplitSink, StreamExt};
use futures_util::SinkExt;
use tokio::sync::mpsc;
use tokio::time::interval;
use tracing::{info, warn};
use uuid::Uuid;

use crate::connection_manager::{ConnectionManager, WRITER_CHANNEL_CAP};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
const PONG_TIMEOUT: Duration = Duration::from_secs(90);

fn spawn_writer(
    mut sink: SplitSink<WebSocket, Message>,
    mut rx: mpsc::Receiver<Message>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let is_close = matches!(msg, Message::Close(_));
            if sink.send(msg).await.is_err() {
                break;
            }
            if is_close {
                break;
            }
        }
        let _ = sink.close().await;
    })
}

struct ConnGuard {
    conn_id: Uuid,
    cm: Arc<ConnectionManager>,
    cleaned: bool,
}

impl Drop for ConnGuard {
    fn drop(&mut self) {
        if !self.cleaned {
            self.cm.remove_connection(&self.conn_id);
            warn!("ConnGuard drop fallback cleanup for conn={}", self.conn_id);
        }
    }
}

pub async fn handle_ws(socket: WebSocket, user_id: Uuid, exp: i64, cm: Arc<ConnectionManager>) {
    let conn_id = Uuid::new_v4();
    let (sink, mut receiver) = socket.split();

    let (tx, rx) = mpsc::channel::<Message>(WRITER_CHANNEL_CAP);
    let writer = spawn_writer(sink, rx);

    let first_local = cm.add_connection(conn_id, user_id, tx.clone());

    let mut guard = ConnGuard {
        conn_id,
        cm: cm.clone(),
        cleaned: false,
    };

    info!("WS connected: user={}, conn={}", user_id, conn_id);

    cm.presence_set_online(user_id).await;
    if first_local {
        cm.publish_presence(user_id, "online").await;
    }

    let hello = serde_json::json!({
        "type": "hello",
        "user_id": user_id,
        "connection_id": conn_id,
    });
    let _ = tx.try_send(Message::Text(hello.to_string()));

    let mut heartbeat = interval(HEARTBEAT_INTERVAL);
    heartbeat.tick().await;

    let mut last_pong = Instant::now();
    let token_deadline = exp_to_deadline(exp);

    loop {
        tokio::select! {
            maybe_msg = receiver.next() => {
                match maybe_msg {
                    Some(Ok(Message::Text(text))) => {
                        last_pong = Instant::now();
                        handle_client_message(&text, &conn_id, user_id, &cm).await;
                    }
                    Some(Ok(Message::Pong(_))) => {
                        last_pong = Instant::now();
                    }
                    Some(Ok(Message::Ping(_))) => {
                        last_pong = Instant::now();
                    }
                    Some(Ok(Message::Close(_))) => break,
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                    None => break,
                }
            }

            _ = heartbeat.tick() => {
                if last_pong.elapsed() > PONG_TIMEOUT {
                    warn!(
                        "WS pong timeout, closing dead connection user={} conn={}",
                        user_id, conn_id
                    );
                    break;
                }
                if Instant::now() >= token_deadline {
                    info!(
                        "WS token expired, closing connection user={} conn={}",
                        user_id, conn_id
                    );
                    break;
                }
                if tx.try_send(Message::Ping(Vec::new())).is_err() {
                    warn!(
                        "WS writer channel closed/full on heartbeat, closing conn={}",
                        conn_id
                    );
                    break;
                }
                cm.presence_refresh(user_id).await;
                cm.huddle_redis_refresh_conn(&conn_id, user_id).await;
            }
        }
    }

    cleanup(&cm, &conn_id, user_id).await;
    guard.cleaned = true;
    drop(tx);
    writer.abort();

    info!("WS disconnected: user={}, conn={}", user_id, conn_id);
}

async fn cleanup(cm: &Arc<ConnectionManager>, conn_id: &Uuid, user_id: Uuid) {
    let huddles = cm.huddle_ids_for_conn(conn_id);
    let removed = cm.remove_connection(conn_id);

    for huddle_id in huddles {
        if !cm.user_in_huddle_local(user_id, huddle_id) {
            cm.huddle_redis_leave(huddle_id, user_id).await;
            cm.publish_huddle(
                "huddle.member_left",
                serde_json::json!({ "huddle_id": huddle_id, "user_id": user_id }),
            )
            .await;
        }
    }

    if let Some((uid, was_last)) = removed {
        if was_last {
            let fully_offline = cm.presence_clear(uid).await;
            if fully_offline {
                cm.publish_presence(uid, "offline").await;
            }
        }
    }
}

fn exp_to_deadline(exp: i64) -> Instant {
    let remaining = exp - chrono::Utc::now().timestamp();
    if remaining <= 0 {
        Instant::now()
    } else {
        Instant::now() + Duration::from_secs(remaining as u64)
    }
}

pub(crate) async fn handle_client_message(
    text: &str,
    conn_id: &Uuid,
    user_id: Uuid,
    cm: &Arc<ConnectionManager>,
) {
    let msg: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => {
            warn!("Invalid JSON from client: {}", text);
            return;
        }
    };

    let msg_type = msg.get("type").and_then(|v| v.as_str()).unwrap_or("");

    match msg_type {
        "subscribe" => {
            if let Some(ws_id) = msg.get("workspace_id").and_then(|v| v.as_str()) {
                if let Ok(ws_id) = ws_id.parse::<Uuid>() {
                    if !cm.is_workspace_member(ws_id, user_id).await {
                        warn!(
                            "Denied subscribe: user {} is not a member of workspace {}",
                            user_id, ws_id
                        );
                        return;
                    }
                    cm.subscribe_workspace(conn_id, ws_id);
                    info!("User {} subscribed to workspace {}", user_id, ws_id);

                    let online = cm.online_users_in_workspace(ws_id).await;
                    let batch = serde_json::json!({
                        "type": "presence.batch",
                        "users": online.iter().map(|u| {
                            serde_json::json!({ "user_id": u, "status": "online" })
                        }).collect::<Vec<_>>(),
                    });
                    cm.send_to_user(user_id, &batch.to_string()).await;

                    cm.publish_presence(user_id, "online").await;
                }
            }
        }
        "channel.join" => {
            if let Some(ch_id) = msg.get("channel_id").and_then(|v| v.as_str()) {
                if let Ok(ch_id) = ch_id.parse::<Uuid>() {
                    if !cm.is_channel_member(ch_id, user_id).await {
                        warn!(
                            "Denied channel.join: user {} is not a member of channel {}",
                            user_id, ch_id
                        );
                        return;
                    }
                    cm.join_channel(conn_id, ch_id);
                    info!("User {} joined channel {}", user_id, ch_id);
                }
            }
        }
        "channel.leave" => {
            if let Some(ch_id) = msg.get("channel_id").and_then(|v| v.as_str()) {
                if let Ok(ch_id) = ch_id.parse::<Uuid>() {
                    cm.leave_channel(conn_id, ch_id);
                    info!("User {} left channel {}", user_id, ch_id);
                }
            }
        }
        "typing.start" => {
            if let Some(ch_id) = msg.get("channel_id").and_then(|v| v.as_str()) {
                if let Ok(ch_id) = ch_id.parse::<Uuid>() {
                    if !cm.is_channel_member(ch_id, user_id).await {
                        warn!(
                            "Denied typing.start: user {} is not a member of channel {}",
                            user_id, ch_id
                        );
                        return;
                    }
                    cm.publish_typing(ch_id, user_id, true).await;
                }
            }
        }
        "typing.stop" => {
            if let Some(ch_id) = msg.get("channel_id").and_then(|v| v.as_str()) {
                if let Ok(ch_id) = ch_id.parse::<Uuid>() {
                    if !cm.is_channel_member(ch_id, user_id).await {
                        warn!(
                            "Denied typing.stop: user {} is not a member of channel {}",
                            user_id, ch_id
                        );
                        return;
                    }
                    cm.publish_typing(ch_id, user_id, false).await;
                }
            }
        }
        "huddle.join" => {
            let Some(huddle_id) = msg_uuid(&msg, "huddle_id") else {
                return;
            };
            let allowed = if let Some(channel_id) = msg_uuid(&msg, "channel_id") {
                cm.is_channel_member(channel_id, user_id).await
            } else if let (Some(ws_id), Some(partner_id)) = (
                msg_uuid(&msg, "workspace_id"),
                msg_uuid(&msg, "dm_partner_id"),
            ) {
                cm.is_workspace_member(ws_id, user_id).await
                    && cm.is_workspace_member(ws_id, partner_id).await
            } else {
                false
            };
            if !allowed {
                warn!(
                    "Denied huddle.join: user {} not authorized for huddle {}",
                    user_id, huddle_id
                );
                return;
            }
            cm.join_huddle(conn_id, huddle_id);
            cm.huddle_redis_join(huddle_id, user_id).await;
            cm.publish_huddle(
                "huddle.member_joined",
                serde_json::json!({ "huddle_id": huddle_id, "user_id": user_id }),
            )
            .await;
            let members = cm.huddle_redis_members(huddle_id).await;
            let snapshot = serde_json::json!({
                "type": "huddle.members",
                "huddle_id": huddle_id,
                "user_ids": members,
            });
            cm.send_to_user(user_id, &snapshot.to_string()).await;
            info!("User {} joined huddle {}", user_id, huddle_id);
        }
        "huddle.leave" => {
            let Some(huddle_id) = msg_uuid(&msg, "huddle_id") else {
                return;
            };
            cm.leave_huddle(conn_id, huddle_id);
            if !cm.user_in_huddle_local(user_id, huddle_id) {
                cm.huddle_redis_leave(huddle_id, user_id).await;
                cm.publish_huddle(
                    "huddle.member_left",
                    serde_json::json!({ "huddle_id": huddle_id, "user_id": user_id }),
                )
                .await;
            }
            info!("User {} left huddle {}", user_id, huddle_id);
        }
        "huddle.offer" | "huddle.answer" | "huddle.ice" => {
            let (Some(huddle_id), Some(to_user_id)) =
                (msg_uuid(&msg, "huddle_id"), msg_uuid(&msg, "to_user_id"))
            else {
                return;
            };
            if !cm.huddle_redis_is_member(huddle_id, user_id).await
                || !cm.huddle_redis_is_member(huddle_id, to_user_id).await
            {
                warn!(
                    "Denied huddle signaling: {} -> {} not both in huddle {}",
                    user_id, to_user_id, huddle_id
                );
                return;
            }
            let mut payload = serde_json::json!({
                "huddle_id": huddle_id,
                "from_user_id": user_id,
                "to_user_id": to_user_id,
            });
            if let Some(sdp) = msg.get("sdp") {
                payload["sdp"] = sdp.clone();
            }
            if let Some(candidate) = msg.get("candidate") {
                payload["candidate"] = candidate.clone();
            }
            cm.publish_huddle(msg_type, payload).await;
        }
        "huddle.mute" => {
            let Some(huddle_id) = msg_uuid(&msg, "huddle_id") else {
                return;
            };
            if !cm.huddle_redis_is_member(huddle_id, user_id).await {
                return;
            }
            let audio_muted = msg
                .get("audio_muted")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            cm.publish_huddle(
                "huddle.mute",
                serde_json::json!({
                    "huddle_id": huddle_id,
                    "user_id": user_id,
                    "audio_muted": audio_muted,
                }),
            )
            .await;
        }
        "huddle.camera" => {
            let Some(huddle_id) = msg_uuid(&msg, "huddle_id") else {
                return;
            };
            if !cm.huddle_redis_is_member(huddle_id, user_id).await {
                return;
            }
            let camera_on = msg
                .get("camera_on")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            cm.publish_huddle(
                "huddle.camera",
                serde_json::json!({
                    "huddle_id": huddle_id,
                    "user_id": user_id,
                    "camera_on": camera_on,
                }),
            )
            .await;
        }
        "huddle.screenshare" => {
            let Some(huddle_id) = msg_uuid(&msg, "huddle_id") else {
                return;
            };
            if !cm.huddle_redis_is_member(huddle_id, user_id).await {
                return;
            }
            let sharing = msg
                .get("sharing")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            cm.publish_huddle(
                "huddle.screenshare",
                serde_json::json!({
                    "huddle_id": huddle_id,
                    "user_id": user_id,
                    "sharing": sharing,
                }),
            )
            .await;
        }
        "huddle.reaction" => {
            let Some(huddle_id) = msg_uuid(&msg, "huddle_id") else {
                return;
            };
            let Some(emoji) = msg.get("emoji").and_then(|v| v.as_str()) else {
                return;
            };
            if emoji.chars().count() > 8 || !cm.huddle_redis_is_member(huddle_id, user_id).await {
                return;
            }
            cm.publish_huddle(
                "huddle.reaction",
                serde_json::json!({
                    "huddle_id": huddle_id,
                    "user_id": user_id,
                    "emoji": emoji,
                }),
            )
            .await;
        }
        "huddle.hand" => {
            let Some(huddle_id) = msg_uuid(&msg, "huddle_id") else {
                return;
            };
            if !cm.huddle_redis_is_member(huddle_id, user_id).await {
                return;
            }
            let raised = msg.get("raised").and_then(|v| v.as_bool()).unwrap_or(false);
            cm.publish_huddle(
                "huddle.hand",
                serde_json::json!({
                    "huddle_id": huddle_id,
                    "user_id": user_id,
                    "raised": raised,
                }),
            )
            .await;
        }
        "ping" => {
            cm.send_to_user(user_id, &serde_json::json!({"type":"pong"}).to_string())
                .await;
        }
        _ => {
            warn!("Unknown client message type: {}", msg_type);
        }
    }
}

fn msg_uuid(msg: &serde_json::Value, key: &str) -> Option<Uuid> {
    msg.get(key)
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<Uuid>().ok())
}
