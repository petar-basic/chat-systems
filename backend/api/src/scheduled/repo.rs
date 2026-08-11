use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::models::ScheduledMessage;

#[derive(Clone)]
pub struct ScheduledRepo {
    pool: PgPool,
}

pub struct NewScheduledMessage<'a> {
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub channel_id: Option<Uuid>,
    pub conversation_id: Option<Uuid>,
    pub content: &'a str,
    pub send_at: DateTime<Utc>,
}

impl ScheduledRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, msg: NewScheduledMessage<'_>) -> sqlx::Result<ScheduledMessage> {
        sqlx::query_as::<_, ScheduledMessage>(
            r"
            INSERT INTO scheduled_messages (workspace_id, user_id, channel_id, conversation_id, content, send_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING *
            ",
        )
        .bind(msg.workspace_id)
        .bind(msg.user_id)
        .bind(msg.channel_id)
        .bind(msg.conversation_id)
        .bind(msg.content)
        .bind(msg.send_at)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn find_by_id(&self, id: Uuid) -> sqlx::Result<Option<ScheduledMessage>> {
        sqlx::query_as::<_, ScheduledMessage>("SELECT * FROM scheduled_messages WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn list_pending_for_user(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<Vec<ScheduledMessage>> {
        sqlx::query_as::<_, ScheduledMessage>(
            r"
            SELECT * FROM scheduled_messages
            WHERE workspace_id = $1 AND user_id = $2 AND canceled_at IS NULL
              AND (sent_at IS NULL OR failure IS NOT NULL)
            ORDER BY send_at
            ",
        )
        .bind(workspace_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn reschedule(
        &self,
        id: Uuid,
        send_at: DateTime<Utc>,
    ) -> sqlx::Result<ScheduledMessage> {
        sqlx::query_as::<_, ScheduledMessage>(
            "UPDATE scheduled_messages SET send_at = $2 WHERE id = $1 RETURNING *",
        )
        .bind(id)
        .bind(send_at)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn cancel(&self, id: Uuid) -> sqlx::Result<()> {
        sqlx::query("UPDATE scheduled_messages SET canceled_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Claims every message whose time has come, marking them sent in the same statement so
    /// two api replicas running the dispatcher cannot deliver the same row twice.
    pub async fn claim_due(&self) -> sqlx::Result<Vec<ScheduledMessage>> {
        sqlx::query_as::<_, ScheduledMessage>(
            r"
            UPDATE scheduled_messages
            SET sent_at = NOW()
            WHERE id IN (
                SELECT id FROM scheduled_messages
                WHERE sent_at IS NULL AND canceled_at IS NULL AND send_at <= NOW()
                ORDER BY send_at
                FOR UPDATE SKIP LOCKED
                LIMIT 100
            )
            RETURNING *
            ",
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Losing access to a channel should not leave a message queued for it. The
    /// delivery-time check is the backstop; this is so the author is told now
    /// rather than at send time, when they may have forgotten writing it.
    pub async fn cancel_pending_for_channel(
        &self,
        channel_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<u64> {
        let result = sqlx::query(
            "UPDATE scheduled_messages SET canceled_at = NOW() \
              WHERE channel_id = $1 AND user_id = $2 \
                AND sent_at IS NULL AND canceled_at IS NULL",
        )
        .bind(channel_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn cancel_pending_for_workspace(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<u64> {
        let result = sqlx::query(
            "UPDATE scheduled_messages SET canceled_at = NOW() \
              WHERE workspace_id = $1 AND user_id = $2 \
                AND sent_at IS NULL AND canceled_at IS NULL",
        )
        .bind(workspace_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn record_failure(&self, id: Uuid, failure: &str) -> sqlx::Result<()> {
        sqlx::query("UPDATE scheduled_messages SET failure = $2 WHERE id = $1")
            .bind(id)
            .bind(failure)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
