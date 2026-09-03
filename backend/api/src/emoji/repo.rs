use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize)]
pub struct WorkspaceEmoji {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
    pub storage_key: String,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct EmojiRepo {
    pool: PgPool,
}

impl EmojiRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        workspace_id: Uuid,
        name: &str,
        storage_key: &str,
        created_by: Uuid,
    ) -> sqlx::Result<WorkspaceEmoji> {
        sqlx::query_as!(
            WorkspaceEmoji,
            r"
            INSERT INTO workspace_emojis (workspace_id, name, storage_key, created_by)
            VALUES ($1, $2, $3, $4)
            RETURNING id, workspace_id, name, storage_key, created_by, created_at
            ",
            workspace_id,
            name,
            storage_key,
            created_by
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn list(&self, workspace_id: Uuid) -> sqlx::Result<Vec<WorkspaceEmoji>> {
        sqlx::query_as!(
            WorkspaceEmoji,
            "SELECT id, workspace_id, name, storage_key, created_by, created_at
               FROM workspace_emojis WHERE workspace_id = $1 ORDER BY name",
            workspace_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn find_by_name(
        &self,
        workspace_id: Uuid,
        name: &str,
    ) -> sqlx::Result<Option<WorkspaceEmoji>> {
        sqlx::query_as!(
            WorkspaceEmoji,
            "SELECT id, workspace_id, name, storage_key, created_by, created_at
               FROM workspace_emojis WHERE workspace_id = $1 AND name = $2",
            workspace_id,
            name
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn find(&self, id: Uuid) -> sqlx::Result<Option<WorkspaceEmoji>> {
        sqlx::query_as!(
            WorkspaceEmoji,
            "SELECT id, workspace_id, name, storage_key, created_by, created_at
               FROM workspace_emojis WHERE id = $1",
            id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn delete(&self, id: Uuid) -> sqlx::Result<()> {
        sqlx::query!("DELETE FROM workspace_emojis WHERE id = $1", id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
