use std::sync::Arc;

use redis::AsyncCommands;
use tracing::{info, warn};

use super::repo::HuddleRepo;
use crate::messaging::stream_group::StreamGroup;

/// Huddle membership is history, not a live signal: it is written to
/// `huddle_participants` and read back as a call log. So it reads through a
/// consumer group with acknowledgement, like notifications and hooks — which is
/// what makes more than one `chat-worker` replica safe.
///
/// `record_join` is an upsert and `end_session` only succeeds once, so a
/// redelivery is a no-op rather than a second row.
pub async fn start_consumer(redis_url: &str, repo: Arc<HuddleRepo>) {
    let Some(mut group) = StreamGroup::connect(redis_url, "huddle").await else {
        return;
    };
    let mut pub_conn = group.connection();

    info!("Huddle consumer started");

    loop {
        for delivery in group.next_batch().await {
            // An async block so that skipping an event cannot skip its
            // acknowledgement: `continue` will not compile here.
            async {
                handle(
                    &repo,
                    &mut pub_conn,
                    &delivery.event.event_type,
                    &delivery.event.payload,
                )
                .await;
            }
            .await;

            group.ack(&delivery.key, &delivery.id).await;
        }
    }
}

async fn handle(
    repo: &HuddleRepo,
    pub_conn: &mut redis::aio::ConnectionManager,
    event_type: &str,
    payload: &serde_json::Value,
) {
    if event_type != "huddle.member_joined" && event_type != "huddle.member_left" {
        return;
    }

    let huddle_id = payload
        .get("huddle_id")
        .and_then(|v| v.as_str())
        .and_then(|v| v.parse::<uuid::Uuid>().ok());
    let user_id = payload
        .get("user_id")
        .and_then(|v| v.as_str())
        .and_then(|v| v.parse::<uuid::Uuid>().ok());
    let (Some(huddle_id), Some(user_id)) = (huddle_id, user_id) else {
        return;
    };

    if event_type == "huddle.member_joined" {
        if let Err(e) = repo.record_join(huddle_id, user_id).await {
            warn!(
                "Huddle consumer: record_join failed huddle={} user={}: {}",
                huddle_id, user_id, e
            );
        }
        return;
    }

    match repo.record_leave(huddle_id, user_id).await {
        // Only the first caller gets a session back, so two replicas racing on
        // the last participant still end it once.
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
