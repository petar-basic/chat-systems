use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use tracing::{info, warn};

use shared_events::frames::ServerFrame;

use crate::connection_manager::Audience;

use crate::connection_manager::ConnectionManager;

pub async fn start_event_consumer(
    redis_url: &str,
    cm: Arc<ConnectionManager>,
    heartbeat: Arc<AtomicI64>,
) {
    let client = match redis::Client::open(redis_url) {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to connect to Redis for event consumer: {}", e);
            return;
        }
    };

    let mut pubsub = match client.get_async_pubsub().await {
        Ok(ps) => ps,
        Err(e) => {
            warn!("Failed to get pubsub connection: {}", e);
            return;
        }
    };

    let channels = [
        "events:message",
        "events:reaction",
        "events:notification",
        "events:workspace",
        "events:conversation",
        "events:presence",
        "events:typing",
        "events:huddle",
        "events:huddle-signal",
        "events:user",
        "events:session",
        "events:channel",
    ];
    for ch in &channels {
        if let Err(e) = pubsub.subscribe(ch).await {
            warn!("Failed to subscribe to {}: {}", ch, e);
        }
    }

    info!("Event consumer started, subscribed to: {:?}", channels);
    heartbeat.store(crate::now_unix(), Ordering::Relaxed);

    let mut stream = pubsub.into_on_message();

    loop {
        tokio::select! {
            maybe_msg = stream.next() => {
                let Some(msg) = maybe_msg else {
                    warn!("Event consumer stream ended");
                    return;
                };
                heartbeat.store(crate::now_unix(), Ordering::Relaxed);
                metrics::counter!("realtime_events_total").increment(1);

                let payload: String = match msg.get_payload() {
                    Ok(p) => p,
                    Err(_) => continue,
                };

                let event: serde_json::Value = match serde_json::from_str(&payload) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                let event_type = event
                    .get("event_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let event_payload = event.get("payload").cloned().unwrap_or_default();
                let stream_id = event
                    .get("stream_id")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);

                handle_event_for(
                    Audience::Everyone,
                    event_type,
                    &event_payload,
                    &cm,
                    stream_id.as_deref(),
                )
                .await;
            }
            _ = tokio::time::sleep(Duration::from_secs(15)) => {
                heartbeat.store(crate::now_unix(), Ordering::Relaxed);
            }
        }
    }
}

#[cfg(test)]
pub(crate) async fn handle_event(
    event_type: &str,
    payload: &serde_json::Value,
    cm: &Arc<ConnectionManager>,
) {
    handle_event_for(Audience::Everyone, event_type, payload, cm, None).await;
}

