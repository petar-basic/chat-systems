use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use tracing::{info, warn};

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
    let framed = |mut value: serde_json::Value| -> String {
        if let Some(id) = stream_id {
            if let Some(obj) = value.as_object_mut() {
                obj.insert("stream_id".into(), serde_json::json!(id));
            }
        }
        value.to_string()
    };

    let channel_id = payload
        .get("channel_id")
        .and_then(|v| v.as_str())
        .and_then(|v| v.parse::<uuid::Uuid>().ok());
    let huddle_id = payload
        .get("huddle_id")
        .and_then(|v| v.as_str())
        .and_then(|v| v.parse::<uuid::Uuid>().ok());
    let to_user_id = payload
        .get("to_user_id")
        .and_then(|v| v.as_str())
        .and_then(|v| v.parse::<uuid::Uuid>().ok());

    match event_type {
        "message.created" => {
            if let Some(ch_id) = channel_id {
                // Hoisted out of the payload so the badge delta is part of the
                // client contract rather than something riding along inside the
                // message body.
                let ws_msg = serde_json::json!({
                    "type": "message.new",
                    "message": payload,
                    "mentioned_user_ids": payload
                        .get("mentioned_user_ids")
                        .cloned()
                        .unwrap_or_else(|| serde_json::json!([])),
                });
                cm.broadcast_to_channel(audience, ch_id, &framed(ws_msg))
                    .await;
            }
        }
        "message.updated" => {
            if let Some(ch_id) = channel_id {
                let ws_msg = serde_json::json!({
                    "type": "message.updated",
                    "message": payload,
                });
                cm.broadcast_to_channel(audience, ch_id, &framed(ws_msg))
                    .await;
            }
        }
        "message.deleted" => {
            if let Some(ch_id) = channel_id {
                let ws_msg = serde_json::json!({
                    "type": "message.deleted",
                    "message_id": payload.get("message_id"),
                    "channel_id": ch_id,
                });
                cm.broadcast_to_channel(audience, ch_id, &framed(ws_msg))
                    .await;
            }
        }
        "message.pinned" => {
            if let Some(ch_id) = channel_id {
                let ws_msg = serde_json::json!({
                    "type": "message.pinned",
                    "message_id": payload.get("message_id"),
                    "channel_id": ch_id,
                    "pinned": payload.get("pinned"),
                });
                cm.broadcast_to_channel(audience, ch_id, &framed(ws_msg))
                    .await;
            }
        }
        "reaction.added" => {
            if let Some(ch_id) = channel_id {
                let ws_msg = serde_json::json!({
                    "type": "reaction.added",
                    "message_id": payload.get("message_id"),
                    "reaction": payload,
                });
                cm.broadcast_to_channel(audience, ch_id, &framed(ws_msg))
                    .await;
            }
        }
        "reaction.removed" => {
            if let Some(ch_id) = channel_id {
                let ws_msg = serde_json::json!({
                    "type": "reaction.removed",
                    "message_id": payload.get("message_id"),
                    "channel_id": ch_id,
                    "user_id": payload.get("user_id"),
                    "emoji": payload.get("emoji"),
                });
                cm.broadcast_to_channel(audience, ch_id, &framed(ws_msg))
                    .await;
            }
        }
        "workspace.deleted" => {
            let workspace_id = payload
                .get("workspace_id")
                .and_then(|v| v.as_str())
                .and_then(|v| v.parse::<uuid::Uuid>().ok());

            if let Some(ws_id) = workspace_id {
                let ws_msg = serde_json::json!({
                    "type": "workspace.deleted",
                    "workspace_id": ws_id,
                    "delete_type": payload.get("delete_type"),
                });
                cm.broadcast_to_workspace(audience, ws_id, &framed(ws_msg))
                    .await;
            }
        }
        "workspace.restored" => {
            let workspace_id = payload
                .get("workspace_id")
                .and_then(|v| v.as_str())
                .and_then(|v| v.parse::<uuid::Uuid>().ok());

            if let Some(ws_id) = workspace_id {
                let ws_msg = serde_json::json!({
                    "type": "workspace.restored",
                    "workspace_id": ws_id,
                });
                cm.broadcast_to_all(audience, &framed(ws_msg)).await;
            }
        }
        "notification.push" => {
            let user_id = payload
                .get("user_id")
                .and_then(|v| v.as_str())
                .and_then(|v| v.parse::<uuid::Uuid>().ok());

            if let Some(uid) = user_id {
                let ws_msg = serde_json::json!({
                    "type": "notification",
                    "workspace_id": payload.get("workspace_id"),
                    "channel_id": payload.get("channel_id"),
                    "message_id": payload.get("message_id"),
                    "title": payload.get("title"),
                    "body": payload.get("body"),
                    "priority": payload.get("priority"),
                });
                cm.send_to_user(audience, uid, &framed(ws_msg)).await;
            }
        }
        "presence.changed" => {
            let subject = payload
                .get("user_id")
                .and_then(|v| v.as_str())
                .and_then(|v| v.parse::<uuid::Uuid>().ok());
            let status = payload.get("status").and_then(|v| v.as_str()).unwrap_or("");
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

            if let Some(subject_id) = subject {
                let ws_msg = serde_json::json!({
                    "type": "presence.changed",
                    "user_id": subject_id,
                    "status": status,
                });
                cm.send_to_workspace_members(subject_id, &workspace_ids, &framed(ws_msg));
            }
        }
        "typing.indicator" => {
            let user_id = payload.get("user_id");
            let is_typing = payload
                .get("is_typing")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            if let Some(ch_id) = channel_id {
                let ws_msg = serde_json::json!({
                    "type": "typing.indicator",
                    "channel_id": ch_id,
                    "user_id": user_id,
                    "is_typing": is_typing,
                });
                cm.broadcast_to_channel(audience, ch_id, &framed(ws_msg))
                    .await;
            } else if let Some(conv_id) = payload
                .get("conversation_id")
                .and_then(|v| v.as_str())
                .and_then(|v| v.parse::<uuid::Uuid>().ok())
            {
                let participants: Vec<uuid::Uuid> = payload
                    .get("participant_ids")
                    .and_then(|v| v.as_array())
                    .map(|ids| {
                        ids.iter()
                            .filter_map(|v| v.as_str())
                            .filter_map(|v| v.parse::<uuid::Uuid>().ok())
                            .collect()
                    })
                    .unwrap_or_default();
                let ws_msg = framed(serde_json::json!({
                    "type": "typing.indicator",
                    "conversation_id": conv_id,
                    "user_id": user_id,
                    "is_typing": is_typing,
                }));
                for participant in participants {
                    cm.send_to_user(audience, participant, &ws_msg).await;
                }
            }
        }
        "conversation.created"
        | "conversation.message.created"
        | "conversation.message.updated"
        | "conversation.message.deleted"
        | "conversation.reaction.added"
        | "conversation.reaction.removed" => {
            let participants: Vec<uuid::Uuid> = payload
                .get("participant_ids")
                .and_then(|v| v.as_array())
                .map(|ids| {
                    ids.iter()
                        .filter_map(|v| v.as_str())
                        .filter_map(|v| v.parse::<uuid::Uuid>().ok())
                        .collect()
                })
                .unwrap_or_default();

            let mut ws_event = payload.clone();
            if let Some(obj) = ws_event.as_object_mut() {
                obj.insert("type".to_string(), serde_json::json!(event_type));
            }
            let msg = ws_event.to_string();
            for participant in participants {
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
            if let Some(hid) = huddle_id {
                let mut ws_msg = payload.clone();
                if let Some(obj) = ws_msg.as_object_mut() {
                    obj.insert("type".to_string(), serde_json::json!(event_type));
                }
                cm.broadcast_to_huddle(audience, hid, &framed(ws_msg)).await;
            }
        }
        "huddle.offer" | "huddle.answer" | "huddle.ice" | "huddle.ring" => {
            if let Some(to_id) = to_user_id {
                let mut ws_msg = payload.clone();
                if let Some(obj) = ws_msg.as_object_mut() {
                    obj.insert("type".to_string(), serde_json::json!(event_type));
                }
                cm.send_to_user(audience, to_id, &framed(ws_msg)).await;
            }
        }
        "huddle.started" | "huddle.ended" => {
            let mut ws_msg = payload.clone();
            if let Some(obj) = ws_msg.as_object_mut() {
                obj.insert("type".to_string(), serde_json::json!(event_type));
            }
            let msg = ws_msg.to_string();
            if let Some(ch_id) = channel_id {
                cm.broadcast_to_channel(audience, ch_id, &msg).await;
            } else {
                let initiator = payload
                    .get("initiator_id")
                    .and_then(|v| v.as_str())
                    .and_then(|v| v.parse::<uuid::Uuid>().ok());
                if let Some(init) = initiator {
                    cm.send_to_user(audience, init, &msg).await;
                }
                if let Some(partner) = payload
                    .get("dm_partner_id")
                    .and_then(|v| v.as_str())
                    .and_then(|v| v.parse::<uuid::Uuid>().ok())
                {
                    cm.send_to_user(audience, partner, &msg).await;
                }
            }
        }
        "channel.member_removed" => {
            let Some(uid) = payload
                .get("user_id")
                .and_then(|v| v.as_str())
                .and_then(|v| v.parse::<uuid::Uuid>().ok())
            else {
                return;
            };
            let Some(ch_id) = channel_id else { return };
            cm.leave_channel_for_user(uid, ch_id);
            let notice = serde_json::json!({
                "type": "channel.access_revoked",
                "channel_id": ch_id,
                "workspace_id": payload.get("workspace_id"),
            });
            cm.send_to_user(audience, uid, &notice.to_string()).await;
        }
        "workspace.member_removed" => {
            let Some(uid) = payload
                .get("user_id")
                .and_then(|v| v.as_str())
                .and_then(|v| v.parse::<uuid::Uuid>().ok())
            else {
                return;
            };
            let Some(ws_id) = payload
                .get("workspace_id")
                .and_then(|v| v.as_str())
                .and_then(|v| v.parse::<uuid::Uuid>().ok())
            else {
                return;
            };
            cm.leave_workspace_for_user(uid, ws_id);
            cm.presence_leave_workspace(uid, ws_id).await;
            let notice = serde_json::json!({
                "type": "workspace.access_revoked",
                "workspace_id": ws_id,
            });
            cm.send_to_user(audience, uid, &notice.to_string()).await;
        }
        "user.suspended" => {
            if let Some(uid) = payload
                .get("user_id")
                .and_then(|v| v.as_str())
                .and_then(|v| v.parse::<uuid::Uuid>().ok())
            {
                cm.disconnect_user(uid, "account suspended");
            }
        }
        "session.revoked" => {
            let Some(uid) = payload
                .get("user_id")
                .and_then(|v| v.as_str())
                .and_then(|v| v.parse::<uuid::Uuid>().ok())
            else {
                return;
            };
            let except = payload
                .get("except_jti")
                .and_then(|v| v.as_str())
                .and_then(|v| v.parse::<uuid::Uuid>().ok());
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
