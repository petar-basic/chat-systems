use redis::AsyncCommands;
use sqlx::{PgConnection, PgPool};
use tracing::{info, warn};
use uuid::Uuid;

use super::models::{Message, Reaction};

use shared_common::errors::{AppError, AppResult};
use shared_events::Event;

/// The replay buffer is bounded by both length and age: it exists so a client
/// that missed a minute can catch up, not to store history. The database is the
/// source of truth, and past the window the client refetches instead.
pub use shared_events::STREAM_MAXLEN;

pub use shared_events::workspace_stream;

/// Events whose absence a client can still notice minutes later, so a gap in
/// them is worth replaying. Typing, presence and WebRTC signalling are
/// deliberately not here: replaying a typing indicator from five minutes ago is
/// not recovery, it is a bug, and a late ICE candidate is worse than none.
fn is_durable(event_type: &str) -> bool {
    // Huddle *membership* is history: it is written to a table and read back as
    // a call log, so it belongs in the log with the other durable events. The
    // ring and the WebRTC signalling stay ephemeral — replaying either one
    // minutes later is worse than not replaying it.
    if matches!(
        event_type,
        "huddle.member_joined" | "huddle.member_left" | "huddle.ring"
    ) {
        return true;
    }

    matches!(
        event_type.split('.').next(),
        Some("message" | "reaction" | "workspace" | "conversation")
    )
}

pub struct EventPublisher {
    redis: redis::aio::ConnectionManager,
    pool: PgPool,
}

/// An event written to the outbox inside the caller's transaction. Handing it
/// to `dispatch` after the commit is the fast path; the worker's relay is the
/// slow one, for whatever the fast path did not manage.
pub struct Staged {
    row_id: i64,
    event: Event,
    workspace_id: Uuid,
}

impl Staged {
    pub fn event_id(&self) -> Uuid {
        self.event.id
    }
}

impl EventPublisher {
    pub fn new(redis: redis::aio::ConnectionManager, pool: PgPool) -> Self {
        Self { redis, pool }
    }

    pub async fn publish(&self, event_type: &str, payload: serde_json::Value) -> AppResult<()> {
        let mut event = Event::new(event_type, payload);
        self.emit(&mut event, None).await
    }

    /// A durable event goes through the outbox in a transaction of its own, so
    /// it is delivered at least once even when the caller has no transaction to
    /// stage it in. Callers that do have one use `stage` and `dispatch` instead,
    /// which makes the event atomic with the row it describes.
    pub async fn publish_scoped(
        &self,
        event_type: &str,
        workspace_id: Uuid,
        payload: serde_json::Value,
    ) -> AppResult<()> {
        if !is_durable(event_type) {
            let mut event = Event::new(event_type, payload);
            return self.emit(&mut event, Some(workspace_id)).await;
        }
        let mut tx = self.pool.begin().await?;
        let staged = self
            .stage(&mut tx, event_type, workspace_id, payload)
            .await?;
        tx.commit().await?;
        self.dispatch(staged).await;
        Ok(())
    }

    pub async fn stage(
        &self,
        conn: &mut PgConnection,
        event_type: &str,
        workspace_id: Uuid,
        payload: serde_json::Value,
    ) -> AppResult<Staged> {
        let event = Event::new(event_type, payload);
        let row_id = sqlx::query_scalar!(
            "INSERT INTO event_outbox (event_id, event_type, workspace_id, payload, created_at)
             VALUES ($1, $2, $3, $4, $5)
             RETURNING id",
            event.id,
            &event.event_type,
            workspace_id,
            &event.payload,
            event.timestamp
        )
        .fetch_one(conn)
        .await?;
        Ok(Staged {
            row_id,
            event,
            workspace_id,
        })
    }

    pub async fn dispatch(&self, staged: Staged) {
        let Staged {
            row_id,
            mut event,
            workspace_id,
        } = staged;
        match self.emit(&mut event, Some(workspace_id)).await {
            Ok(()) => self.mark_published(row_id).await,
            Err(e) => warn!(
                "event {} (id={}) stays in the outbox for the relay: {}",
                event.event_type, event.id, e
            ),
        }
    }

    pub async fn dispatch_all(&self, staged: Vec<Staged>) {
        for event in staged {
            self.dispatch(event).await;
        }
    }

    pub(crate) async fn mark_published(&self, row_id: i64) {
        let marked = sqlx::query!(
            "UPDATE event_outbox SET published_at = NOW() WHERE id = $1",
            row_id
        )
        .execute(&self.pool)
        .await;
        if let Err(e) = marked {
            warn!("could not mark outbox row {} as published: {}", row_id, e);
        }
    }