/// The live path and the replay path are the same function. Replay only narrows
/// the audience to one connection — every visibility rule below still applies,
/// which is what stops a backlog from carrying channels the client cannot see.
pub(crate) async fn handle_event_for(
    audience: Audience,
    event_type: &str,
    payload: &serde_json::Value,
    cm: &Arc<ConnectionManager>,
    stream_id: Option<&str>,
) {
    // Every frame carries the position it occupies in the workspace log, live or
    // replayed, because that is what the client sends back to resume.
    let framed = |frame: &ServerFrame| -> String {
        let mut value = serde_json::to_value(frame).unwrap_or_default();
        if let Some(id) = stream_id {
            if let Some(obj) = value.as_object_mut() {
                obj.insert("stream_id".into(), serde_json::json!(id));
            }
        }
        value.to_string()
    };
    let field = |name: &str| {
        payload
            .get(name)
            .and_then(|v| v.as_str())
            .and_then(|v| v.parse::<uuid::Uuid>().ok())
    };
    let text = |name: &str| {
        payload
            .get(name)
            .and_then(|v| v.as_str())
            .map(str::to_string)
    };

    match event_type {
        "message.created" => {
            if let Some(ch_id) = field("channel_id") {
                // Hoisted out of the payload so the badge delta is part of the
                // client contract rather than something riding along inside the
                // message body.
                let mentioned_user_ids = payload
                    .get("mentioned_user_ids")
                    .and_then(|v| v.as_array())
                    .map(|ids| {
                        ids.iter()
                            .filter_map(|v| v.as_str())
                            .filter_map(|v| v.parse().ok())
                            .collect()
                    })
                    .unwrap_or_default();
                let frame = ServerFrame::MessageNew {
                    message: payload.clone(),
                    mentioned_user_ids,
                };
                cm.broadcast_to_channel(audience, ch_id, &framed(&frame))
                    .await;
            }
        }
        "message.updated" => {
            if let Some(ch_id) = field("channel_id") {
                let frame = ServerFrame::MessageUpdated {
                    message: payload.clone(),
                };
                cm.broadcast_to_channel(audience, ch_id, &framed(&frame))
                    .await;
            }
        }
        "message.deleted" => {
            if let (Some(channel_id), Some(message_id)) = (field("channel_id"), field("message_id"))
            {
                let frame = ServerFrame::MessageDeleted {
                    message_id,
                    channel_id,
                };
                cm.broadcast_to_channel(audience, channel_id, &framed(&frame))
                    .await;
            }
        }
        "message.pinned" => {
            if let (Some(channel_id), Some(message_id)) = (field("channel_id"), field("message_id"))
            {
                let frame = ServerFrame::MessagePinned {
                    message_id,
                    channel_id,
                    pinned: payload
                        .get("pinned")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false),
                };
                cm.broadcast_to_channel(audience, channel_id, &framed(&frame))
                    .await;
            }
        }
        "reaction.added" => {
            if let (Some(ch_id), Some(message_id)) = (field("channel_id"), field("message_id")) {
                let frame = ServerFrame::ReactionAdded {
                    message_id,
                    reaction: payload.clone(),
                };
                cm.broadcast_to_channel(audience, ch_id, &framed(&frame))
                    .await;
            }
        }
        "reaction.removed" => {
            if let (Some(channel_id), Some(message_id), Some(user_id)) =
                (field("channel_id"), field("message_id"), field("user_id"))
            {
                let frame = ServerFrame::ReactionRemoved {
                    message_id,
                    channel_id,
                    user_id,
                    emoji: text("emoji").unwrap_or_default(),
                };
                cm.broadcast_to_channel(audience, channel_id, &framed(&frame))
                    .await;
            }
        }
        "workspace.deleted" => {
            if let Some(workspace_id) = field("workspace_id") {
                let frame = ServerFrame::WorkspaceDeleted {
                    workspace_id,
                    delete_type: text("delete_type").unwrap_or_default(),
                };
                cm.broadcast_to_workspace(audience, workspace_id, &framed(&frame))
                    .await;
            }
        }
        "workspace.restored" => {
            if let Some(workspace_id) = field("workspace_id") {
                let frame = ServerFrame::WorkspaceRestored { workspace_id };
                cm.broadcast_to_all(audience, &framed(&frame)).await;
            }
        }
        "notification.push" => {
            if let Some(uid) = field("user_id") {
                let frame = ServerFrame::Notification {
                    workspace_id: field("workspace_id"),
                    channel_id: field("channel_id"),
                    message_id: field("message_id"),
                    title: text("title").unwrap_or_default(),
                    body: text("body"),
                    priority: text("priority"),
                };
                cm.send_to_user(audience, uid, &framed(&frame)).await;
            }
        }
        "presence.changed" => {
            let workspace_ids: Vec<uuid::Uuid> = payload
                .get("workspace_ids")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .filter_map(|s| s.parse().ok())
                        .collect()
                })
                .unwrap_or_default();

            if let Some(subject_id) = field("user_id") {
                let frame = ServerFrame::PresenceChanged {
                    user_id: subject_id,
                    status: text("status").unwrap_or_default(),
                };
                cm.send_to_workspace_members(subject_id, &workspace_ids, &framed(&frame));
            }
        }
        "typing.indicator" => {
            let Some(user_id) = field("user_id") else {
                return;
            };
            let is_typing = payload
                .get("is_typing")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            let Some(channel_id) = field("channel_id") else {
                return;
            };
            let frame = ServerFrame::TypingIndicator {
                channel_id,
                user_id,
                is_typing,
            };
            cm.broadcast_to_channel(audience, channel_id, &framed(&frame))
                .await;
        }
        "conversation.created" => {
            let Some(frame) = typed(event_type, payload) else {
                return;
            };
            let msg = framed(&frame);
            for participant in participant_ids(payload) {
                cm.send_to_user(audience, participant, &msg).await;
            }
        }
        "huddle.member_joined"
        | "huddle.member_left"
        | "huddle.mute"
        | "huddle.camera"
        | "huddle.screenshare"
        | "huddle.reaction"
        | "huddle.hand" => {
            let (Some(hid), Some(frame)) = (field("huddle_id"), typed(event_type, payload)) else {
                return;
            };
            cm.broadcast_to_huddle(audience, hid, &framed(&frame)).await;
        }
        "huddle.offer" | "huddle.answer" | "huddle.ice" | "huddle.ring" => {
            let (Some(to_id), Some(frame)) = (field("to_user_id"), typed(event_type, payload))
            else {
                return;
            };
            cm.send_to_user(audience, to_id, &framed(&frame)).await;
        }
        "huddle.started" | "huddle.ended" => {
            let Some(frame) = typed(event_type, payload) else {
                return;
            };
            let msg = framed(&frame);
            if let Some(ch_id) = field("channel_id") {
                cm.broadcast_to_channel(audience, ch_id, &msg).await;
            } else {
                if let Some(init) = field("initiator_id") {
                    cm.send_to_user(audience, init, &msg).await;
                }
                if let Some(partner) = field("dm_partner_id") {
                    cm.send_to_user(audience, partner, &msg).await;
                }
            }
        }
        "channel.member_removed" => {
            let (Some(uid), Some(ch_id)) = (field("user_id"), field("channel_id")) else {
                return;
            };
            cm.leave_channel_for_user(uid, ch_id);
            let notice = ServerFrame::ChannelAccessRevoked {
                channel_id: ch_id,
                workspace_id: field("workspace_id"),
            };
            cm.send_to_user(audience, uid, &framed(&notice)).await;
        }
        "workspace.member_removed" => {
            let (Some(uid), Some(ws_id)) = (field("user_id"), field("workspace_id")) else {
                return;
            };
            cm.leave_workspace_for_user(uid, ws_id);
            cm.presence_leave_workspace(uid, ws_id).await;
            let notice = ServerFrame::WorkspaceAccessRevoked {
                workspace_id: ws_id,
            };
            cm.send_to_user(audience, uid, &framed(&notice)).await;
        }
        "user.suspended" => {
            if let Some(uid) = field("user_id") {
                cm.disconnect_user(uid, "account suspended");
            }
        }
        "session.revoked" => {
            let Some(uid) = field("user_id") else { return };
            let except = field("except_jti");
            let reason = payload
                .get("reason")
                .and_then(|v| v.as_str())
                .unwrap_or("session revoked");
            cm.disconnect_user_except(uid, except, reason);
        }
        _ => {
            tracing::debug!("Unhandled event type: {}", event_type);
        }
    }
}

fn participant_ids(payload: &serde_json::Value) -> Vec<uuid::Uuid> {
    payload
        .get("participant_ids")
        .and_then(|v| v.as_array())
        .map(|ids| {
            ids.iter()
                .filter_map(|v| v.as_str())
                .filter_map(|v| v.parse::<uuid::Uuid>().ok())
                .collect()
        })
        .unwrap_or_default()
}

/// A payload that does not read back as its frame is a producer bug, and the
/// place to find out is the gateway log rather than a client that quietly
/// ignores a shape it does not understand.
fn typed(event_type: &str, payload: &serde_json::Value) -> Option<ServerFrame> {
    match ServerFrame::from_event(event_type, payload) {
        Ok(frame) => Some(frame),
        Err(e) => {
            tracing::warn!(
                "event {} does not match its frame, dropped: {}",
                event_type,
                e
            );
            None
        }
    }
}
