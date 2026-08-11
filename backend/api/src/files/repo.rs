use sqlx::PgPool;
use uuid::Uuid;

use super::models::FileRecord;

pub struct NewFile<'a> {
    pub user_id: Uuid,
    pub workspace_id: Uuid,
    pub message_id: Option<Uuid>,
    pub filename: &'a str,
    pub storage_key: &'a str,
    pub mime_type: &'a str,
    pub size_bytes: i64,
}

pub struct FileRepo {
    pool: PgPool,
}

impl FileRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, file: NewFile<'_>) -> sqlx::Result<FileRecord> {
        sqlx::query_as::<_, FileRecord>(
            r"
            INSERT INTO files (user_id, workspace_id, message_id, filename, storage_key, mime_type, size_bytes)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING *
            ",
        )
        .bind(file.user_id)
        .bind(file.workspace_id)
        .bind(file.message_id)
        .bind(file.filename)
        .bind(file.storage_key)
        .bind(file.mime_type)
        .bind(file.size_bytes)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn find_by_id(&self, id: Uuid) -> sqlx::Result<Option<FileRecord>> {
        sqlx::query_as::<_, FileRecord>("SELECT * FROM files WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn find_by_storage_key(&self, key: &str) -> sqlx::Result<Option<FileRecord>> {
        sqlx::query_as::<_, FileRecord>("SELECT * FROM files WHERE storage_key = $1")
            .bind(key)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn list_by_workspace_for_user(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> sqlx::Result<Vec<FileRecord>> {
        sqlx::query_as::<_, FileRecord>(
            "SELECT * FROM files WHERE workspace_id = $1 AND user_id = $2 ORDER BY created_at DESC LIMIT $3 OFFSET $4",
        )
        .bind(workspace_id)
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn link_to_channel_message(
        &self,
        storage_keys: &[String],
        message_id: Uuid,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<()> {
        sqlx::query(
            r"
            UPDATE files
            SET message_id = $1
            WHERE storage_key = ANY($2)
              AND workspace_id = $3
              AND user_id = $4
              AND message_id IS NULL
              AND conversation_message_id IS NULL
            ",
        )
        .bind(message_id)
        .bind(storage_keys)
        .bind(workspace_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn link_to_conversation_message(
        &self,
        storage_keys: &[String],
        message_id: Uuid,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<()> {
        sqlx::query(
            r"
            UPDATE files
            SET conversation_message_id = $1
            WHERE storage_key = ANY($2)
              AND workspace_id = $3
              AND user_id = $4
              AND message_id IS NULL
              AND conversation_message_id IS NULL
            ",
        )
        .bind(message_id)
        .bind(storage_keys)
        .bind(workspace_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn channel_id_for_message(&self, message_id: Uuid) -> sqlx::Result<Option<Uuid>> {
        sqlx::query_scalar("SELECT channel_id FROM messages WHERE id = $1")
            .bind(message_id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn conversation_id_for_message(
        &self,
        message_id: Uuid,
    ) -> sqlx::Result<Option<Uuid>> {
        sqlx::query_scalar("SELECT conversation_id FROM conversation_messages WHERE id = $1")
            .bind(message_id)
            .fetch_optional(&self.pool)
            .await
    }

    /// Avatars are uploaded like any other file and never posted to a message,
    /// so they land in the "nothing owns it" bucket. They are meant to be seen.
    pub async fn is_avatar(&self, storage_key: &str) -> sqlx::Result<bool> {
        sqlx::query_scalar(
            r"
            SELECT EXISTS(
              SELECT 1 FROM users
              WHERE avatar_url IS NOT NULL
                AND position($1 in avatar_url) > 0
            )
            ",
        )
        .bind(storage_key)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn delete(&self, id: Uuid) -> sqlx::Result<Option<FileRecord>> {
        sqlx::query_as::<_, FileRecord>("DELETE FROM files WHERE id = $1 RETURNING *")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    /// Deleting the rows and getting the storage keys back has to be one
    /// statement: a read followed by a delete can hand the same key to two
    /// concurrent callers, and the second one deletes an object the first has
    /// already replaced.
    pub async fn delete_for_channel_message(
        &self,
        message_id: Uuid,
    ) -> sqlx::Result<Vec<FileRecord>> {
        sqlx::query_as::<_, FileRecord>("DELETE FROM files WHERE message_id = $1 RETURNING *")
            .bind(message_id)
            .fetch_all(&self.pool)
            .await
    }

    pub async fn delete_for_conversation_message(
        &self,
        message_id: Uuid,
    ) -> sqlx::Result<Vec<FileRecord>> {
        sqlx::query_as::<_, FileRecord>(
            "DELETE FROM files WHERE conversation_message_id = $1 RETURNING *",
        )
        .bind(message_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn delete_unlinked_from_channel_message(
        &self,
        message_id: Uuid,
        kept_keys: &[String],
    ) -> sqlx::Result<Vec<FileRecord>> {
        sqlx::query_as::<_, FileRecord>(
            "DELETE FROM files WHERE message_id = $1 AND NOT (storage_key = ANY($2)) RETURNING *",
        )
        .bind(message_id)
        .bind(kept_keys)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn delete_unlinked_from_conversation_message(
        &self,
        message_id: Uuid,
        kept_keys: &[String],
    ) -> sqlx::Result<Vec<FileRecord>> {
        sqlx::query_as::<_, FileRecord>(
            "DELETE FROM files WHERE conversation_message_id = $1 \
             AND NOT (storage_key = ANY($2)) RETURNING *",
        )
        .bind(message_id)
        .bind(kept_keys)
        .fetch_all(&self.pool)
        .await
    }
}
