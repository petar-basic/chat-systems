use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::ws::{CloseFrame, Message, WebSocket};
use futures_util::stream::{SplitSink, StreamExt};
use futures_util::SinkExt;
use tokio::sync::mpsc;
use tokio::time::interval;
use tracing::{info, warn};

use crate::connection_manager::Audience;
use uuid::Uuid;

use crate::connection_manager::{ConnectionManager, WRITER_CHANNEL_CAP};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
const PONG_TIMEOUT: Duration = Duration::from_secs(90);

/// Inbound frames are cheap for the client and not for us: every subscribe,
/// join and typing frame costs a database round trip. Bound them per socket.
const INBOUND_PER_SEC: f32 = 20.0;
const INBOUND_BURST: f32 = 40.0;
const TYPING_COALESCE: Duration = Duration::from_secs(3);
const INBOUND_FLOOD_CLOSE_CODE: u16 = 4002;

pub(crate) struct InboundState {
    tokens: f32,
    last_refill: Instant,
    last_typing: std::collections::HashMap<Uuid, Instant>,
}

impl InboundState {
    pub(crate) fn new() -> Self {
        Self {
            tokens: INBOUND_BURST,
            last_refill: Instant::now(),
            last_typing: std::collections::HashMap::new(),
        }
    }

    fn allow(&mut self) -> bool {
        let now = Instant::now();
        self.tokens = (self.tokens
            + now.duration_since(self.last_refill).as_secs_f32() * INBOUND_PER_SEC)
            .min(INBOUND_BURST);
        self.last_refill = now;
        if self.tokens < 1.0 {
            return false;
        }
        self.tokens -= 1.0;
        true
    }

    /// A client that keeps typing re-sends `typing.start` every few keystrokes.
    /// Republishing each one costs a membership lookup and a fan-out for no
    /// change in what anyone sees.
    fn should_publish_typing(&mut self, channel_id: Uuid) -> bool {
        let now = Instant::now();
        match self.last_typing.get(&channel_id) {
            Some(last) if now.duration_since(*last) < TYPING_COALESCE => false,
            _ => {
                self.last_typing.insert(channel_id, now);
                true
            }
        }
    }

    fn forget_typing(&mut self, channel_id: Uuid) {
        self.last_typing.remove(&channel_id);
    }
}

impl Default for InboundState {
    fn default() -> Self {
        Self::new()
    }
}

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

