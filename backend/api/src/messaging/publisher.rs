use redis::AsyncCommands;
use tracing::{info, warn};
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};
use shared_events::Event;

/// The replay buffer is bounded by both length and age: it exists so a client
/// that missed a minute can catch up, not to store history. The database is the
/// source of truth, and past the window the client refetches instead.
pub const STREAM_MAXLEN: usize = 10_000;

pub fn workspace_stream(workspace_id: Uuid) -> String {
    format!("stream:ws:{workspace_id}")
}

/// Events whose absence a client can still notice minutes later, so a gap in
/// them is worth replaying. Typing, presence and WebRTC signalling are
/// deliberately not here: replaying a typing indicator from five minutes ago is
/// not recovery, it is a bug, and a late ICE candidate is worse than none.
fn is_durable(event_type: &str) -> bool {
    matches!(
        event_type.split('.').next(),
        Some("message" | "reaction" | "workspace" | "conversation")
    )
}

pub struct EventPublisher {
    redis: redis::aio::ConnectionManager,
}

impl EventPublisher {
    pub fn new(redis: redis::aio::ConnectionManager) -> Self {
        Self { redis }
    }

    pub async fn publish(&self, event_type: &str, payload: serde_json::Value) -> AppResult<()> {
        self.emit(event_type, payload, None).await
    }

    /// A durable event goes to its workspace's stream first and then out over
    /// pub/sub carrying the stream id it was given.
    ///
    /// The stream is the log and pub/sub is the live tail, rather than the
    /// gateway reading the tail from the stream itself. The seam between
    /// replaying a gap and joining the live tail is the part of that design most
    /// likely to drop or duplicate an event, and it would have to be got right
    /// per connection; here the replay simply overlaps the tail and the client
    /// discards what it has already applied — which it must do anyway, because
    /// delivery is at-least-once by design.
    pub async fn publish_scoped(
        &self,
        event_type: &str,
        workspace_id: Uuid,
        payload: serde_json::Value,
    ) -> AppResult<()> {
        self.emit(event_type, payload, Some(workspace_id)).await
    }

    async fn emit(
        &self,
        event_type: &str,
        payload: serde_json::Value,
        workspace_id: Option<Uuid>,
    ) -> AppResult<()> {
        let mut event = Event::new(event_type, payload);
        let mut conn = self.redis.clone();

        if let Some(workspace_id) = workspace_id.filter(|_| is_durable(event_type)) {
            let body = serde_json::to_string(&event)
                .map_err(|e| AppError::Internal(format!("Event serialize failed: {e}")))?;
            let key = workspace_stream(workspace_id);
            let _: redis::RedisResult<()> = redis::cmd("SADD")
                .arg(crate::messaging::stream_group::STREAM_INDEX_KEY)
                .arg(&key)
                .query_async(&mut conn)
                .await;

            let appended: redis::RedisResult<String> = redis::cmd("XADD")
                .arg(&key)
                .arg("MAXLEN")
                .arg("~")
                .arg(STREAM_MAXLEN)
                .arg("*")
                .arg("event")
                .arg(&body)
                .query_async(&mut conn)
                .await;

            match appended {
                // The id is what the client stores and sends back on reconnect,
                // so it travels with the live copy too.
                Ok(id) => event.stream_id = Some(id),
                Err(e) => warn!(
                    "Redis XADD failed for event {} (id={}): {}",
                    event_type, event.id, e
                ),
            }
        }

        let json = serde_json::to_string(&event)
            .map_err(|e| AppError::Internal(format!("Event serialize failed: {e}")))?;

        let channel = format!(
            "events:{}",
            event_type.split('.').next().unwrap_or("general")
        );

        if let Err(e) = conn.publish::<_, _, ()>(&channel, &json).await {
            warn!(
                "Redis publish failed for event {} (id={}): {}",
                event_type, event.id, e
            );
            return Err(AppError::Internal(format!("Redis publish failed: {e}")));
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
        self.publish_scoped("message.created", workspace_id, payload)
            .await
    }

    pub async fn publish_message_updated(
        &self,
        message: &serde_json::Value,
        workspace_id: Uuid,
    ) -> AppResult<()> {
        self.publish_scoped("message.updated", workspace_id, message.clone())
            .await
    }

    pub async fn publish_message_deleted(
        &self,
        message_id: Uuid,
        channel_id: Uuid,
        workspace_id: Uuid,
    ) -> AppResult<()> {
        self.publish_scoped(
            "message.deleted",
            workspace_id,
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
        workspace_id: Uuid,
    ) -> AppResult<()> {
        let mut payload = reaction.clone();
        if let Some(obj) = payload.as_object_mut() {
            obj.insert("channel_id".into(), serde_json::json!(channel_id));
        }
        self.publish_scoped("reaction.added", workspace_id, payload)
            .await
    }

    pub async fn publish_reaction_removed(
        &self,
        message_id: Uuid,
        channel_id: Uuid,
        workspace_id: Uuid,
        user_id: Uuid,
        emoji: &str,
    ) -> AppResult<()> {
        self.publish_scoped(
            "reaction.removed",
            workspace_id,
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
        self.publish_scoped(
            "workspace.deleted",
            workspace_id,
            serde_json::json!({
                "workspace_id": workspace_id,
                "delete_type": delete_type,
            }),
        )
        .await
    }

    pub async fn publish_workspace_restored(&self, workspace_id: Uuid) -> AppResult<()> {
        self.publish_scoped(
            "workspace.restored",
            workspace_id,
            serde_json::json!({ "workspace_id": workspace_id }),
        )
        .await
    }

    pub async fn publish_workspace_member_removed(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> AppResult<()> {
        self.publish_scoped(
            "workspace.member_removed",
            workspace_id,
            serde_json::json!({ "workspace_id": workspace_id, "user_id": user_id }),
        )
        .await
    }

    pub async fn publish_channel_member_removed(
        &self,
        channel_id: Uuid,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> AppResult<()> {
        self.publish_scoped(
            "channel.member_removed",
            workspace_id,
            serde_json::json!({
                "channel_id": channel_id,
                "workspace_id": workspace_id,
                "user_id": user_id,
            }),
        )
        .await
    }

    pub async fn publish_message_pinned(
        &self,
        message_id: Uuid,
        channel_id: Uuid,
        workspace_id: Uuid,
        pinned: bool,
    ) -> AppResult<()> {
        self.publish_scoped(
            "message.pinned",
            workspace_id,
            serde_json::json!({
                "message_id": message_id,
                "channel_id": channel_id,
                "pinned": pinned,
            }),
        )
        .await
    }
}
