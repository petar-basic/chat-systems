use std::sync::Arc;

use redis::AsyncCommands;
use sha2::{Digest, Sha256};
use tracing::{info, warn};
use uuid::Uuid;

use super::{fetch, links};
use crate::messaging::models::Unfurl;
use crate::messaging::stream_group::{Delivery, StreamGroup};
use crate::state::AppState;

const CACHE_SECONDS: u64 = 24 * 60 * 60;

pub async fn start_unfurl_consumer(redis_url: &str, state: Arc<AppState>) {
    let Some(mut group) = StreamGroup::connect(redis_url, "unfurls").await else {
        return;
    };
    info!("Unfurl consumer started");
    loop {
        for delivery in group.next_batch().await {
            unfurl_delivery(&state, &delivery).await;
            group.ack(&delivery.key, &delivery.id).await;
        }
    }
}

async fn unfurl_delivery(state: &AppState, delivery: &Delivery) {
    if !matches!(
        delivery.event.event_type.as_str(),
        "message.created" | "message.updated"
    ) {
        return;
    }
    let Some(id) = delivery
        .event
        .payload
        .get("id")
        .and_then(|v| v.as_str())
        .and_then(|v| v.parse::<Uuid>().ok())
    else {
        return;
    };

    let target = match state.message_repo.find_unfurl_target(id).await {
        Ok(Some(target)) => target,
        Ok(None) => return,
        Err(e) => {
            warn!(message_id = %id, "unfurl: failed to load the message: {e}");
            return;
        }
    };
    let hidden = target
        .metadata
        .get("unfurls_hidden")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !target.previews_enabled || hidden {
        return;
    }

    let urls = links::extract(&target.content);
    let source = source_of(&urls);
    let current = target
        .metadata
        .get("unfurls_source")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if current == source {
        return;
    }

    let mut unfurls = Vec::new();
    for url in &urls {
        if let Some(unfurl) = cached_or_fetched(state, url).await {
            unfurls.push(unfurl);
        }
    }

    if let Err(e) = store(
        state,
        id,
        &target.content,
        &source,
        &unfurls,
        target.workspace_id,
    )
    .await
    {
        warn!(message_id = %id, "unfurl: failed to store previews: {e}");
    }
}

async fn store(
    state: &AppState,
    id: Uuid,
    content: &str,
    source: &str,
    unfurls: &[Unfurl],
    workspace_id: Uuid,
) -> shared_common::errors::AppResult<()> {
    let unfurls = serde_json::to_value(unfurls)
        .map_err(|e| shared_common::errors::AppError::Internal(e.to_string()))?;
    let mut tx = state.pool.begin().await?;
    let Some(message) = state
        .message_repo
        .store_unfurls_in(&mut tx, id, content, source, &unfurls)
        .await?
    else {
        return Ok(());
    };
    let staged = state
        .publisher
        .stage_message_updated(&mut tx, &message, workspace_id)
        .await?;
    tx.commit().await?;
    state.publisher.dispatch(staged).await;
    Ok(())
}

async fn cached_or_fetched(state: &AppState, url: &str) -> Option<Unfurl> {
    let key = format!("unfurl:v1:{}", hex(&Sha256::digest(url.as_bytes())));
    let mut redis = state.redis.clone();
    let cached: Option<String> = redis.get(&key).await.unwrap_or_default();
    if let Some(cached) = cached {
        if let Ok(unfurl) = serde_json::from_str::<Option<Unfurl>>(&cached) {
            return unfurl;
        }
    }

    let unfurl = fetch::fetch(url, false).await;
    if let Ok(encoded) = serde_json::to_string(&unfurl) {
        let stored: redis::RedisResult<()> = redis.set_ex(&key, encoded, CACHE_SECONDS).await;
        if let Err(e) = stored {
            warn!("unfurl: failed to cache a preview: {e}");
        }
    }
    unfurl
}

fn source_of(urls: &[String]) -> String {
    if urls.is_empty() {
        return String::new();
    }
    hex(&Sha256::digest(urls.join("\n").as_bytes()))[..32].to_string()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
