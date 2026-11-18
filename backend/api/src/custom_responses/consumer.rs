use std::sync::Arc;

use tracing::{info, warn};
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};
use shared_events::Event;

use super::matcher::{self, Matchable};
use super::service;
use crate::authz;
use crate::messaging::stream_group::StreamGroup;
use crate::state::AppState;
use crate::workspace::models::ChannelType;

pub async fn start_consumer(state: Arc<AppState>) {
    let Some(mut group) = StreamGroup::connect(&state.config.redis_url, "custom_responses").await
    else {
        return;
    };
    info!("Custom responses consumer started");
    loop {
        for delivery in group.next_batch().await {
            if let Err(e) = handle(&state, &delivery.event).await {
                warn!(event_id = %delivery.event.id, "could not answer a message: {e}");
            }
            group.ack(&delivery.key, &delivery.id).await;
        }
    }
}

pub async fn handle(state: &AppState, event: &Event) -> AppResult<Option<Uuid>> {
    if event.event_type != "message.created" {
        return Ok(None);
    }
    let Some(message_id) = event
        .payload
        .get("id")
        .and_then(|v| v.as_str())
        .and_then(|v| v.parse::<Uuid>().ok())
    else {
        return Ok(None);
    };
    let Some(heard) = state.custom_response_repo.heard(message_id).await? else {
        return Ok(None);
    };
    if heard.automated {
        return Ok(None);
    }
    let responses = state.custom_response_repo.list(heard.workspace_id).await?;
    let matchable: Vec<Matchable<'_>> = responses
        .iter()
        .map(|response| Matchable {
            id: response.id,
            triggers: &response.triggers,
        })
        .collect();
    let ranked = matcher::ranked(&heard.content, &matchable);
    if ranked.is_empty() {
        return Ok(None);
    }
    let cooling = state.custom_response_repo.cooling(heard.channel_id).await?;
    let Some(response_id) = ranked.into_iter().find(|id| !cooling.contains(id)) else {
        return Ok(None);
    };

    let access = match authz::require_channel_post(state, heard.channel_id, heard.author_id).await {
        Ok(access) => access,
        Err(AppError::Forbidden(_) | AppError::NotFound(_)) => return Ok(None),
        Err(e) => return Err(e),
    };
    if matches!(
        access.channel.channel_type,
        ChannelType::Dm | ChannelType::GroupDm
    ) {
        return Ok(None);
    }
    service::answer(state, &heard, response_id).await
}
