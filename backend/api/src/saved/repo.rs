use sqlx::PgPool;
use uuid::Uuid;

use super::models::{SavedMessage, SavedMessageDetail};

pub struct SavedRepo {
    pool: PgPool,
}

pub struct NewSavedMessage<'a> {
    pub user_id: Uuid,
    pub workspace_id: Uuid,
    pub message_id: Option<Uuid>,
    pub conversation_message_id: Option<Uuid>,
    pub note: Option<&'a str>,
}

impl SavedRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Saving twice is the same as saving once: the partial unique indexes turn
    /// the second insert into a no-op, and the row already there is returned.
    pub async fn save(&self, saved: NewSavedMessage<'_>) -> sqlx::Result<SavedMessage> {
        let inserted = sqlx::query_as::<_, SavedMessage>(
            r"
            INSERT INTO saved_messages (user_id, workspace_id, message_id, conversation_message_id, note)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT DO NOTHING
            RETURNING *
            ",
        )
        .bind(saved.user_id)
        .bind(saved.workspace_id)
        .bind(saved.message_id)
        .bind(saved.conversation_message_id)
        .bind(saved.note)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = inserted {
            return Ok(row);
        }

        sqlx::query_as::<_, SavedMessage>(
            r"
            SELECT * FROM saved_messages
            WHERE user_id = $1
              AND message_id IS NOT DISTINCT FROM $2
              AND conversation_message_id IS NOT DISTINCT FROM $3
            ",
        )
        .bind(saved.user_id)
        .bind(saved.message_id)
        .bind(saved.conversation_message_id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn find(&self, id: Uuid) -> sqlx::Result<Option<SavedMessage>> {
        sqlx::query_as::<_, SavedMessage>("SELECT * FROM saved_messages WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn list(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
    ) -> sqlx::Result<Vec<SavedMessageDetail>> {
        sqlx::query_as::<_, SavedMessageDetail>(
            r"
            SELECT s.id,
                   s.workspace_id,
                   s.message_id,
                   s.conversation_message_id,
                   s.note,
                   s.created_at,
                   m.channel_id,
                   cm.conversation_id,
                   COALESCE(m.user_id, cm.user_id)     AS author_id,
                   COALESCE(m.content, cm.content)     AS content,
                   COALESCE(m.created_at, cm.created_at) AS sent_at
            FROM saved_messages s
            LEFT JOIN messages m ON m.id = s.message_id AND m.deleted_at IS NULL
            LEFT JOIN conversation_messages cm
                   ON cm.id = s.conversation_message_id AND cm.deleted_at IS NULL
            WHERE s.user_id = $1
              AND s.workspace_id = $2
              AND (m.id IS NOT NULL OR cm.id IS NOT NULL)
            ORDER BY s.created_at DESC
            ",
        )
        .bind(user_id)
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn delete(&self, id: Uuid) -> sqlx::Result<()> {
        sqlx::query("DELETE FROM saved_messages WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
