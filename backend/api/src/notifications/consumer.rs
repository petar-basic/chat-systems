use std::sync::Arc;

use redis::AsyncCommands;
use tracing::{info, warn};

use shared_events::Event;

use super::models::NotificationType;
use super::repo::NotificationRepo;
use crate::messaging::stream_group::StreamGroup;
use crate::push::sender::{PushPayload, PushSender};

pub const RING_MAX_AGE: chrono::Duration = chrono::Duration::seconds(60);

pub async fn start_consumer(
    redis_url: &str,
    app_state: Arc<crate::state::AppState>,
    repo: Arc<NotificationRepo>,
    push: Arc<PushSender>,
) {
    let Some(mut group) = StreamGroup::connect(redis_url, "notifications").await else {
        return;
    };
    let mut pub_conn = group.connection();

    info!("Notification consumer started");

    loop {
        for delivery in group.next_batch().await {
            // An async block rather than a bare match so that skipping an event
            // cannot skip its acknowledgement: `continue` will not compile here,
            // which is the point.
            async {
                match delivery.event.event_type.as_str() {
                    "message.created" => {
                        notify_mentions(
                            &app_state,
                            &repo,
                            &push,
                            &mut pub_conn,
                            &delivery.event.payload,
                        )
                        .await;
                    }
                    "huddle.ring" => notify_ring(&repo, &mut pub_conn, &delivery.event).await,
                    _ => {}
                }
            }
            .await;

            group.ack(&delivery.key, &delivery.id).await;
        }
    }
}

/// Extracted from the loop so the suppression rules can be tested without
/// driving a stream: everything that decides whether somebody hears about a
/// mention lives here.
pub async fn notify_mentions(
    state: &crate::state::AppState,
    repo: &NotificationRepo,
    push: &PushSender,
    pub_conn: &mut redis::aio::ConnectionManager,
    event_payload: &serde_json::Value,
) {
    let Some(mentioned) = event_payload
        .get("mentioned_user_ids")
        .and_then(|v| v.as_array())
    else {
        return;
    };

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
        let Some(uid) = uid_val.as_str().and_then(|v| v.parse::<uuid::Uuid>().ok()) else {
            continue;
        };
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
        let _: Result<(), _> = pub_conn.publish("events:notification", &json).await;

        // The in-app notification has already fired for anybody holding a
        // socket; a second one on their phone for the message they are looking
        // at is how people turn notifications off altogether.
        if let Some(ws_id) = workspace_id {
            if crate::presence::is_online(pub_conn, ws_id, uid).await {
                continue;
            }

            let badge = repo.unread_count(uid, ws_id).await.unwrap_or(0);
            let reached = push
                .send_to_user(
                    uid,
                    &PushPayload {
                        title: "You were mentioned".into(),
                        body: PushPayload::preview(content),
                        workspace_id: Some(ws_id.to_string()),
                        channel_id: channel_id.map(str::to_string),
                        message_id: message_id.map(str::to_string),
                        badge_count: badge,
                    },
                )
                .await;

            // No socket, and no device that could be woken. Without this the
            // mention waits until they next open the app, which for somebody on
            // holiday is not a notification system.
            if !reached {
                let channel_name = match channel_uuid {
                    Some(id) => state
                        .workspace_service
                        .repo
                        .find_channel_by_id(id)
                        .await
                        .ok()
                        .flatten()
                        .and_then(|c| c.name)
                        .unwrap_or_default(),
                    None => String::new(),
                };
                let sender_name = match sender_id.and_then(|v| v.parse::<uuid::Uuid>().ok()) {
                    Some(id) => state
                        .auth_service
                        .repo()
                        .find_by_id(id)
                        .await
                        .ok()
                        .flatten()
                        .and_then(|u| u.display_name)
                        .unwrap_or_else(|| "Somebody".to_string()),
                    None => "Somebody".to_string(),
                };

                super::email::enqueue(
                    state,
                    super::email::PendingMention {
                        user_id: uid,
                        workspace_id: ws_id,
                        channel_id: channel_uuid,
                        message_id: message_id.and_then(|v| v.parse().ok()),
                        sender_name: &sender_name,
                        channel_name: &channel_name,
                    },
                )
                .await;
            }
        }
    }
}

/// A ring that reaches the worker late — redelivered after a crash, or read
/// from a backlog — is dropped rather than rung: a call announced a minute
/// after it started is worse than one not announced. A ring delivered twice
/// creates one notification, because the row is unique per person and call.
pub async fn notify_ring(
    repo: &NotificationRepo,
    pub_conn: &mut redis::aio::ConnectionManager,
    event: &Event,
) {
    if chrono::Utc::now() - event.timestamp > RING_MAX_AGE {
        return;
    }

    let field = |name: &str| {
        event
            .payload
            .get(name)
            .and_then(|v| v.as_str())
            .and_then(|v| v.parse::<uuid::Uuid>().ok())
    };
    let (Some(to_user_id), Some(workspace_id), Some(huddle_id)) = (
        field("to_user_id"),
        field("workspace_id"),
        field("huddle_id"),
    ) else {
        return;
    };

    let data = serde_json::json!({
        "huddle_id": huddle_id,
        "from_user_id": event.payload.get("from_user_id"),
    });

    let created = match repo
        .create(
            to_user_id,
            workspace_id,
            &NotificationType::Call,
            "Incoming huddle",
            Some("Someone is starting a huddle"),
            &data,
        )
        .await
    {
        Ok(created) => created,
        Err(e) => {
            warn!(user_id = %to_user_id, "failed to persist call notification: {}", e);
            return;
        }
    };
    if created.is_none() {
        return;
    }

    if repo.is_dnd_active(to_user_id).await.unwrap_or(false) {
        return;
    }

    let notif_event = serde_json::json!({
        "event_type": "notification.push",
        "payload": {
            "user_id": to_user_id.to_string(),
            "workspace_id": workspace_id.to_string(),
            "title": "Incoming huddle",
            "body": "Someone is starting a huddle",
            "priority": "call",
        }
    });
    let json = serde_json::to_string(&notif_event).unwrap_or_default();
    let _: Result<(), _> = pub_conn.publish("events:notification", &json).await;
}
