use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::models::*;

#[derive(Clone)]
pub struct HookRepo {
    pool: PgPool,
}

pub struct NewReminder<'a> {
    pub workspace_id: Uuid,
    pub created_by: Uuid,
    pub target_user_id: Uuid,
    pub channel_id: Option<Uuid>,
    pub message_id: Option<Uuid>,
    pub content: &'a str,
    pub remind_at: DateTime<Utc>,
}

impl HookRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_hook(
        &self,
        workspace_id: Uuid,
        created_by: Uuid,
        hook_type: &HookType,
        name: &str,
        description: Option<&str>,
        config: &serde_json::Value,
    ) -> sqlx::Result<Hook> {
        sqlx::query_as::<_, Hook>(
            r"
            INSERT INTO hooks (workspace_id, created_by, hook_type, name, description, config)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING *
            ",
        )
        .bind(workspace_id)
        .bind(created_by)
        .bind(hook_type)
        .bind(name)
        .bind(description)
        .bind(config)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn find_hook_by_id(&self, id: Uuid) -> sqlx::Result<Option<Hook>> {
        sqlx::query_as::<_, Hook>("SELECT * FROM hooks WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn list_hooks(&self, workspace_id: Uuid) -> sqlx::Result<Vec<Hook>> {
        sqlx::query_as::<_, Hook>(
            "SELECT * FROM hooks WHERE workspace_id = $1 ORDER BY created_at DESC",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn update_hook_config(
        &self,
        id: Uuid,
        config: &serde_json::Value,
    ) -> sqlx::Result<Hook> {
        sqlx::query_as::<_, Hook>(
            "UPDATE hooks SET config = $2, updated_at = NOW() WHERE id = $1 RETURNING *",
        )
        .bind(id)
        .bind(config)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn delete_hook(&self, id: Uuid) -> sqlx::Result<()> {
        sqlx::query("DELETE FROM hooks WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn find_active_incoming_hook_by_token(
        &self,
        token: &str,
    ) -> sqlx::Result<Option<Hook>> {
        sqlx::query_as::<_, Hook>(
            "SELECT * FROM hooks WHERE hook_type = 'incoming_webhook' AND is_active = true AND config->>'token' = $1",
        )
        .bind(token)
        .fetch_optional(&self.pool)
        .await
    }

    /// Scoped by the channel the message was posted in. A workspace-wide
    /// outgoing webhook is a copy of every private conversation in the
    /// workspace being posted to a third-party URL.
    pub async fn list_active_outgoing_hooks_for_channel(
        &self,
        workspace_id: Uuid,
        channel_id: Uuid,
    ) -> sqlx::Result<Vec<Hook>> {
        sqlx::query_as::<_, Hook>(
            "SELECT * FROM hooks \
              WHERE workspace_id = $1 \
                AND hook_type = 'outgoing_webhook' \
                AND is_active = true \
                AND config->'channel_ids' ? $2::text",
        )
        .bind(workspace_id)
        .bind(channel_id.to_string())
        .fetch_all(&self.pool)
        .await
    }

    /// One registered command per name per workspace: `/deploy` has to mean one
    /// thing, or invoking it is a coin flip.
    pub async fn find_slash_command(
        &self,
        workspace_id: Uuid,
        command: &str,
    ) -> sqlx::Result<Option<Hook>> {
        sqlx::query_as::<_, Hook>(
            "SELECT * FROM hooks \
              WHERE workspace_id = $1 \
                AND hook_type = 'slash_command' \
                AND is_active = true \
                AND config->>'command' = $2",
        )
        .bind(workspace_id)
        .bind(command)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn list_slash_commands(&self, workspace_id: Uuid) -> sqlx::Result<Vec<Hook>> {
        sqlx::query_as::<_, Hook>(
            "SELECT * FROM hooks \
              WHERE workspace_id = $1 \
                AND hook_type = 'slash_command' \
                AND is_active = true \
              ORDER BY config->>'command'",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn channel_ids_with_outgoing_hooks(
        &self,
        workspace_id: Uuid,
    ) -> sqlx::Result<Vec<String>> {
        sqlx::query_scalar(
            "SELECT DISTINCT jsonb_array_elements_text(config->'channel_ids') \
               FROM hooks \
              WHERE workspace_id = $1 \
                AND hook_type = 'outgoing_webhook' \
                AND is_active = true \
                AND jsonb_typeof(config->'channel_ids') = 'array'",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await
    }

    /// Reserves this (hook, event) pair. Returns false when somebody already
    /// has it, which is how a redelivery is told apart from a first delivery.
    pub async fn claim_execution(&self, hook_id: Uuid, event_id: Uuid) -> sqlx::Result<bool> {
        let claimed = sqlx::query(
            r"
            INSERT INTO hook_executions (hook_id, event_id, event_type)
            VALUES ($1, $2, 'pending')
            ON CONFLICT DO NOTHING
            ",
        )
        .bind(hook_id)
        .bind(event_id)
        .execute(&self.pool)
        .await?;
        Ok(claimed.rows_affected() == 1)
    }

    pub async fn record_execution_result(
        &self,
        hook_id: Uuid,
        event_id: Uuid,
        event_type: &str,
        payload: &serde_json::Value,
        response_status: Option<i32>,
        response_body: Option<&str>,
    ) -> sqlx::Result<()> {
        sqlx::query(
            r"
            UPDATE hook_executions
               SET event_type = $3, payload = $4, response_status = $5, response_body = $6
             WHERE hook_id = $1 AND event_id = $2
            ",
        )
        .bind(hook_id)
        .bind(event_id)
        .bind(event_type)
        .bind(payload)
        .bind(response_status)
        .bind(response_body)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn log_execution(
        &self,
        hook_id: Uuid,
        event_type: &str,
        payload: &serde_json::Value,
        response_status: Option<i32>,
        response_body: Option<&str>,
    ) -> sqlx::Result<HookExecution> {
        sqlx::query_as::<_, HookExecution>(
            r"
            INSERT INTO hook_executions (hook_id, event_type, payload, response_status, response_body)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            ",
        )
        .bind(hook_id)
        .bind(event_type)
        .bind(payload)
        .bind(response_status)
        .bind(response_body)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn create_reminder(&self, reminder: NewReminder<'_>) -> sqlx::Result<Reminder> {
        sqlx::query_as::<_, Reminder>(
            r"
            INSERT INTO reminders (workspace_id, created_by, target_user_id, channel_id, message_id, content, remind_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING *
            ",
        )
        .bind(reminder.workspace_id)
        .bind(reminder.created_by)
        .bind(reminder.target_user_id)
        .bind(reminder.channel_id)
        .bind(reminder.message_id)
        .bind(reminder.content)
        .bind(reminder.remind_at)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn list_reminders(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<Vec<Reminder>> {
        sqlx::query_as::<_, Reminder>(
            "SELECT * FROM reminders WHERE workspace_id = $1 AND target_user_id = $2 ORDER BY remind_at",
        )
        .bind(workspace_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn find_reminder(&self, id: Uuid) -> sqlx::Result<Option<Reminder>> {
        sqlx::query_as::<_, Reminder>("SELECT * FROM reminders WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn delete_reminder(&self, id: Uuid) -> sqlx::Result<()> {
        sqlx::query("DELETE FROM reminders WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// "at 15:00" means 15:00 where the person is, and Postgres already carries
    /// the IANA database that turns that into an instant. A past time means they
    /// meant tomorrow.
    pub async fn resolve_local_time(
        &self,
        timezone: &str,
        day_offset: i32,
        hour: i32,
        minute: i32,
    ) -> sqlx::Result<DateTime<Utc>> {
        sqlx::query_scalar(
            r"
            WITH local AS (
                SELECT date_trunc('day', NOW() AT TIME ZONE $1)
                       + make_interval(days => $2, hours => $3, mins => $4) AS at
            )
            SELECT CASE
                     WHEN (at AT TIME ZONE $1) <= NOW() THEN (at + interval '1 day') AT TIME ZONE $1
                     ELSE at AT TIME ZONE $1
                   END
            FROM local
            ",
        )
        .bind(timezone)
        .bind(day_offset)
        .bind(hour)
        .bind(minute)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn claim_due_reminders(&self) -> sqlx::Result<Vec<Reminder>> {
        sqlx::query_as::<_, Reminder>(
            r"
            UPDATE reminders
            SET is_delivered = true
            WHERE id IN (
                SELECT id FROM reminders
                WHERE remind_at <= NOW() AND is_delivered = false
                ORDER BY remind_at
                FOR UPDATE SKIP LOCKED
                LIMIT 100
            )
            RETURNING *
            ",
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn release_reminder(&self, id: Uuid) -> sqlx::Result<()> {
        sqlx::query("UPDATE reminders SET is_delivered = false WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
