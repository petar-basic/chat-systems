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
            r#"
            INSERT INTO hooks (workspace_id, created_by, hook_type, name, description, config)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING *
            "#,
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

    pub async fn list_active_outgoing_hooks(&self, workspace_id: Uuid) -> sqlx::Result<Vec<Hook>> {
        sqlx::query_as::<_, Hook>(
            "SELECT * FROM hooks WHERE workspace_id = $1 AND hook_type = 'outgoing_webhook' AND is_active = true",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await
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
            r#"
            INSERT INTO hook_executions (hook_id, event_type, payload, response_status, response_body)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            "#,
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
            r#"
            INSERT INTO reminders (workspace_id, created_by, target_user_id, channel_id, message_id, content, remind_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING *
            "#,
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

    pub async fn get_due_reminders(&self) -> sqlx::Result<Vec<Reminder>> {
        sqlx::query_as::<_, Reminder>(
            "SELECT * FROM reminders WHERE remind_at <= NOW() AND is_delivered = false ORDER BY remind_at",
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn mark_reminder_delivered(&self, id: Uuid) -> sqlx::Result<()> {
        sqlx::query("UPDATE reminders SET is_delivered = true WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
