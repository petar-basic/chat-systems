use std::sync::Arc;

use futures_util::StreamExt;
use redis::AsyncCommands;
use tracing::{info, warn};

use super::models::NotificationType;
use super::repo::NotificationRepo;
use crate::messaging::stream_group::StreamGroup;

pub async fn start_consumer(redis_url: &str, repo: Arc<NotificationRepo>) {
    let Some(mut group) = StreamGroup::connect(redis_url, "notifications").await else {
        return;
    };
    let mut pub_conn = group.connection();

    info!("Notification consumer started");

    loop {
        for delivery in group.next_batch().await {
            let event_type = delivery.event.event_type.as_str();
            let event_payload = delivery.event.payload.clone();

            // An async block rather than a bare match so that skipping an event
            // cannot skip its acknowledgement: `continue` will not compile here,
            // which is the point.
            async {

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
            .await;

            group.ack(&delivery.key, &delivery.id).await;
        }
    }
}

/// Call notifications stay on pub/sub because the event that triggers them does.
/// A ring is worth nothing a minute later, so it is not in the replay log — and
/// that means the stream consumer never sees it.
pub async fn start_call_consumer(redis_url: &str, repo: Arc<NotificationRepo>) {
    let client = match redis::Client::open(redis_url) {
        Ok(c) => c,
        Err(e) => {
            warn!("Call notification consumer: failed to connect Redis: {}", e);
            return;
        }
    };
    let mut pubsub = match client.get_async_pubsub().await {
        Ok(ps) => ps,
        Err(e) => {
            warn!("Call notification consumer: failed to get pubsub: {}", e);
            return;
        }
    };
    if let Err(e) = pubsub.subscribe("events:huddle").await {
        warn!("Call notification consumer: failed to subscribe: {}", e);
        return;
    }

    let mut pub_conn = match redis::Client::open(redis_url) {
        Ok(c) => match redis::aio::ConnectionManager::new(c).await {
            Ok(conn) => conn,
            Err(_) => return,
        },
        Err(_) => return,
    };

    info!("Call notification consumer started");
    let mut stream = pubsub.into_on_message();

    while let Some(msg) = stream.next().await {
        let Ok(raw) = msg.get_payload::<String>() else {
            continue;
        };
        let Ok(event) = serde_json::from_str::<serde_json::Value>(&raw) else {
            continue;
        };
        if event.get("event_type").and_then(|v| v.as_str()) != Some("huddle.ring") {
            continue;
        }
        let Some(event_payload) = event.get("payload").cloned() else {
            continue;
        };

        async {
            let to_user_id = event_payload
                .get("to_user_id")
                .and_then(|v| v.as_str())
                .and_then(|v| v.parse::<uuid::Uuid>().ok());
            let workspace_id = event_payload
                .get("workspace_id")
                .and_then(|v| v.as_str())
                .and_then(|v| v.parse::<uuid::Uuid>().ok());

            if let (Some(uid), Some(ws_id)) = (to_user_id, workspace_id) {
                let data_json = serde_json::json!({
                    "huddle_id": event_payload.get("huddle_id"),
                    "from_user_id": event_payload.get("from_user_id"),
                });

                if let Err(e) = repo
                    .create(
                        uid,
                        ws_id,
                        &NotificationType::Call,
                        "Incoming huddle",
                        Some("Someone is starting a huddle"),
                        &data_json,
                    )
                    .await
                {
                    warn!(
                        user_id = %uid,
                        "Huddle consumer: failed to persist call notification: {}", e
                    );
                }

                if repo.is_dnd_active(uid).await.unwrap_or(false) {
                    return;
                }

                let notif_event = serde_json::json!({
                    "event_type": "notification.push",
                    "payload": {
                        "user_id": uid.to_string(),
                        "workspace_id": ws_id.to_string(),
                        "title": "Incoming huddle",
                        "body": "Someone is starting a huddle",
                        "priority": "call",
                    }
                });
                let json = serde_json::to_string(&notif_event).unwrap_or_default();
                let _: Result<(), _> = pub_conn.publish("events:notification", &json).await;
            }
        }
        .await;
    }

    warn!("Call notification consumer: event stream ended, exiting for restart");
}
