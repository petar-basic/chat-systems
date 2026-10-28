use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "export_scope", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ExportScope {
    Workspace,
    User,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "export_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ExportStatus {
    Pending,
    Running,
    Complete,
    Failed,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ExportJob {
    pub id: Uuid,
    pub scope: ExportScope,
    pub workspace_id: Option<Uuid>,
    pub subject_user_id: Option<Uuid>,
    pub requested_by: Uuid,
    pub include_dms: bool,
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
    pub status: ExportStatus,
    pub storage_key: Option<String>,
    pub manifest: Option<serde_json::Value>,
    pub error: Option<String>,
    #[serde(skip_serializing)]
    pub download_token: Option<String>,
    pub token_expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateExportRequest {
    #[serde(default)]
    pub include_dms: bool,
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
}

pub struct NewExport {
    pub scope: ExportScope,
    pub workspace_id: Option<Uuid>,
    pub subject_user_id: Option<Uuid>,
    pub requested_by: Uuid,
    pub include_dms: bool,
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
}

#[derive(Debug, Default, Clone, Copy, Serialize, ToSchema)]
pub struct ErasedCounts {
    pub messages: u64,
    pub edits: u64,
    pub reactions: u64,
    pub votes: u64,
    pub comments: u64,
    pub scheduled: u64,
    pub files: u64,
}

#[derive(Clone)]
pub struct ExportRepo {
    pool: PgPool,
}

impl ExportRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, job: NewExport) -> sqlx::Result<ExportJob> {
        sqlx::query_as!(
            ExportJob,
            r#"
            INSERT INTO export_jobs
                (scope, workspace_id, subject_user_id, requested_by, include_dms, since, until)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id, scope AS "scope: ExportScope", workspace_id, subject_user_id, requested_by, include_dms,
                   since, until, status AS "status: ExportStatus", storage_key, manifest, error,
                   download_token, token_expires_at, created_at, completed_at
            "#,
            job.scope as ExportScope,
            job.workspace_id,
            job.subject_user_id,
            job.requested_by,
            job.include_dms,
            job.since,
            job.until
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn find(&self, id: Uuid) -> sqlx::Result<Option<ExportJob>> {
        sqlx::query_as!(
            ExportJob,
            r#"SELECT id, scope AS "scope: ExportScope", workspace_id, subject_user_id, requested_by, include_dms,
                   since, until, status AS "status: ExportStatus", storage_key, manifest, error,
                   download_token, token_expires_at, created_at, completed_at
                 FROM export_jobs WHERE id = $1"#,
            id
        )
        .fetch_optional(&self.pool)
        .await
    }

    /// A workspace's exports, and the person exports its admins started for
    /// its members: those have no workspace of their own to be listed under.
    pub async fn list_for_workspace(
        &self,
        workspace_id: Uuid,
        viewer: Uuid,
    ) -> sqlx::Result<Vec<ExportJob>> {
        sqlx::query_as!(
            ExportJob,
            r#"SELECT id, scope AS "scope: ExportScope", workspace_id, subject_user_id, requested_by, include_dms,
                   since, until, status AS "status: ExportStatus", storage_key, manifest, error,
                   download_token, token_expires_at, created_at, completed_at
                 FROM export_jobs e
                WHERE e.workspace_id = $1
                   OR (e.scope = 'user'
                       AND e.requested_by = $2
                       AND EXISTS (SELECT 1 FROM workspace_members wm
                                    WHERE wm.workspace_id = $1 AND wm.user_id = e.subject_user_id))
                ORDER BY created_at DESC
                LIMIT 50"#,
            workspace_id,
            viewer
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Claims one pending job, marking it running in the same statement so two
    /// worker replicas cannot both run the same export.
    pub async fn claim_next(&self) -> sqlx::Result<Option<ExportJob>> {
        sqlx::query_as!(
            ExportJob,
            r#"
            UPDATE export_jobs
               SET status = 'running'
             WHERE id = (
                 SELECT id FROM export_jobs
                  WHERE status = 'pending'
                  ORDER BY created_at
                  FOR UPDATE SKIP LOCKED
                  LIMIT 1
             )
            RETURNING id, scope AS "scope: ExportScope", workspace_id, subject_user_id, requested_by, include_dms,
                   since, until, status AS "status: ExportStatus", storage_key, manifest, error,
                   download_token, token_expires_at, created_at, completed_at
            "#
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn complete(
        &self,
        id: Uuid,
        storage_key: &str,
        manifest: &serde_json::Value,
        download_token: &str,
        token_ttl_hours: i64,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            r"
            UPDATE export_jobs
               SET status = 'complete',
                   storage_key = $2,
                   manifest = $3,
                   download_token = $4,
                   token_expires_at = NOW() + make_interval(hours => $5),
                   completed_at = NOW()
             WHERE id = $1
            ",
            id,
            storage_key,
            manifest,
            download_token,
            token_ttl_hours as i32
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn fail(&self, id: Uuid, error: &str) -> sqlx::Result<()> {
        sqlx::query!(
            "UPDATE export_jobs SET status = 'failed', error = $2, completed_at = NOW() WHERE id = $1",
            id,
            error
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Consumes the token as it resolves it: a download link works once, and a
    /// second attempt finds nothing rather than handing the archive out again.
    pub async fn claim_download(&self, token: &str) -> sqlx::Result<Option<ExportJob>> {
        sqlx::query_as!(
            ExportJob,
            r#"
            UPDATE export_jobs
               SET download_token = NULL
             WHERE download_token = $1
               AND token_expires_at > NOW()
               AND status = 'complete'
            RETURNING id, scope AS "scope: ExportScope", workspace_id, subject_user_id, requested_by, include_dms,
                   since, until, status AS "status: ExportStatus", storage_key, manifest, error,
                   download_token, token_expires_at, created_at, completed_at
            "#,
            token
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn erase_writing(&self, user_id: Uuid) -> sqlx::Result<ErasedCounts> {
        let mut tx = self.pool.begin().await?;
        let mut erased = ErasedCounts::default();

        sqlx::query!(
            r#"
            DELETE FROM notifications
             WHERE user_id = $1
                OR data->>'message_id' IN (SELECT id::text FROM messages WHERE user_id = $1)
            "#,
            user_id
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "DELETE FROM pending_mention_emails WHERE message_id IN (SELECT id FROM messages WHERE user_id = $1)",
            user_id
        )
        .execute(&mut *tx)
        .await?;
        erased.edits = sqlx::query!(
            r#"
            DELETE FROM message_edits
             WHERE edited_by = $1
                OR message_id IN (SELECT id FROM messages WHERE user_id = $1)
            "#,
            user_id
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();
        erased.reactions = sqlx::query!("DELETE FROM reactions WHERE user_id = $1", user_id)
            .execute(&mut *tx)
            .await?
            .rows_affected();
        erased.votes = sqlx::query!("DELETE FROM poll_votes WHERE user_id = $1", user_id)
            .execute(&mut *tx)
            .await?
            .rows_affected();
        erased.comments =
            sqlx::query!("DELETE FROM list_item_comments WHERE user_id = $1", user_id)
                .execute(&mut *tx)
                .await?
                .rows_affected();
        erased.scheduled =
            sqlx::query!("DELETE FROM scheduled_messages WHERE user_id = $1", user_id)
                .execute(&mut *tx)
                .await?
                .rows_affected();
        sqlx::query!("DELETE FROM later_items WHERE user_id = $1", user_id)
            .execute(&mut *tx)
            .await?;

        let kept = sqlx::query!(
            r#"
            UPDATE messages m
               SET content = '', metadata = '{}'::jsonb, deleted_at = COALESCE(m.deleted_at, NOW())
             WHERE m.user_id = $1
               AND EXISTS (SELECT 1 FROM messages r WHERE r.thread_parent_id = m.id AND r.user_id <> $1 AND r.deleted_at IS NULL)
            "#,
            user_id
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();
        sqlx::query!(
            r#"
            UPDATE messages p
               SET reply_count = GREATEST(p.reply_count - c.n::int, 0)
              FROM (SELECT thread_parent_id, COUNT(*) AS n FROM messages
                     WHERE user_id = $1 AND thread_parent_id IS NOT NULL AND deleted_at IS NULL
                     GROUP BY thread_parent_id) c
             WHERE p.id = c.thread_parent_id
            "#,
            user_id
        )
        .execute(&mut *tx)
        .await?;
        let removed = sqlx::query!(
            r#"
            DELETE FROM messages m
             WHERE m.user_id = $1
               AND NOT EXISTS (SELECT 1 FROM messages r WHERE r.thread_parent_id = m.id AND r.user_id <> $1 AND r.deleted_at IS NULL)
            "#,
            user_id
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();
        erased.messages = kept + removed;

        tx.commit().await?;
        Ok(erased)
    }
}
