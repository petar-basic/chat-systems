use std::sync::Arc;

use futures_util::StreamExt;
use tracing::{info, warn};

use crate::connection_manager::ConnectionManager;

pub async fn start_event_consumer(redis_url: &str, cm: Arc<ConnectionManager>) {
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
    ];
    for ch in &channels {
        if let Err(e) = pubsub.subscribe(ch).await {
            warn!("Failed to subscribe to {}: {}", ch, e);
        }
    }

    info!("Event consumer started, subscribed to: {:?}", channels);

    let mut stream = pubsub.into_on_message();

    while let Some(msg) = stream.next().await {
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

            if let Some(subject_id) = subject {
                let ws_msg = serde_json::json!({
                    "type": "presence.changed",
                    "user_id": subject_id,
                    "status": status,
                });
                let msg = ws_msg.to_string();
                for uid in cm.local_users() {
                    if uid != subject_id {
                        cm.send_to_user(uid, &msg).await;
                    }
                }
            }
        }
        "typing.indicator" => {
            if let Some(ch_id) = channel_id {
                let user_id = payload.get("user_id");
                let ws_msg = serde_json::json!({
                    "type": "typing.indicator",
                    "channel_id": ch_id,
                    "user_id": user_id,
                    "is_typing": payload.get("is_typing").and_then(|v| v.as_bool()).unwrap_or(false),
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
        _ => {
            tracing::debug!("Unhandled event type: {}", event_type);
        }
    }
}
