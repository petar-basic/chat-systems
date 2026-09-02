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
}
