use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use tracing::{info, warn};

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
        "events:dm",
        "events:presence",
        "events:typing",
        "events:huddle",
        "events:huddle-signal",
        "events:user",
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

                handle_event(event_type, &event_payload, &cm).await;
            }
            _ = tokio::time::sleep(Duration::from_secs(15)) => {
                heartbeat.store(crate::now_unix(), Ordering::Relaxed);
            }
        }
    }
}

pub(crate) async fn handle_event(
    event_type: &str,
    payload: &serde_json::Value,
    cm: &Arc<ConnectionManager>,
) {
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
                let ws_msg = serde_json::json!({
                    "type": "message.new",
                    "message": payload,
                });
                cm.broadcast_to_channel(ch_id, &ws_msg.to_string()).await;
            }
        }
        "message.updated" => {
            if let Some(ch_id) = channel_id {
                let ws_msg = serde_json::json!({
                    "type": "message.updated",
                    "message": payload,
                });
                cm.broadcast_to_channel(ch_id, &ws_msg.to_string()).await;
            }
        }
        "message.deleted" => {
            if let Some(ch_id) = channel_id {
                let ws_msg = serde_json::json!({
                    "type": "message.deleted",
                    "message_id": payload.get("message_id"),
                    "channel_id": ch_id,
                });
                cm.broadcast_to_channel(ch_id, &ws_msg.to_string()).await;
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
                cm.broadcast_to_channel(ch_id, &ws_msg.to_string()).await;
            }
        }
        "reaction.added" => {
            if let Some(ch_id) = channel_id {
                let ws_msg = serde_json::json!({
                    "type": "reaction.added",
                    "message_id": payload.get("message_id"),
                    "reaction": payload,
                });
                cm.broadcast_to_channel(ch_id, &ws_msg.to_string()).await;
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
                cm.broadcast_to_channel(ch_id, &ws_msg.to_string()).await;
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
                cm.broadcast_to_workspace(ws_id, &ws_msg.to_string()).await;
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
                cm.broadcast_to_all(&ws_msg.to_string()).await;
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
                cm.send_to_user(uid, &ws_msg.to_string()).await;
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
                cm.send_to_workspace_members(subject_id, &workspace_ids, &ws_msg.to_string());
            }
        }
        "typing.indicator" => {
            if let Some(ch_id) = channel_id {
                let user_id = payload.get("user_id");
                let ws_msg = serde_json::json!({
                    "type": "typing.indicator",
                    "channel_id": ch_id,
                    "user_id": user_id,
                    "is_typing": payload.get("is_typing").and_then(serde_json::Value::as_bool).unwrap_or(false),
                });
                cm.broadcast_to_channel(ch_id, &ws_msg.to_string()).await;
            }
        }
        "dm.created" => {
            let from_user_id = payload
                .get("from_user_id")
                .and_then(|v| v.as_str())
                .and_then(|v| v.parse::<uuid::Uuid>().ok());
            let to_user_id = payload
                .get("to_user_id")
                .and_then(|v| v.as_str())
                .and_then(|v| v.parse::<uuid::Uuid>().ok());

            if let (Some(from_id), Some(to_id)) = (from_user_id, to_user_id) {
                let ws_event = serde_json::json!({
                    "type": "dm.new",
                    "message": payload,
                });
                let msg = ws_event.to_string();
                cm.send_to_user(from_id, &msg).await;
                cm.send_to_user(to_id, &msg).await;
            }
        }
        "dm.updated" | "dm.deleted" => {
            let from_user_id = payload
                .get("from_user_id")
                .and_then(|v| v.as_str())
                .and_then(|v| v.parse::<uuid::Uuid>().ok());
            let to_user_id = payload
                .get("to_user_id")
                .and_then(|v| v.as_str())
                .and_then(|v| v.parse::<uuid::Uuid>().ok());

            if let (Some(from_id), Some(to_id)) = (from_user_id, to_user_id) {
                let ws_type = if event_type == "dm.updated" {
                    "dm.updated"
                } else {
                    "dm.deleted"
                };
                let ws_event = serde_json::json!({
                    "type": ws_type,
                    "message": payload,
                });
                let msg = ws_event.to_string();
                cm.send_to_user(from_id, &msg).await;
                cm.send_to_user(to_id, &msg).await;
            }
        }
        "dm.reaction.added" | "dm.reaction.removed" => {
            let from_user_id = payload
                .get("from_user_id")
                .and_then(|v| v.as_str())
                .and_then(|v| v.parse::<uuid::Uuid>().ok());
            let to_user_id = payload
                .get("to_user_id")
                .and_then(|v| v.as_str())
                .and_then(|v| v.parse::<uuid::Uuid>().ok());

            if let (Some(from_id), Some(to_id)) = (from_user_id, to_user_id) {
                let mut ws_event = payload.clone();
                if let Some(obj) = ws_event.as_object_mut() {
                    obj.insert("type".to_string(), serde_json::json!(event_type));
                }
                let msg = ws_event.to_string();
                cm.send_to_user(from_id, &msg).await;
                cm.send_to_user(to_id, &msg).await;
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
                cm.broadcast_to_huddle(hid, &ws_msg.to_string()).await;
            }
        }
        "huddle.offer" | "huddle.answer" | "huddle.ice" | "huddle.ring" => {
            if let Some(to_id) = to_user_id {
                let mut ws_msg = payload.clone();
                if let Some(obj) = ws_msg.as_object_mut() {
                    obj.insert("type".to_string(), serde_json::json!(event_type));
                }
                cm.send_to_user(to_id, &ws_msg.to_string()).await;
            }
        }
        "huddle.started" | "huddle.ended" => {
            let mut ws_msg = payload.clone();
            if let Some(obj) = ws_msg.as_object_mut() {
                obj.insert("type".to_string(), serde_json::json!(event_type));
            }
            let msg = ws_msg.to_string();
            if let Some(ch_id) = channel_id {
                cm.broadcast_to_channel(ch_id, &msg).await;
            } else {
                let initiator = payload
                    .get("initiator_id")
                    .and_then(|v| v.as_str())
                    .and_then(|v| v.parse::<uuid::Uuid>().ok());
                if let Some(init) = initiator {
                    cm.send_to_user(init, &msg).await;
                }
                if let Some(partner) = payload
                    .get("dm_partner_id")
                    .and_then(|v| v.as_str())
                    .and_then(|v| v.parse::<uuid::Uuid>().ok())
                {
                    cm.send_to_user(partner, &msg).await;
                }
            }
        }
        "user.suspended" => {
            if let Some(uid) = payload
                .get("user_id")
                .and_then(|v| v.as_str())
                .and_then(|v| v.parse::<uuid::Uuid>().ok())
            {
                cm.disconnect_user(uid);
            }
        }
        _ => {
            tracing::debug!("Unhandled event type: {}", event_type);
        }
    }
}
