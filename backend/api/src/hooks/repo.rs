use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::models::*;

pub struct NewHook<'a> {
    pub workspace_id: Uuid,
    pub created_by: Uuid,
    pub hook_type: &'a HookType,
    pub name: &'a str,
    pub description: Option<&'a str>,
    pub config: &'a serde_json::Value,
    pub app_id: Option<Uuid>,
}

#[derive(Clone)]
pub struct HookRepo {
    pool: PgPool,
}

impl HookRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_hook(&self, hook: NewHook<'_>) -> sqlx::Result<Hook> {
        sqlx::query_as!(
            Hook,
            r#"
            INSERT INTO hooks (workspace_id, created_by, hook_type, name, description, config, app_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id, workspace_id, created_by, hook_type AS "hook_type: HookType", name, description,
                   config, is_active AS "is_active!", disabled_reason, app_id, created_at, updated_at
            "#,
            hook.workspace_id,
            hook.created_by,
            hook.hook_type.clone() as HookType,
            hook.name,
            hook.description,
            hook.config,
            hook.app_id
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn find_hook_by_id(&self, id: Uuid) -> sqlx::Result<Option<Hook>> {
        sqlx::query_as!(
            Hook,
            r#"SELECT id, workspace_id, created_by, hook_type AS "hook_type: HookType", name, description,
                   config, is_active AS "is_active!", disabled_reason, app_id, created_at, updated_at
                 FROM hooks WHERE id = $1"#,
            id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn list_hooks(&self, workspace_id: Uuid) -> sqlx::Result<Vec<Hook>> {
        sqlx::query_as!(
            Hook,
            r#"SELECT id, workspace_id, created_by, hook_type AS "hook_type: HookType", name, description,
                   config, is_active AS "is_active!", disabled_reason, app_id, created_at, updated_at
                 FROM hooks WHERE workspace_id = $1 ORDER BY created_at DESC"#,
            workspace_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn update_hook_config(
        &self,
        id: Uuid,
        config: &serde_json::Value,
    ) -> sqlx::Result<Hook> {
        sqlx::query_as!(
            Hook,
            r#"UPDATE hooks SET config = $2, updated_at = NOW() WHERE id = $1
               RETURNING id, workspace_id, created_by, hook_type AS "hook_type: HookType", name, description,
                   config, is_active AS "is_active!", disabled_reason, app_id, created_at, updated_at"#,
            id,
            config
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn delete_hook(&self, id: Uuid) -> sqlx::Result<()> {
        sqlx::query!("DELETE FROM hooks WHERE id = $1", id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn find_active_incoming_hook_by_token(
        &self,
        token: &str,
    ) -> sqlx::Result<Option<Hook>> {
        sqlx::query_as!(
            Hook,
            r#"SELECT id, workspace_id, created_by, hook_type AS "hook_type: HookType", name, description,
                   config, is_active AS "is_active!", disabled_reason, app_id, created_at, updated_at
                 FROM hooks WHERE hook_type = 'incoming_webhook' AND is_active = true AND config->>'token' = $1"#,
            token
        )
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
        sqlx::query_as!(
            Hook,
            r#"SELECT id, workspace_id, created_by, hook_type AS "hook_type: HookType", name, description,
                   config, is_active AS "is_active!", disabled_reason, app_id, created_at, updated_at
                 FROM hooks
                WHERE workspace_id = $1
                  AND hook_type = 'outgoing_webhook'
                  AND is_active = true
                  AND config->'channel_ids' ? $2::text"#,
            workspace_id,
            channel_id.to_string()
        )
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
        sqlx::query_as!(
            Hook,
            r#"SELECT id, workspace_id, created_by, hook_type AS "hook_type: HookType", name, description,
                   config, is_active AS "is_active!", disabled_reason, app_id, created_at, updated_at
                 FROM hooks
                WHERE workspace_id = $1
                  AND hook_type = 'slash_command'
                  AND is_active = true
                  AND config->>'command' = $2"#,
            workspace_id,
            command
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn list_slash_commands(&self, workspace_id: Uuid) -> sqlx::Result<Vec<Hook>> {
        sqlx::query_as!(
            Hook,
            r#"SELECT id, workspace_id, created_by, hook_type AS "hook_type: HookType", name, description,
                   config, is_active AS "is_active!", disabled_reason, app_id, created_at, updated_at
                 FROM hooks
                WHERE workspace_id = $1
                  AND hook_type = 'slash_command'
                  AND is_active = true
                ORDER BY config->>'command'"#,
            workspace_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn incoming_for_channel(
        &self,
        workspace_id: Uuid,
        channel_id: Uuid,
    ) -> sqlx::Result<Vec<ChannelIncomingWebhook>> {
        sqlx::query_as!(
            ChannelIncomingWebhook,
            r#"SELECT id, name, description, created_by, created_at
                 FROM hooks
                WHERE workspace_id = $1
                  AND hook_type = 'incoming_webhook'
                  AND is_active = true
                  AND config->>'channel_id' = $2
                ORDER BY created_at"#,
            workspace_id,
            channel_id.to_string()
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn outgoing_for_channel(
        &self,
        workspace_id: Uuid,
        channel_id: Uuid,
    ) -> sqlx::Result<Vec<ChannelOutgoingWebhook>> {
        sqlx::query_as!(
            ChannelOutgoingWebhook,
            r#"SELECT id, name, description
                 FROM hooks
                WHERE workspace_id = $1
                  AND hook_type = 'outgoing_webhook'
                  AND is_active = true
                  AND config->'channel_ids' ? $2::text
                ORDER BY lower(name)"#,
            workspace_id,
            channel_id.to_string()
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn commands_for_channel(
        &self,
        workspace_id: Uuid,
        channel_id: Uuid,
    ) -> sqlx::Result<Vec<ChannelSlashCommand>> {
        sqlx::query_as!(
            ChannelSlashCommand,
            r#"SELECT id, name, config->>'command' AS "command!"
                 FROM hooks
                WHERE workspace_id = $1
                  AND hook_type = 'slash_command'
                  AND is_active = true
                  AND config->'channel_ids' ? $2::text
                ORDER BY config->>'command'"#,
            workspace_id,
            channel_id.to_string()
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn channel_ids_with_outgoing_hooks(
        &self,
        workspace_id: Uuid,
    ) -> sqlx::Result<Vec<String>> {
        sqlx::query_scalar!(
            r#"SELECT DISTINCT jsonb_array_elements_text(config->'channel_ids') AS "channel_id!"
                 FROM hooks
                WHERE workspace_id = $1
                  AND hook_type = 'outgoing_webhook'
                  AND is_active = true
                  AND jsonb_typeof(config->'channel_ids') = 'array'"#,
            workspace_id
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Reserves this (hook, event) pair. Returns false when somebody already
    /// has it, which is how a redelivery is told apart from a first delivery.
    pub async fn disable(&self, id: Uuid, reason: &str) -> sqlx::Result<()> {
        sqlx::query!(
            "UPDATE hooks SET is_active = false, disabled_reason = $2, updated_at = NOW() WHERE id = $1",
            id,
            reason
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn enable(&self, id: Uuid, created_by: Uuid) -> sqlx::Result<Hook> {
        sqlx::query_as!(
            Hook,
            r#"UPDATE hooks
                  SET is_active = true, disabled_reason = NULL, created_by = $2, updated_at = NOW()
                WHERE id = $1
            RETURNING id, workspace_id, created_by, hook_type AS "hook_type: HookType", name, description,
                      config, is_active AS "is_active!", disabled_reason, app_id, created_at, updated_at"#,
            id,
            created_by
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn disable_by_creator(
        &self,
        workspace_id: Option<Uuid>,
        created_by: Uuid,
        reason: &str,
    ) -> sqlx::Result<Vec<DisabledHook>> {
        sqlx::query_as!(
            DisabledHook,
            r"
            UPDATE hooks
               SET is_active = false, disabled_reason = $3, updated_at = NOW()
             WHERE created_by = $2
               AND ($1::uuid IS NULL OR workspace_id = $1)
               AND is_active
            RETURNING id, workspace_id, name
            ",
            workspace_id,
            created_by,
            reason
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn claim_execution(&self, hook_id: Uuid, event_id: Uuid) -> sqlx::Result<bool> {
        let claimed = sqlx::query!(
            r"
            INSERT INTO hook_executions (hook_id, event_id, event_type)
            VALUES ($1, $2, 'pending')
            ON CONFLICT DO NOTHING
            ",
            hook_id,
            event_id
        )
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
        sqlx::query!(
            r"
            UPDATE hook_executions
               SET event_type = $3, payload = $4, response_status = $5, response_body = $6
             WHERE hook_id = $1 AND event_id = $2
            ",
            hook_id,
            event_id,
            event_type,
            payload,
            response_status,
            response_body
        )
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
        sqlx::query_as!(
            HookExecution,
            r"
            INSERT INTO hook_executions (hook_id, event_type, payload, response_status, response_body)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, hook_id, event_type, payload, response_status, response_body, executed_at
            ",
            hook_id,
            event_type,
            payload,
            response_status,
            response_body
        )
        .fetch_one(&self.pool)
        .await
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
        sqlx::query_scalar!(
            r#"
            WITH local AS (
                SELECT date_trunc('day', NOW() AT TIME ZONE $1)
                       + make_interval(days => $2, hours => $3, mins => $4) AS at
            )
            SELECT CASE
                     WHEN (at AT TIME ZONE $1) <= NOW() THEN (at + interval '1 day') AT TIME ZONE $1
                     ELSE at AT TIME ZONE $1
                   END AS "at!"
            FROM local
            "#,
            timezone,
            day_offset,
            hour,
            minute
        )
        .fetch_one(&self.pool)
        .await
    }
}
