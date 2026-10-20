use sqlx::PgPool;
use uuid::Uuid;

use super::models::{SavedMessage, SavedMessageDetail};

pub struct SavedRepo {
    pool: PgPool,
}

pub struct NewSavedMessage<'a> {
    pub user_id: Uuid,
    pub workspace_id: Uuid,
    pub message_id: Uuid,
    pub note: Option<&'a str>,
}

impl SavedRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Saving twice is the same as saving once: the unique index turns the
    /// second insert into a no-op, and the row already there is returned.
    pub async fn save(&self, saved: NewSavedMessage<'_>) -> sqlx::Result<SavedMessage> {
        let inserted = sqlx::query_as!(
            SavedMessage,
            r"
            INSERT INTO saved_messages (user_id, workspace_id, message_id, note)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT DO NOTHING
            RETURNING id, user_id, workspace_id, message_id, note, created_at
            ",
            saved.user_id,
            saved.workspace_id,
            saved.message_id,
            saved.note
        )
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = inserted {
            return Ok(row);
        }

        sqlx::query_as!(
            SavedMessage,
            r"
            SELECT id, user_id, workspace_id, message_id, note, created_at
              FROM saved_messages
             WHERE user_id = $1 AND message_id = $2
            ",
            saved.user_id,
            saved.message_id
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn find(&self, id: Uuid) -> sqlx::Result<Option<SavedMessage>> {
        sqlx::query_as!(
            SavedMessage,
            "SELECT id, user_id, workspace_id, message_id, note, created_at
               FROM saved_messages WHERE id = $1",
            id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn list(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
    ) -> sqlx::Result<Vec<SavedMessageDetail>> {
        sqlx::query_as!(
            SavedMessageDetail,
            r#"
            SELECT s.id,
                   s.workspace_id,
                   s.message_id,
                   s.note,
                   s.created_at,
                   m.channel_id,
                   m.user_id AS author_id,
                   m.content,
                   m.created_at AS sent_at
            FROM saved_messages s
            JOIN messages m ON m.id = s.message_id AND m.deleted_at IS NULL
            WHERE s.user_id = $1
              AND s.workspace_id = $2
            ORDER BY s.created_at DESC
            "#,
            user_id,
            workspace_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn delete(&self, id: Uuid) -> sqlx::Result<()> {
        sqlx::query!("DELETE FROM saved_messages WHERE id = $1", id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
