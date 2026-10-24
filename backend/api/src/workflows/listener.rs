use std::sync::Arc;

use tracing::{info, warn};
use uuid::Uuid;

use shared_events::Event;

use super::routes::RUNS_PER_MINUTE;
use crate::messaging::stream_group::StreamGroup;
use crate::rate_limit::{self, LimiterFailure};
use crate::state::AppState;

pub async fn start_listener(state: Arc<AppState>) {
    let Some(mut group) = StreamGroup::connect(&state.config.redis_url, "workflows").await else {
        return;
    };

    info!("Workflow listener started");

    loop {
        for delivery in group.next_batch().await {
            async {
                if let Err(e) = handle(&state, &delivery.event).await {
                    warn!(
                        event_id = %delivery.event.id,
                        "could not start the workflows an event triggers: {}", e
                    );
                }
            }
            .await;

            group.ack(&delivery.key, &delivery.id).await;
        }
    }
}

struct Fired {
    kind: &'static str,
    channel_id: Uuid,
    user_id: Uuid,
    emoji: Option<String>,
    message_id: Option<Uuid>,
}

fn fired(event: &Event) -> Option<Fired> {
    let id = |key: &str| event.payload.get(key)?.as_str()?.parse::<Uuid>().ok();
    match event.event_type.as_str() {
        "reaction.added" => Some(Fired {
            kind: "reaction_added",
            channel_id: id("channel_id")?,
            user_id: id("user_id")?,
            emoji: Some(event.payload.get("emoji")?.as_str()?.to_string()),
            message_id: Some(id("message_id")?),
        }),
        "channel.member_joined" => Some(Fired {
            kind: "member_joined",
            channel_id: id("channel_id")?,
            user_id: id("user_id")?,
            emoji: None,
            message_id: None,
        }),
        _ => None,
    }
}

pub async fn handle(state: &AppState, event: &Event) -> sqlx::Result<usize> {
    let Some(fired) = fired(event) else {
        return Ok(0);
    };
    let listeners = state
        .workflow_repo
        .listening(fired.kind, fired.channel_id, fired.emoji.as_deref())
        .await?;
    let context = serde_json::json!({
        "trigger": {
            "user_id": fired.user_id,
            "channel_id": fired.channel_id,
            "message_id": fired.message_id,
        }
    });
    let key = format!("event:{}", event.id);
    let mut conn = state.redis.clone();
    for workflow in &listeners {
        let within_limit = rate_limit::enforce(
            &mut conn,
            &format!("rate_limit:workflow:{}", workflow.id),
            RUNS_PER_MINUTE,
            60,
            LimiterFailure::Open,
        )
        .await
        .is_ok();
        if within_limit {
            state
                .workflow_repo
                .queue_run(workflow.id, workflow.version, &key, &context)
                .await?;
        } else {
            state
                .workflow_repo
                .skip_run(
                    workflow.id,
                    workflow.version,
                    &key,
                    &context,
                    &format!("Skipped: it started more than {RUNS_PER_MINUTE} times in a minute"),
                )
                .await?;
        }
    }
    Ok(listeners.len())
}
