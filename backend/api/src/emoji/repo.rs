use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
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
        sqlx::query_as::<_, WorkspaceEmoji>(
            r"
            INSERT INTO workspace_emojis (workspace_id, name, storage_key, created_by)
            VALUES ($1, $2, $3, $4)
            RETURNING *
            ",
        )
        .bind(workspace_id)
        .bind(name)
        .bind(storage_key)
        .bind(created_by)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn list(&self, workspace_id: Uuid) -> sqlx::Result<Vec<WorkspaceEmoji>> {
        sqlx::query_as::<_, WorkspaceEmoji>(
            "SELECT * FROM workspace_emojis WHERE workspace_id = $1 ORDER BY name",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn find(&self, id: Uuid) -> sqlx::Result<Option<WorkspaceEmoji>> {
        sqlx::query_as::<_, WorkspaceEmoji>("SELECT * FROM workspace_emojis WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn delete(&self, id: Uuid) -> sqlx::Result<()> {
        sqlx::query("DELETE FROM workspace_emojis WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
