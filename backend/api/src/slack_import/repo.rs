use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "slack_import_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ImportStatus {
    Pending,
    Running,
    Complete,
    Failed,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ImportRun {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub source: String,
    pub dry_run: bool,
    pub report: serde_json::Value,
    pub status: ImportStatus,
    #[serde(skip_serializing)]
    pub storage_key: Option<String>,
    pub requested_by: Option<Uuid>,
    pub error: Option<String>,
    pub retryable: bool,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub archive_deleted_at: Option<DateTime<Utc>>,
}

pub struct UserMapping {
    pub slack_user_id: String,
    pub user_id: Uuid,
    pub name: String,
    pub erased: bool,
}

pub struct ChannelMapping {
    pub slack_channel_id: String,
    pub channel_id: Uuid,
    pub created_by_run: Option<Uuid>,
}

pub struct KeptArchive {
    pub id: Uuid,
    pub storage_key: String,
}

pub struct SlackImportRepo {
    pool: PgPool,
}

impl SlackImportRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn start_run(
        &self,
        workspace_id: Uuid,
        source: &str,
        dry_run: bool,
    ) -> sqlx::Result<Uuid> {
        sqlx::query_scalar!(
            r"
            INSERT INTO slack_imports (workspace_id, source, dry_run)
            VALUES ($1, $2, $3)
            RETURNING id
            ",
            workspace_id,
            source,
            dry_run
        )
        .fetch_one(&self.pool)
        .await
    }

    /// Queued rather than run: the archive is already in storage, and the
    /// worker picks it up. Started as `complete` by default (that is what the
    /// CLI writes), so a queued row says so explicitly.
    pub async fn queue_run(
        &self,
        workspace_id: Uuid,
        source: &str,
        storage_key: &str,
        requested_by: Uuid,
        dry_run: bool,
    ) -> sqlx::Result<ImportRun> {
        sqlx::query_as!(
            ImportRun,
            r#"
            INSERT INTO slack_imports (workspace_id, source, storage_key, requested_by, dry_run, status)
            VALUES ($1, $2, $3, $4, $5, 'pending')
            RETURNING id, workspace_id, source, dry_run, report, status AS "status: ImportStatus",
                   storage_key, requested_by, error,
                   (status = 'failed' AND storage_key IS NOT NULL) AS "retryable!",
                   started_at, finished_at, archive_deleted_at
            "#,
            workspace_id,
            source,
            storage_key,
            requested_by,
            dry_run
        )
        .fetch_one(&self.pool)
        .await
    }

    /// One worker takes one job; a second replica takes the next one instead of
    /// the same one.
    pub async fn claim_next(&self) -> sqlx::Result<Option<ImportRun>> {
        sqlx::query_as!(
            ImportRun,
            r#"
            UPDATE slack_imports
               SET status = 'running', heartbeat_at = NOW()
             WHERE id = (
                 SELECT id FROM slack_imports
                  WHERE status = 'pending'
                  ORDER BY started_at
                  FOR UPDATE SKIP LOCKED
                  LIMIT 1
             )
            RETURNING id, workspace_id, source, dry_run, report, status AS "status: ImportStatus",
                   storage_key, requested_by, error,
                   (status = 'failed' AND storage_key IS NOT NULL) AS "retryable!",
                   started_at, finished_at, archive_deleted_at
            "#
        )
        .fetch_optional(&self.pool)
        .await
    }

    /// Written as the import goes, not only when it ends: a run that takes an
    /// hour should be able to say what it has done so far.
    pub async fn record_progress(&self, id: Uuid, report: &serde_json::Value) -> sqlx::Result<()> {
        sqlx::query!(
            "UPDATE slack_imports SET report = $2 WHERE id = $1",
            id,
            report
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn beat(&self, id: Uuid) -> sqlx::Result<()> {
        sqlx::query!(
            "UPDATE slack_imports SET heartbeat_at = NOW() WHERE id = $1 AND status = 'running'",
            id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn fail_stale(
        &self,
        silent_for: chrono::Duration,
        error: &str,
    ) -> sqlx::Result<Vec<Uuid>> {
        sqlx::query_scalar!(
            r"
            UPDATE slack_imports
               SET status = 'failed', error = $2, finished_at = NOW()
             WHERE status = 'running'
               AND COALESCE(heartbeat_at, started_at) < NOW() - make_interval(secs => $1)
            RETURNING id
            ",
            silent_for.num_seconds() as f64,
            error
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn requeue(&self, id: Uuid, workspace_id: Uuid) -> sqlx::Result<Option<ImportRun>> {
        sqlx::query_as!(
            ImportRun,
            r#"
            UPDATE slack_imports
               SET status = 'pending', error = NULL, finished_at = NULL, heartbeat_at = NULL,
                   report = '{}'
             WHERE id = $1 AND workspace_id = $2
               AND status = 'failed' AND storage_key IS NOT NULL
            RETURNING id, workspace_id, source, dry_run, report, status AS "status: ImportStatus",
                   storage_key, requested_by, error,
                   (status = 'failed' AND storage_key IS NOT NULL) AS "retryable!",
                   started_at, finished_at, archive_deleted_at
            "#,
            id,
            workspace_id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn archives_kept_past(&self, days: i32) -> sqlx::Result<Vec<KeptArchive>> {
        sqlx::query_as!(
            KeptArchive,
            r#"
            WITH due AS (
                SELECT id, storage_key FROM slack_imports
                 WHERE status IN ('failed', 'complete') AND storage_key IS NOT NULL
                   AND finished_at < NOW() - make_interval(days => $1)
                 FOR UPDATE SKIP LOCKED
            )
            UPDATE slack_imports s
               SET storage_key = NULL, archive_deleted_at = NOW()
              FROM due
             WHERE s.id = due.id
            RETURNING s.id, due.storage_key AS "storage_key!"
            "#,
            days
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn discard_archive(
        &self,
        id: Uuid,
        workspace_id: Uuid,
    ) -> sqlx::Result<Option<KeptArchive>> {
        sqlx::query_as!(
            KeptArchive,
            r#"
            WITH due AS (
                SELECT id, storage_key FROM slack_imports
                 WHERE id = $1 AND workspace_id = $2
                   AND status = 'failed' AND storage_key IS NOT NULL
                 FOR UPDATE
            )
            UPDATE slack_imports s
               SET storage_key = NULL, archive_deleted_at = NOW()
              FROM due
             WHERE s.id = due.id
            RETURNING s.id, due.storage_key AS "storage_key!"
            "#,
            id,
            workspace_id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn keep_archive(&self, id: Uuid, storage_key: &str) -> sqlx::Result<()> {
        sqlx::query!(
            "UPDATE slack_imports SET storage_key = $2, archive_deleted_at = NULL WHERE id = $1",
            id,
            storage_key
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn archive_deleted(&self, id: Uuid) -> sqlx::Result<()> {
        sqlx::query!(
            "UPDATE slack_imports SET storage_key = NULL, archive_deleted_at = NOW() WHERE id = $1",
            id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn fail_run(&self, id: Uuid, error: &str) -> sqlx::Result<()> {
        sqlx::query!(
            "UPDATE slack_imports SET status = 'failed', error = $2, finished_at = NOW() WHERE id = $1",
            id,
            error
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn find_run(&self, id: Uuid) -> sqlx::Result<Option<ImportRun>> {
        sqlx::query_as!(
            ImportRun,
            r#"SELECT id, workspace_id, source, dry_run, report, status AS "status: ImportStatus",
                   storage_key, requested_by, error,
                   (status = 'failed' AND storage_key IS NOT NULL) AS "retryable!",
                   started_at, finished_at, archive_deleted_at
                 FROM slack_imports WHERE id = $1"#,
            id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn list_runs(&self, workspace_id: Uuid) -> sqlx::Result<Vec<ImportRun>> {
        sqlx::query_as!(
            ImportRun,
            r#"SELECT id, workspace_id, source, dry_run, report, status AS "status: ImportStatus",
                   storage_key, requested_by, error,
                   (status = 'failed' AND storage_key IS NOT NULL) AS "retryable!",
                   started_at, finished_at, archive_deleted_at
                 FROM slack_imports WHERE workspace_id = $1 ORDER BY started_at DESC LIMIT 20"#,
            workspace_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn finish_run(&self, id: Uuid, report: &serde_json::Value) -> sqlx::Result<()> {
        sqlx::query!(
            "UPDATE slack_imports SET report = $2, status = 'complete', finished_at = NOW() WHERE id = $1",
            id,
            report
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn user_mappings(&self, workspace_id: Uuid) -> sqlx::Result<Vec<UserMapping>> {
        sqlx::query_as!(
            UserMapping,
            r#"
            SELECT su.slack_user_id, su.user_id,
                   COALESCE(
                       (SELECT wm.display_name FROM workspace_members wm
                         WHERE wm.workspace_id = su.workspace_id AND wm.user_id = su.user_id),
                       u.display_name, u.email
                   ) AS "name!",
                   (u.email = 'deleted-' || u.id || '@invalid') AS "erased!"
            FROM slack_users su
            JOIN users u ON u.id = su.user_id
            WHERE su.workspace_id = $1
            "#,
            workspace_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn workspace_member_ids(&self, workspace_id: Uuid) -> sqlx::Result<Vec<Uuid>> {
        sqlx::query_scalar!(
            "SELECT user_id FROM workspace_members WHERE workspace_id = $1",
            workspace_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn channel_member_ids(&self, channel_id: Uuid) -> sqlx::Result<Vec<Uuid>> {
        sqlx::query_scalar!(
            "SELECT user_id FROM channel_members WHERE channel_id = $1",
            channel_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn record_departures(&self, channel_id: Uuid, user_ids: &[Uuid]) -> sqlx::Result<()> {
        sqlx::query!(
            r"
            INSERT INTO conversation_departures (channel_id, user_id, joined_at, left_at, last_read_at)
            SELECT $1, user_id, NOW(), NOW(), NOW() FROM UNNEST($2::uuid[]) AS gone(user_id)
            ON CONFLICT (channel_id, user_id) DO NOTHING
            ",
            channel_id,
            user_ids
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn map_user(
        &self,
        workspace_id: Uuid,
        slack_user_id: &str,
        user_id: Uuid,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            r"
            INSERT INTO slack_users (workspace_id, slack_user_id, user_id)
            VALUES ($1, $2, $3)
            ON CONFLICT (workspace_id, slack_user_id) DO UPDATE SET user_id = $3
            ",
            workspace_id,
            slack_user_id,
            user_id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn channel_mappings(&self, workspace_id: Uuid) -> sqlx::Result<Vec<ChannelMapping>> {
        sqlx::query_as!(
            ChannelMapping,
            "SELECT slack_channel_id, channel_id, created_by_run FROM slack_channels WHERE workspace_id = $1",
            workspace_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn map_channel(
        &self,
        workspace_id: Uuid,
        slack_channel_id: &str,
        channel_id: Uuid,
        created_by_run: Option<Uuid>,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            r"
            INSERT INTO slack_channels (workspace_id, slack_channel_id, channel_id, created_by_run)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (workspace_id, slack_channel_id)
            DO UPDATE SET channel_id = $3, created_by_run = $4
            ",
            workspace_id,
            slack_channel_id,
            channel_id,
            created_by_run
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Everything this channel already holds from a previous run, keyed by the
    /// Slack timestamp it came from. One query per channel rather than one per
    /// message: a resumed import of 200k messages should not be 200k lookups.
    pub async fn imported_message_ids(
        &self,
        channel_id: Uuid,
    ) -> sqlx::Result<Vec<(String, Uuid)>> {
        let rows = sqlx::query!(
            r#"SELECT slack_ts AS "slack_ts!", id FROM messages WHERE channel_id = $1 AND slack_ts IS NOT NULL"#,
            channel_id
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| (r.slack_ts, r.id)).collect())
    }
}
