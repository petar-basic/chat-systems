use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RetentionPolicy {
    pub workspace_id: Uuid,
    pub message_days: Option<i32>,
    pub file_days: Option<i32>,
    pub audit_days: i32,
    pub notification_days: i32,
    pub updated_by: Option<Uuid>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRetentionRequest {
    pub message_days: Option<i32>,
    pub file_days: Option<i32>,
    pub audit_days: Option<i32>,
    pub notification_days: Option<i32>,
}

#[derive(Clone)]
pub struct RetentionRepo {
    pool: PgPool,
}

/// Bounded so a first run on a large instance holds no long locks; the caller
/// sleeps between batches.
pub const PURGE_BATCH: i64 = 500;

impl RetentionRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn get(&self, workspace_id: Uuid) -> sqlx::Result<Option<RetentionPolicy>> {
        sqlx::query_as::<_, RetentionPolicy>(
            "SELECT * FROM retention_policies WHERE workspace_id = $1",
        )
        .bind(workspace_id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn upsert(
        &self,
        workspace_id: Uuid,
        req: &UpdateRetentionRequest,
        updated_by: Uuid,
    ) -> sqlx::Result<RetentionPolicy> {
        sqlx::query_as::<_, RetentionPolicy>(
            r"
            INSERT INTO retention_policies
                (workspace_id, message_days, file_days, audit_days, notification_days, updated_by)
            VALUES ($1, $2, $3, COALESCE($4, 730), COALESCE($5, 90), $6)
            ON CONFLICT (workspace_id) DO UPDATE SET
                message_days = EXCLUDED.message_days,
                file_days = EXCLUDED.file_days,
                audit_days = EXCLUDED.audit_days,
                notification_days = EXCLUDED.notification_days,
                updated_by = EXCLUDED.updated_by,
                updated_at = NOW()
            RETURNING *
            ",
        )
        .bind(workspace_id)
        .bind(req.message_days)
        .bind(req.file_days)
        .bind(req.audit_days)
        .bind(req.notification_days)
        .bind(updated_by)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn workspaces_with_policies(&self) -> sqlx::Result<Vec<RetentionPolicy>> {
        sqlx::query_as::<_, RetentionPolicy>("SELECT * FROM retention_policies")
            .fetch_all(&self.pool)
            .await
    }

    /// Counts what a purge would remove without removing it. Dry-run reports
    /// through the same queries the deletion uses, so the number it prints is
    /// the number that would actually go.
    pub async fn count_messages_past(&self, workspace_id: Uuid, days: i32) -> sqlx::Result<i64> {
        sqlx::query_scalar(
            r"
            SELECT COUNT(*) FROM messages m
              JOIN channels c ON c.id = m.channel_id
             WHERE c.workspace_id = $1
               AND m.created_at < NOW() - ($2 || ' days')::interval
            ",
        )
        .bind(workspace_id)
        .bind(days.to_string())
        .fetch_one(&self.pool)
        .await
    }

    /// Deletes one bounded batch and reports how many went, so the caller can
    /// loop until a batch comes back short.
    pub async fn purge_messages(&self, workspace_id: Uuid, days: i32) -> sqlx::Result<u64> {
        let result = sqlx::query(
            r"
            DELETE FROM messages
             WHERE id IN (
                 SELECT m.id FROM messages m
                   JOIN channels c ON c.id = m.channel_id
                  WHERE c.workspace_id = $1
                    AND m.created_at < NOW() - ($2 || ' days')::interval
                  LIMIT $3
             )
            ",
        )
        .bind(workspace_id)
        .bind(days.to_string())
        .bind(PURGE_BATCH)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Files come back so the object can be removed before its row: a row
    /// deleted first leaves an object nobody knows about.
    pub async fn files_past(
        &self,
        workspace_id: Uuid,
        days: i32,
    ) -> sqlx::Result<Vec<(Uuid, String)>> {
        sqlx::query_as(
            r"
            SELECT id, storage_key FROM files
             WHERE workspace_id = $1
               AND created_at < NOW() - ($2 || ' days')::interval
             LIMIT $3
            ",
        )
        .bind(workspace_id)
        .bind(days.to_string())
        .bind(PURGE_BATCH)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn delete_file_rows(&self, ids: &[Uuid]) -> sqlx::Result<u64> {
        if ids.is_empty() {
            return Ok(0);
        }
        let result = sqlx::query("DELETE FROM files WHERE id = ANY($1)")
            .bind(ids)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    pub async fn purge_notifications(&self, workspace_id: Uuid, days: i32) -> sqlx::Result<u64> {
        let result = sqlx::query(
            r"
            DELETE FROM notifications
             WHERE id IN (
                 SELECT id FROM notifications
                  WHERE workspace_id = $1
                    AND created_at < NOW() - ($2 || ' days')::interval
                  LIMIT $3
             )
            ",
        )
        .bind(workspace_id)
        .bind(days.to_string())
        .bind(PURGE_BATCH)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn purge_audit(&self, workspace_id: Uuid, days: i32) -> sqlx::Result<u64> {
        let result = sqlx::query(
            r"
            DELETE FROM audit_log
             WHERE id IN (
                 SELECT id FROM audit_log
                  WHERE workspace_id = $1
                    AND created_at < NOW() - ($2 || ' days')::interval
                  LIMIT $3
             )
            ",
        )
        .bind(workspace_id)
        .bind(days.to_string())
        .bind(PURGE_BATCH)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// No policy governs these and there is no argument for keeping them: a
    /// consumed reset token, an expired refresh token, an invite nobody can use
    /// any more, and webhook call logs old enough that nobody is debugging them.
    pub async fn purge_expired_tokens(&self) -> sqlx::Result<(u64, u64, u64, u64)> {
        let reset = sqlx::query("DELETE FROM password_reset_tokens WHERE expires_at < NOW()")
            .execute(&self.pool)
            .await?
            .rows_affected();

        let refresh = sqlx::query("DELETE FROM refresh_tokens WHERE expires_at < NOW()")
            .execute(&self.pool)
            .await?
            .rows_affected();

        let invites = sqlx::query(
            "DELETE FROM workspace_invites WHERE expires_at IS NOT NULL AND expires_at < NOW()",
        )
        .execute(&self.pool)
        .await?
        .rows_affected();

        let executions = sqlx::query(
            "DELETE FROM hook_executions WHERE executed_at < NOW() - INTERVAL '30 days'",
        )
        .execute(&self.pool)
        .await?
        .rows_affected();

        Ok((reset, refresh, invites, executions))
    }

    pub async fn count_expired_tokens(&self) -> sqlx::Result<(i64, i64, i64, i64)> {
        let reset: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM password_reset_tokens WHERE expires_at < NOW()",
        )
        .fetch_one(&self.pool)
        .await?;
        let refresh: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM refresh_tokens WHERE expires_at < NOW()")
                .fetch_one(&self.pool)
                .await?;
        let invites: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM workspace_invites WHERE expires_at IS NOT NULL AND expires_at < NOW()",
        )
        .fetch_one(&self.pool)
        .await?;
        let executions: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM hook_executions WHERE executed_at < NOW() - INTERVAL '30 days'",
        )
        .fetch_one(&self.pool)
        .await?;
        Ok((reset, refresh, invites, executions))
    }
}
