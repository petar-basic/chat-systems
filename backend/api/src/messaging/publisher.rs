use redis::AsyncCommands;
use tracing::{info, warn};
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};
use shared_events::Event;

pub struct EventPublisher {
    redis: redis::aio::ConnectionManager,
}

impl EventPublisher {
    pub fn new(redis: redis::aio::ConnectionManager) -> Self {
        Self { redis }
    }

    pub async fn publish(&self, event_type: &str, payload: serde_json::Value) -> AppResult<()> {
        let event = Event::new(event_type, payload);
        let json = serde_json::to_string(&event)
            .map_err(|e| AppError::Internal(format!("Event serialize failed: {}", e)))?;

        let channel = format!(
            "events:{}",
            event_type.split('.').next().unwrap_or("general")
        );

        let mut conn = self.redis.clone();
        if let Err(e) = conn.publish::<_, _, ()>(&channel, &json).await {
            warn!(
                "Redis publish failed for event {} (id={}): {}",
                event_type, event.id, e
            );
            return Err(AppError::Internal(format!("Redis publish failed: {}", e)));
        }

        info!("Published event: {} (id={})", event_type, event.id);
        Ok(())
    }

    pub async fn publish_message_created(
        &self,
        message: &serde_json::Value,
        workspace_id: Uuid,
        mentioned_user_ids: &[Uuid],
    ) -> AppResult<()> {
        let mut payload = message.clone();
        if let Some(obj) = payload.as_object_mut() {
            obj.insert("workspace_id".into(), serde_json::json!(workspace_id));
            obj.insert(
                "mentioned_user_ids".into(),
                serde_json::json!(mentioned_user_ids),
            );
        }
        self.publish("message.created", payload).await
    }

    pub async fn publish_message_updated(&self, message: &serde_json::Value) -> AppResult<()> {
        self.publish("message.updated", message.clone()).await
    }

    pub async fn publish_message_deleted(
        &self,
        message_id: Uuid,
        channel_id: Uuid,
    ) -> AppResult<()> {
        self.publish(
            "message.deleted",
            serde_json::json!({
                "message_id": message_id,
                "channel_id": channel_id,
            }),
        )
        .await
    }

    pub async fn publish_reaction_added(
        &self,
        reaction: &serde_json::Value,
        channel_id: Uuid,
    ) -> AppResult<()> {
        let mut payload = reaction.clone();
        if let Some(obj) = payload.as_object_mut() {
            obj.insert("channel_id".into(), serde_json::json!(channel_id));
        }
        self.publish("reaction.added", payload).await
    }

    pub async fn publish_reaction_removed(
        &self,
        message_id: Uuid,
        channel_id: Uuid,
        user_id: Uuid,
        emoji: &str,
    ) -> AppResult<()> {
        self.publish(
            "reaction.removed",
            serde_json::json!({
                "message_id": message_id,
                "channel_id": channel_id,
                "user_id": user_id,
                "emoji": emoji,
            }),
        )
        .await
    }

    pub async fn publish_workspace_deleted(
        &self,
        workspace_id: Uuid,
        delete_type: &str,
    ) -> AppResult<()> {
        self.publish(
            "workspace.deleted",
            serde_json::json!({
                "workspace_id": workspace_id,
                "delete_type": delete_type,
            }),
        )
        .await
    }

    pub async fn publish_workspace_restored(&self, workspace_id: Uuid) -> AppResult<()> {
        self.publish(
            "workspace.restored",
            serde_json::json!({ "workspace_id": workspace_id }),
        )
        .await
    }

    pub async fn publish_message_pinned(
        &self,
        message_id: Uuid,
        channel_id: Uuid,
        pinned: bool,
    ) -> AppResult<()> {
        self.publish(
            "message.pinned",
            serde_json::json!({
                "message_id": message_id,
                "channel_id": channel_id,
                "pinned": pinned,
            }),
        )
        .await
    }
}
