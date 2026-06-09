use std::sync::Arc;

use futures_util::StreamExt;
use redis::AsyncCommands;
use tracing::{info, warn};

use super::repo::HuddleRepo;

pub async fn start_consumer(redis_url: &str, repo: Arc<HuddleRepo>) {
    let client = match redis::Client::open(redis_url) {
        Ok(c) => c,
        Err(e) => {
            warn!("Huddle consumer: failed to connect Redis: {}", e);
            return;
        }
    };

    let mut pubsub = match client.get_async_pubsub().await {
        Ok(ps) => ps,
        Err(e) => {
            warn!("Huddle consumer: failed to get pubsub: {}", e);
            return;
        }
    };

    if let Err(e) = pubsub.subscribe("events:huddle").await {
        warn!(
            "Huddle consumer: failed to subscribe to events:huddle: {}",
            e
        );
        return;
    }

    info!("Huddle consumer started");
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
        if event_type != "huddle.member_joined" && event_type != "huddle.member_left" {
            continue;
        }
        let p = match event.get("payload") {
            Some(p) => p,
            None => continue,
        };
        let huddle_id = p
            .get("huddle_id")
            .and_then(|v| v.as_str())
            .and_then(|v| v.parse::<uuid::Uuid>().ok());
        let user_id = p
            .get("user_id")
            .and_then(|v| v.as_str())
            .and_then(|v| v.parse::<uuid::Uuid>().ok());
        let (Some(huddle_id), Some(user_id)) = (huddle_id, user_id) else {
            continue;
        };

        if event_type == "huddle.member_joined" {
            if let Err(e) = repo.record_join(huddle_id, user_id).await {
                warn!(
                    "Huddle consumer: record_join failed huddle={} user={}: {}",
                    huddle_id, user_id, e
                );
            }
            continue;
        }

        match repo.record_leave(huddle_id, user_id).await {
            Ok(0) => match repo.end_session(huddle_id).await {
                Ok(Some(session)) => {
                    let ended = serde_json::json!({
                        "event_type": "huddle.ended",
                        "payload": {
                            "huddle_id": session.id,
                            "workspace_id": session.workspace_id,
                            "channel_id": session.channel_id,
                            "dm_partner_id": session.dm_partner_id,
                            "initiator_id": session.initiated_by,
                        }
                    });
                    let json = serde_json::to_string(&ended).unwrap_or_default();
                    let _: Result<(), _> = pub_conn.publish("events:huddle", &json).await;
                }
                Ok(None) => {}
                Err(e) => warn!(
                    "Huddle consumer: end_session failed huddle={}: {}",
                    huddle_id, e
                ),
            },
            Ok(_) => {}
            Err(e) => warn!(
                "Huddle consumer: record_leave failed huddle={} user={}: {}",
                huddle_id, user_id, e
            ),
        }
    }
}
