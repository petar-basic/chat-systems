use std::sync::Arc;

use tracing::{info, warn};

use shared_events::Event;

use super::repo::NewDelivery;
use super::{envelope, event_id, hears, translate, EVENT_TTL_SECS, KIND_EVENT};
use crate::messaging::stream_group::StreamGroup;
use crate::state::AppState;

pub async fn start_consumer(redis_url: &str, state: Arc<AppState>) {
    let Some(mut group) = StreamGroup::connect(redis_url, "app_events").await else {
        return;
    };
    info!("App event consumer started");
    loop {
        for delivery in group.next_batch().await {
            if fan_out(&state, &delivery.event).await {
                group.ack(&delivery.key, &delivery.id).await;
            }
        }
    }
}

pub async fn fan_out(state: &AppState, event: &Event) -> bool {
    let Some(translated) = translate(event) else {
        return true;
    };
    let subscribers = match state
        .app_event_repo
        .subscribers(translated.workspace_id, translated.channel_id)
        .await
    {
        Ok(subscribers) => subscribers,
        Err(e) => {
            warn!(event_id = %event.id, "App events: failed to list subscribers: {e}");
            return false;
        }
    };
    for subscriber in &subscribers {
        for (app_event, body) in &translated.events {
            if !hears(subscriber, &translated, *app_event) {
                continue;
            }
            let envelope = envelope(event, translated.workspace_id, *app_event, body);
            let queued = state
                .app_event_repo
                .enqueue(NewDelivery {
                    app_id: subscriber.id,
                    event_id: event_id(event.id, *app_event),
                    channel_id: translated.channel_id,
                    kind: KIND_EVENT,
                    body: &envelope,
                    ttl_secs: EVENT_TTL_SECS,
                })
                .await;
            if let Err(e) = queued {
                warn!(app_id = %subscriber.id, "App events: failed to queue an event: {e}");
                return false;
            }
        }
    }
    true
}
