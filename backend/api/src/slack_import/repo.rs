use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, sqlx::Type)]
#[sqlx(type_name = "slack_import_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ImportStatus {
    Pending,
    Running,
    Complete,
    Failed,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
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
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
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
        sqlx::query_scalar(
            r"
            INSERT INTO slack_imports (workspace_id, source, dry_run)
            VALUES ($1, $2, $3)
            RETURNING id
            ",
        )
        .bind(workspace_id)
        .bind(source)
        .bind(dry_run)
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
        sqlx::query_as::<_, ImportRun>(
            r"
            INSERT INTO slack_imports (workspace_id, source, storage_key, requested_by, dry_run, status)
            VALUES ($1, $2, $3, $4, $5, 'pending')
            RETURNING *
            ",
        )
        .bind(workspace_id)
        .bind(source)
        .bind(storage_key)
        .bind(requested_by)
        .bind(dry_run)
        .fetch_one(&self.pool)
        .await
    }

    /// One worker takes one job; a second replica takes the next one instead of
    /// the same one.
    pub async fn claim_next(&self) -> sqlx::Result<Option<ImportRun>> {
        sqlx::query_as::<_, ImportRun>(
            r"
            UPDATE slack_imports
               SET status = 'running'
             WHERE id = (
                 SELECT id FROM slack_imports
                  WHERE status = 'pending'
                  ORDER BY started_at
                  FOR UPDATE SKIP LOCKED
                  LIMIT 1
             )
            RETURNING *
            ",
        )
        .fetch_optional(&self.pool)
        .await
    }

    /// Written as the import goes, not only when it ends: a run that takes an
    /// hour should be able to say what it has done so far.
    pub async fn record_progress(&self, id: Uuid, report: &serde_json::Value) -> sqlx::Result<()> {
        sqlx::query("UPDATE slack_imports SET report = $2 WHERE id = $1")
            .bind(id)
            .bind(report)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn fail_run(&self, id: Uuid, error: &str) -> sqlx::Result<()> {
        sqlx::query(
            "UPDATE slack_imports SET status = 'failed', error = $2, finished_at = NOW() WHERE id = $1",
        )
        .bind(id)
        .bind(error)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn find_run(&self, id: Uuid) -> sqlx::Result<Option<ImportRun>> {
        sqlx::query_as::<_, ImportRun>("SELECT * FROM slack_imports WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn list_runs(&self, workspace_id: Uuid) -> sqlx::Result<Vec<ImportRun>> {
        sqlx::query_as::<_, ImportRun>(
            "SELECT * FROM slack_imports WHERE workspace_id = $1 ORDER BY started_at DESC LIMIT 20",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn finish_run(&self, id: Uuid, report: &serde_json::Value) -> sqlx::Result<()> {
        sqlx::query(
            "UPDATE slack_imports SET report = $2, status = 'complete', finished_at = NOW() WHERE id = $1",
        )
            .bind(id)
            .bind(report)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn user_mappings(
        &self,
        workspace_id: Uuid,
    ) -> sqlx::Result<Vec<(String, Uuid, String)>> {
        sqlx::query_as(
            r"
            SELECT su.slack_user_id, su.user_id, COALESCE(u.display_name, u.email)
            FROM slack_users su
            JOIN users u ON u.id = su.user_id
            WHERE su.workspace_id = $1
            ",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn map_user(
        &self,
        workspace_id: Uuid,
        slack_user_id: &str,
        user_id: Uuid,
    ) -> sqlx::Result<()> {
        sqlx::query(
            r"
            INSERT INTO slack_users (workspace_id, slack_user_id, user_id)
            VALUES ($1, $2, $3)
            ON CONFLICT (workspace_id, slack_user_id) DO UPDATE SET user_id = $3
            ",
        )
        .bind(workspace_id)
        .bind(slack_user_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn channel_mappings(&self, workspace_id: Uuid) -> sqlx::Result<Vec<(String, Uuid)>> {
        sqlx::query_as(
            "SELECT slack_channel_id, channel_id FROM slack_channels WHERE workspace_id = $1",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn map_channel(
        &self,
        workspace_id: Uuid,
        slack_channel_id: &str,
        channel_id: Uuid,
    ) -> sqlx::Result<()> {
        sqlx::query(
            r"
            INSERT INTO slack_channels (workspace_id, slack_channel_id, channel_id)
            VALUES ($1, $2, $3)
            ON CONFLICT (workspace_id, slack_channel_id) DO UPDATE SET channel_id = $3
            ",
        )
        .bind(workspace_id)
        .bind(slack_channel_id)
        .bind(channel_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn conversation_mappings(
        &self,
        workspace_id: Uuid,
    ) -> sqlx::Result<Vec<(String, Uuid)>> {
        sqlx::query_as(
            "SELECT slack_channel_id, conversation_id FROM slack_conversations WHERE workspace_id = $1",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn map_conversation(
        &self,
        workspace_id: Uuid,
        slack_channel_id: &str,
        conversation_id: Uuid,
    ) -> sqlx::Result<()> {
        sqlx::query(
            r"
            INSERT INTO slack_conversations (workspace_id, slack_channel_id, conversation_id)
            VALUES ($1, $2, $3)
            ON CONFLICT (workspace_id, slack_channel_id) DO UPDATE SET conversation_id = $3
            ",
        )
        .bind(workspace_id)
        .bind(slack_channel_id)
        .bind(conversation_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn imported_conversation_message_ids(
        &self,
        conversation_id: Uuid,
    ) -> sqlx::Result<Vec<(String, Uuid)>> {
        sqlx::query_as(
            "SELECT slack_ts, id FROM conversation_messages WHERE conversation_id = $1 AND slack_ts IS NOT NULL",
        )
        .bind(conversation_id)
        .fetch_all(&self.pool)
        .await
    }

    /// Everything this channel already holds from a previous run, keyed by the
    /// Slack timestamp it came from. One query per channel rather than one per
    /// message: a resumed import of 200k messages should not be 200k lookups.
    pub async fn imported_message_ids(
        &self,
        channel_id: Uuid,
    ) -> sqlx::Result<Vec<(String, Uuid)>> {
        sqlx::query_as(
            "SELECT slack_ts, id FROM messages WHERE channel_id = $1 AND slack_ts IS NOT NULL",
        )
        .bind(channel_id)
        .fetch_all(&self.pool)
        .await
    }
}