    /// The stream is the log and pub/sub is the live tail, rather than the
    /// gateway reading the tail from the stream itself. The seam between
    /// replaying a gap and joining the live tail is the part of that design most
    /// likely to drop or duplicate an event, and it would have to be got right
    /// per connection; here the replay simply overlaps the tail and the client
    /// discards what it has already applied — which it must do anyway, because
    /// delivery is at-least-once by design.
    pub(crate) async fn emit(
        &self,
        event: &mut Event,
        workspace_id: Option<Uuid>,
    ) -> AppResult<()> {
        let mut conn = self.redis.clone();

        if let Some(workspace_id) = workspace_id.filter(|_| is_durable(&event.event_type)) {
            let body = serde_json::to_string(&event)
                .map_err(|e| AppError::Internal(format!("Event serialize failed: {e}")))?;
            let key = workspace_stream(workspace_id);
            let _: redis::RedisResult<()> = redis::cmd("SADD")
                .arg(crate::messaging::stream_group::STREAM_INDEX_KEY)
                .arg(&key)
                .query_async(&mut conn)
                .await;

            let appended: String = redis::cmd("XADD")
                .arg(&key)
                .arg("MAXLEN")
                .arg("~")
                .arg(STREAM_MAXLEN)
                .arg("*")
                .arg("event")
                .arg(&body)
                .query_async(&mut conn)
                .await
                .map_err(|e| AppError::Internal(format!("Redis XADD failed: {e}")))?;
            // The id is what the client stores and sends back on reconnect,
            // so it travels with the live copy too.
            event.stream_id = Some(appended);
        }

        let json = serde_json::to_string(&event)
            .map_err(|e| AppError::Internal(format!("Event serialize failed: {e}")))?;

        let channel = format!(
            "events:{}",
            event.event_type.split('.').next().unwrap_or("general")
        );

        if let Err(e) = conn.publish::<_, _, ()>(&channel, &json).await {
            warn!(
                "Redis publish failed for event {} (id={}): {}",
                event.event_type, event.id, e
            );
            return Err(AppError::Internal(format!("Redis publish failed: {e}")));
        }

        info!("Published event: {} (id={})", event.event_type, event.id);
        Ok(())
    }

    fn message_created_payload(
        message: &serde_json::Value,
        workspace_id: Uuid,
        mentioned_user_ids: &[Uuid],
    ) -> serde_json::Value {
        let mut payload = message.clone();
        if let Some(obj) = payload.as_object_mut() {
            obj.insert("workspace_id".into(), serde_json::json!(workspace_id));
            obj.insert(
                "mentioned_user_ids".into(),
                serde_json::json!(mentioned_user_ids),
            );
        }
        payload
    }

    pub async fn stage_message_created(
        &self,
        conn: &mut PgConnection,
        message: &Message,
        workspace_id: Uuid,
        mentioned_user_ids: &[Uuid],
    ) -> AppResult<Staged> {
        let message =
            serde_json::to_value(message).map_err(|e| AppError::Internal(e.to_string()))?;
        let payload = Self::message_created_payload(&message, workspace_id, mentioned_user_ids);
        self.stage(conn, "message.created", workspace_id, payload)
            .await
    }

    pub async fn stage_message_updated(
        &self,
        conn: &mut PgConnection,
        message: &Message,
        workspace_id: Uuid,
    ) -> AppResult<Staged> {
        let payload =
            serde_json::to_value(message).map_err(|e| AppError::Internal(e.to_string()))?;
        self.stage(conn, "message.updated", workspace_id, payload)
            .await
    }

    pub async fn stage_message_deleted(
        &self,
        conn: &mut PgConnection,
        message_id: Uuid,
        channel_id: Uuid,
        workspace_id: Uuid,
    ) -> AppResult<Staged> {
        self.stage(
            conn,
            "message.deleted",
            workspace_id,
            serde_json::json!({ "message_id": message_id, "channel_id": channel_id }),
        )
        .await
    }

    pub async fn stage_message_pinned(
        &self,
        conn: &mut PgConnection,
        message_id: Uuid,
        channel_id: Uuid,
        workspace_id: Uuid,
        pinned: bool,
    ) -> AppResult<Staged> {
        self.stage(
            conn,
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

    pub async fn stage_reaction_added(
        &self,
        conn: &mut PgConnection,
        reaction: &Reaction,
        channel_id: Uuid,
        workspace_id: Uuid,
    ) -> AppResult<Staged> {
        let mut payload =
            serde_json::to_value(reaction).map_err(|e| AppError::Internal(e.to_string()))?;
        if let Some(obj) = payload.as_object_mut() {
            obj.insert("channel_id".into(), serde_json::json!(channel_id));
        }
        self.stage(conn, "reaction.added", workspace_id, payload)
            .await
    }

    pub async fn stage_reaction_removed(
        &self,
        conn: &mut PgConnection,
        message_id: Uuid,
        channel_id: Uuid,
        workspace_id: Uuid,
        user_id: Uuid,
        emoji: &str,
    ) -> AppResult<Staged> {
        self.stage(
            conn,
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

    pub async fn stage_workspace_deleted(
        &self,
        conn: &mut PgConnection,
        workspace_id: Uuid,
        delete_type: &str,
    ) -> AppResult<Staged> {
        self.stage(
            conn,
            "workspace.deleted",
            workspace_id,
            serde_json::json!({ "workspace_id": workspace_id, "delete_type": delete_type }),
        )
        .await
    }

    pub async fn stage_workspace_restored(
        &self,
        conn: &mut PgConnection,
        workspace_id: Uuid,
    ) -> AppResult<Staged> {
        self.stage(
            conn,
            "workspace.restored",
            workspace_id,
            serde_json::json!({ "workspace_id": workspace_id }),
        )
        .await
    }

    pub async fn stage_workspace_member_removed(
        &self,
        conn: &mut PgConnection,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> AppResult<Staged> {
        self.stage(
            conn,
            "workspace.member_removed",
            workspace_id,
            serde_json::json!({ "workspace_id": workspace_id, "user_id": user_id }),
        )
        .await
    }

    pub async fn stage_channel_member_removed(
        &self,
        conn: &mut PgConnection,
        channel_id: Uuid,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> AppResult<Staged> {
        self.stage(
            conn,
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

    pub async fn publish_message_created(
        &self,
        message: &serde_json::Value,
        workspace_id: Uuid,
        mentioned_user_ids: &[Uuid],
    ) -> AppResult<()> {
        let payload = Self::message_created_payload(message, workspace_id, mentioned_user_ids);
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
