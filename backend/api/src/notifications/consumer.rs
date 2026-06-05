use std::sync::Arc;

use futures_util::StreamExt;
use redis::AsyncCommands;
use tracing::{info, warn};

use super::models::NotificationType;
use super::repo::NotificationRepo;

pub async fn start_consumer(redis_url: &str, repo: Arc<NotificationRepo>) {
    let client = match redis::Client::open(redis_url) {
        Ok(c) => c,
        Err(e) => {
            warn!("Notification consumer: failed to connect Redis: {}", e);
            return;
        }
    };

    let mut pubsub = match client.get_async_pubsub().await {
        Ok(ps) => ps,
        Err(e) => {
            warn!("Notification consumer: failed to get pubsub: {}", e);
            return;
        }
    };

    let channels = ["events:message", "events:reaction"];
    for ch in &channels {
        if let Err(e) = pubsub.subscribe(ch).await {
            warn!(
                "Notification consumer: failed to subscribe to {}: {}",
                ch, e
            );
        }
    }

    info!("Notification consumer started");
    let mut stream = pubsub.into_on_message();

    let pub_client = match redis::Client::open(redis_url) {
        Ok(c) => c,
        Err(_) => return,
    };
    let mut pub_conn = match redis::aio::ConnectionManager::new(pub_client).await {
        Ok(c) => c,
        Err(_) => return,
    };

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
        let event_payload = match event.get("payload") {
            Some(p) => p.clone(),
            None => continue,
        };

        match event_type {
            "message.created" => {
                if let Some(mentioned) = event_payload
                    .get("mentioned_user_ids")
                    .and_then(|v| v.as_array())
                {
                    let sender_id = event_payload.get("user_id").and_then(|v| v.as_str());
                    let channel_id = event_payload.get("channel_id").and_then(|v| v.as_str());
                    let message_id = event_payload.get("id").and_then(|v| v.as_str());
                    let content = event_payload
                        .get("content")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    let workspace_id = event_payload
                        .get("workspace_id")
                        .and_then(|v| v.as_str())
                        .and_then(|v| v.parse::<uuid::Uuid>().ok());
                    let channel_uuid = channel_id.and_then(|v| v.parse::<uuid::Uuid>().ok());

                    for uid_val in mentioned {
                        if let Some(uid) =
                            uid_val.as_str().and_then(|v| v.parse::<uuid::Uuid>().ok())
                        {
                            if sender_id == Some(&uid.to_string()) {
                                continue;
                            }

                            if let Some(ch) = channel_uuid {
                                if repo.is_channel_muted(ch, uid).await.unwrap_or(false) {
                                    continue;
                                }
                            }

                            if let Some(ws_id) = workspace_id {
                                let data_json = serde_json::json!({
                                    "channel_id": channel_id,
                                    "message_id": message_id,
                                });

                                if let Err(e) = repo
                                    .create(
                                        uid,
                                        ws_id,
                                        &NotificationType::Mention,
                                        "You were mentioned",
                                        Some(content),
                                        &data_json,
                                    )
                                    .await
                                {
                                    warn!(
                                        user_id = %uid,
                                        workspace_id = %ws_id,
                                        "Notification consumer: failed to persist mention notification: {}",
                                        e
                                    );
                                }
                            }

                            if repo.is_dnd_active(uid).await.unwrap_or(false) {
                                continue;
                            }

                            let notif_event = serde_json::json!({
                                "event_type": "notification.push",
                                "payload": {
                                    "user_id": uid.to_string(),
                                    "workspace_id": workspace_id.map(|w| w.to_string()),
                                    "channel_id": channel_id,
                                    "message_id": message_id,
                                    "title": "You were mentioned",
                                    "body": content,
                                    "priority": "mention",
                                }
                            });

                            let json = serde_json::to_string(&notif_event).unwrap_or_default();
                            let _: Result<(), _> =
                                pub_conn.publish("events:notification", &json).await;
                        }
                    }
                }
            }
            "reaction.added" => {}
            _ => {}
        }
    }
}