pub async fn handle_ws(socket: WebSocket, claims: crate::Claims, cm: Arc<ConnectionManager>) {
    let user_id = claims.sub;
    let exp = claims.exp;
    let conn_id = Uuid::new_v4();
    let (sink, mut receiver) = socket.split();

    let (tx, rx) = mpsc::channel::<Message>(WRITER_CHANNEL_CAP);
    let writer = spawn_writer(sink, rx);

    let first_local = cm.add_connection(conn_id, user_id, claims.jti, tx.clone());

    let mut guard = ConnGuard {
        conn_id,
        cm: cm.clone(),
        cleaned: false,
    };

    info!("WS connected: user={}, conn={}", user_id, conn_id);

    // Resolved once per connection instead of on every heartbeat: the set of
    // workspaces a user belongs to changes far more slowly than the 30-second
    // refresh, and `workspace.member_removed` corrects it when it does.
    let presence_workspaces = cm.user_workspace_ids(user_id).await;

    cm.presence_set_online(user_id, &presence_workspaces).await;
    if first_local {
        cm.publish_presence(user_id, "online").await;
    }

    let hello = serde_json::json!({
        "type": "hello",
        "user_id": user_id,
        "connection_id": conn_id,
    });
    let _ = tx.try_send(Message::Text(hello.to_string().into()));

    let mut heartbeat = interval(HEARTBEAT_INTERVAL);
    heartbeat.tick().await;

    let mut last_pong = Instant::now();
    let mut inbound = InboundState::new();
    let token_deadline = exp_to_deadline(exp);

    loop {
        tokio::select! {
            maybe_msg = receiver.next() => {
                match maybe_msg {
                    Some(Ok(Message::Text(text))) => {
                        last_pong = Instant::now();
                        if !inbound.allow() {
                            metrics::counter!("realtime_inbound_dropped_total").increment(1);
                            warn!(
                                "WS inbound flood, closing connection user={} conn={}",
                                user_id, conn_id
                            );
                            let _ = tx.try_send(Message::Close(Some(CloseFrame {
                                code: INBOUND_FLOOD_CLOSE_CODE,
                                reason: "too many messages".into(),
                            })));
                            break;
                        }
                        handle_client_message(text.as_str(), &conn_id, user_id, &cm, &mut inbound).await;
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
                if tx.try_send(Message::Ping(Default::default())).is_err() {
                    warn!(
                        "WS writer channel closed/full on heartbeat, closing conn={}",
                        conn_id
                    );
                    break;
                }
                cm.presence_refresh(user_id, &presence_workspaces).await;
                cm.huddle_redis_refresh_conn(&conn_id, user_id).await;
            }
        }
    }

    cleanup(&cm, &conn_id, user_id, &presence_workspaces).await;
    guard.cleaned = true;
    drop(tx);
    writer.abort();

    info!("WS disconnected: user={}, conn={}", user_id, conn_id);
}

async fn cleanup(
    cm: &Arc<ConnectionManager>,
    conn_id: &Uuid,
    user_id: Uuid,
    presence_workspaces: &[Uuid],
) {
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
            let fully_offline = cm.presence_clear(uid, presence_workspaces).await;
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
    inbound: &mut InboundState,
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

                    // The client sends back the last position it processed, so
                    // a socket that dropped for thirty seconds gets the gap
                    // rather than a refetch of whatever happens to be open.
                    let resume = msg.get("last_event_id").and_then(|v| v.as_str());
                    let sync = match resume {
                        Some(last_id) if !last_id.is_empty() => {
                            crate::replay::replay_workspace(cm, *conn_id, ws_id, last_id).await
                        }
                        _ => crate::replay::Replay::Caught {
                            events: 0,
                            last_id: crate::replay::current_tail(cm, ws_id).await,
                        },
                    };

                    let frame = match &sync {
                        crate::replay::Replay::RefetchRequired => serde_json::json!({
                            "type": "sync.refetch_required",
                            "workspace_id": ws_id,
                        }),
                        crate::replay::Replay::Caught { events, last_id } => serde_json::json!({
                            "type": "sync.complete",
                            "workspace_id": ws_id,
                            "replayed": events,
                            "last_event_id": last_id,
                        }),
                    };
                    cm.send_to_conn(*conn_id, &frame.to_string());

                    let online = cm.online_users_in_workspace(ws_id).await;
                    let batch = serde_json::json!({
                        "type": "presence.batch",
                        "users": online.iter().map(|u| {
                            serde_json::json!({ "user_id": u, "status": "online" })
                        }).collect::<Vec<_>>(),
                    });
                    cm.send_to_user(Audience::Everyone, user_id, &batch.to_string())
                        .await;

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
                    inbound.forget_typing(ch_id);
                    cm.leave_channel(conn_id, ch_id);
                    info!("User {} left channel {}", user_id, ch_id);
                }
            }
        }
        "typing.start" => {
            if let Some(ch_id) = msg.get("channel_id").and_then(|v| v.as_str()) {
                if let Ok(ch_id) = ch_id.parse::<Uuid>() {
                    if !inbound.should_publish_typing(ch_id) {
                        return;
                    }
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
                    inbound.forget_typing(ch_id);
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
            cm.send_to_user(Audience::Everyone, user_id, &snapshot.to_string())
                .await;
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
                .and_then(serde_json::Value::as_bool)
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
                .and_then(serde_json::Value::as_bool)
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
                .and_then(serde_json::Value::as_bool)
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
            let raised = msg
                .get("raised")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
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
            cm.send_to_user(
                Audience::Everyone,
                user_id,
                &serde_json::json!({"type":"pong"}).to_string(),
            )
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
