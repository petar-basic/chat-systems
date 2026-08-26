use sqlx::PgPool;
use uuid::Uuid;

use super::models::*;

pub struct ConversationRepo {
    pool: PgPool,
}

impl ConversationRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn find_by_id(&self, id: Uuid) -> sqlx::Result<Option<Conversation>> {
        sqlx::query_as::<_, Conversation>("SELECT * FROM conversations WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn find_direct(
        &self,
        workspace_id: Uuid,
        a: Uuid,
        b: Uuid,
    ) -> sqlx::Result<Option<Conversation>> {
        sqlx::query_as::<_, Conversation>(
            r"
            SELECT c.* FROM conversations c
            JOIN conversation_participants p1 ON p1.conversation_id = c.id AND p1.user_id = $2
            JOIN conversation_participants p2 ON p2.conversation_id = c.id AND p2.user_id = $3
            WHERE c.workspace_id = $1 AND c.kind = 'direct'
            LIMIT 1
            ",
        )
        .bind(workspace_id)
        .bind(a)
        .bind(b)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn create(
        &self,
        workspace_id: Uuid,
        kind: ConversationKind,
        created_by: Uuid,
        participants: &[Uuid],
    ) -> sqlx::Result<Conversation> {
        let mut tx = self.pool.begin().await?;

        let conversation = sqlx::query_as::<_, Conversation>(
            r"
            INSERT INTO conversations (workspace_id, kind, created_by)
            VALUES ($1, $2, $3)
            RETURNING *
            ",
        )
        .bind(workspace_id)
        .bind(kind)
        .bind(created_by)
        .fetch_one(&mut *tx)
        .await?;

        for participant in participants {
            sqlx::query(
                r"
                INSERT INTO conversation_participants (conversation_id, user_id)
                VALUES ($1, $2)
                ON CONFLICT (conversation_id, user_id) DO NOTHING
                ",
            )
            .bind(conversation.id)
            .bind(participant)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(conversation)
    }

    pub async fn list_for_user(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<Vec<ConversationSummary>> {
        sqlx::query_as::<_, ConversationSummary>(
            r"
            SELECT c.id,
                   c.workspace_id,
                   c.kind,
                   c.last_message_at,
                   mine.last_read_at,
                   ARRAY(
                       SELECT p.user_id FROM conversation_participants p
                       WHERE p.conversation_id = c.id
                       ORDER BY p.joined_at
                   ) AS participant_ids
            FROM conversations c
            JOIN conversation_participants mine
              ON mine.conversation_id = c.id AND mine.user_id = $2
            WHERE c.workspace_id = $1
            ORDER BY c.last_message_at DESC
            ",
        )
        .bind(workspace_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn participant_ids(&self, conversation_id: Uuid) -> sqlx::Result<Vec<Uuid>> {
        let rows: Vec<(Uuid,)> = sqlx::query_as(
            "SELECT user_id FROM conversation_participants WHERE conversation_id = $1 ORDER BY joined_at",
        )
        .bind(conversation_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    pub async fn is_participant(&self, conversation_id: Uuid, user_id: Uuid) -> sqlx::Result<bool> {
        let row: (bool,) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM conversation_participants WHERE conversation_id = $1 AND user_id = $2)",
        )
        .bind(conversation_id)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }

    pub async fn create_message(
        &self,
        id: Uuid,
        conversation_id: Uuid,
        user_id: Uuid,
        content: &str,
        client_message_id: Option<Uuid>,
        thread_parent_id: Option<Uuid>,
    ) -> sqlx::Result<ConversationMessage> {
        let mut tx = self.pool.begin().await?;

        let message = sqlx::query_as::<_, ConversationMessage>(
            r"
            INSERT INTO conversation_messages (id, conversation_id, user_id, content, client_message_id, thread_parent_id)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING *
            ",
        )
        .bind(id)
        .bind(conversation_id)
        .bind(user_id)
        .bind(content)
        .bind(client_message_id)
        .bind(thread_parent_id)
        .fetch_one(&mut *tx)
        .await?;

        if let Some(parent_id) = thread_parent_id {
            sqlx::query(
                "UPDATE conversation_messages SET reply_count = reply_count + 1 WHERE id = $1",
            )
            .bind(parent_id)
            .execute(&mut *tx)
            .await?;
        }

        sqlx::query("UPDATE conversations SET last_message_at = $2 WHERE id = $1")
            .bind(conversation_id)
            .bind(message.created_at)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(message)
    }

    /// A direct message that already happened somewhere else. Same shape as the
    /// channel side: its own timestamp, no read state moved, no client id.
    pub async fn insert_imported(
        &self,
        conversation_id: Uuid,
        user_id: Uuid,
        content: &str,
        thread_parent_id: Option<Uuid>,
        slack_ts: &str,
        created_at: chrono::DateTime<chrono::Utc>,
    ) -> sqlx::Result<ConversationMessage> {
        let mut tx = self.pool.begin().await?;

        let message = sqlx::query_as::<_, ConversationMessage>(
            r"
            INSERT INTO conversation_messages
                (conversation_id, user_id, content, thread_parent_id, slack_ts, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $6)
            RETURNING *
            ",
        )
        .bind(conversation_id)
        .bind(user_id)
        .bind(content)
        .bind(thread_parent_id)
        .bind(slack_ts)
        .bind(created_at)
        .fetch_one(&mut *tx)
        .await?;

        if let Some(parent_id) = thread_parent_id {
            sqlx::query(
                "UPDATE conversation_messages SET reply_count = reply_count + 1 WHERE id = $1",
            )
            .bind(parent_id)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(message)
    }

    pub async fn list_edits(&self, message_id: Uuid) -> sqlx::Result<Vec<ConversationMessageEdit>> {
        sqlx::query_as::<_, ConversationMessageEdit>(
            "SELECT * FROM conversation_message_edits \
              WHERE message_id = $1 ORDER BY edited_at DESC, id DESC",
        )
        .bind(message_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn find_message(&self, id: Uuid) -> sqlx::Result<Option<ConversationMessage>> {
        sqlx::query_as::<_, ConversationMessage>(
            "SELECT * FROM conversation_messages WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
    }

    /// The retry half of idempotent sending. Scoped by construction: a client id
    /// only ever names a row inside the conversation it was used in.
    pub async fn find_by_client_id(
        &self,
        conversation_id: Uuid,
        client_message_id: Uuid,
    ) -> sqlx::Result<Option<ConversationMessage>> {
        sqlx::query_as::<_, ConversationMessage>(
            "SELECT * FROM conversation_messages \
              WHERE conversation_id = $1 AND client_message_id = $2",
        )
        .bind(conversation_id)
        .bind(client_message_id)
        .fetch_optional(&self.pool)
        .await
    }

    /// Scoped by participation rather than by workspace: a DM belongs to the
    /// people in it, and there is no channel membership to fall back on. The
    /// workspace filter is still applied so a search in one workspace does not
    /// return conversations from another the person also belongs to.
    pub async fn search(
        &self,
        query: &str,
        workspace_id: Uuid,
        requester_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> sqlx::Result<Vec<ConversationMessage>> {
        sqlx::query_as::<_, ConversationMessage>(
            r"
            SELECT m.* FROM conversation_messages m
            JOIN conversations c ON c.id = m.conversation_id
            WHERE (
                m.search_vector @@ plainto_tsquery(search_text_config(), search_normalize($1))
                OR search_normalize($1) <% search_normalize(m.content)
              )
              AND c.workspace_id = $2
              AND m.deleted_at IS NULL
              AND EXISTS (
                SELECT 1 FROM conversation_participants p
                WHERE p.conversation_id = c.id AND p.user_id = $3
              )
            ORDER BY
              ts_rank(m.search_vector, plainto_tsquery(search_text_config(), search_normalize($1))) DESC,
              word_similarity(search_normalize($1), search_normalize(m.content)) DESC,
              m.created_at DESC
            LIMIT $4 OFFSET $5
            ",
        )
        .bind(query)
        .bind(workspace_id)
        .bind(requester_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn list_messages(
        &self,
        conversation_id: Uuid,
        limit: i64,
        before: Option<Uuid>,
    ) -> sqlx::Result<Vec<ConversationMessage>> {
        if let Some(cursor) = before {
            sqlx::query_as::<_, ConversationMessage>(
                r"
                SELECT * FROM conversation_messages
                WHERE conversation_id = $1
                  AND thread_parent_id IS NULL
                  AND (created_at, id) < (SELECT created_at, id FROM conversation_messages WHERE id = $3)
                ORDER BY created_at DESC, id DESC
                LIMIT $2
                ",
            )
            .bind(conversation_id)
            .bind(limit)
            .bind(cursor)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as::<_, ConversationMessage>(
                r"
                SELECT * FROM conversation_messages
                WHERE conversation_id = $1
                  AND thread_parent_id IS NULL
                ORDER BY created_at DESC, id DESC
                LIMIT $2
                ",
            )
            .bind(conversation_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
        }
    }

    pub async fn list_thread(
        &self,
        parent_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> sqlx::Result<Vec<ConversationMessage>> {
        sqlx::query_as::<_, ConversationMessage>(
            r"
            SELECT * FROM conversation_messages
            WHERE thread_parent_id = $1
            ORDER BY created_at ASC, id ASC
            LIMIT $2 OFFSET $3
            ",
        )
        .bind(parent_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn update_message(
        &self,
        id: Uuid,
        content: &str,
        edited_by: Uuid,
    ) -> sqlx::Result<ConversationMessage> {
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            r"
            INSERT INTO conversation_message_edits (message_id, previous_content, edited_by)
            SELECT id, content, $2 FROM conversation_messages WHERE id = $1
            ",
        )
        .bind(id)
        .bind(edited_by)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r"
            DELETE FROM conversation_message_edits
             WHERE message_id = $1
               AND id NOT IN (
                   SELECT id FROM conversation_message_edits
                    WHERE message_id = $1
                    ORDER BY edited_at DESC, id DESC
                    LIMIT $2
               )
            ",
        )
        .bind(id)
        .bind(crate::messaging::repo::MAX_STORED_EDITS)
        .execute(&mut *tx)
        .await?;

        let message = sqlx::query_as::<_, ConversationMessage>(
            r"
            UPDATE conversation_messages
            SET content = $2, edited_at = NOW(), updated_at = NOW()
            WHERE id = $1
            RETURNING *
            ",
        )
        .bind(id)
        .bind(content)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(message)
    }

    pub async fn soft_delete_message(&self, id: Uuid) -> sqlx::Result<ConversationMessage> {
        sqlx::query_as::<_, ConversationMessage>(
            r"
            UPDATE conversation_messages
            SET deleted_at = NOW(), updated_at = NOW()
            WHERE id = $1
            RETURNING *
            ",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn mark_read(&self, conversation_id: Uuid, user_id: Uuid) -> sqlx::Result<()> {
        sqlx::query(
            r"
            UPDATE conversation_participants
            SET last_read_at = NOW()
            WHERE conversation_id = $1 AND user_id = $2
            ",
        )
        .bind(conversation_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn add_reaction(
        &self,
        message_id: Uuid,
        user_id: Uuid,
        emoji: &str,
    ) -> sqlx::Result<ConversationReaction> {
        sqlx::query_as::<_, ConversationReaction>(
            r"
            INSERT INTO conversation_message_reactions (message_id, user_id, emoji)
            VALUES ($1, $2, $3)
            RETURNING *
            ",
        )
        .bind(message_id)
        .bind(user_id)
        .bind(emoji)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn remove_reaction(
        &self,
        message_id: Uuid,
        user_id: Uuid,
        emoji: &str,
    ) -> sqlx::Result<()> {
        sqlx::query(
            "DELETE FROM conversation_message_reactions WHERE message_id = $1 AND user_id = $2 AND emoji = $3",
        )
        .bind(message_id)
        .bind(user_id)
        .bind(emoji)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_reactions_for_messages(
        &self,
        message_ids: &[Uuid],
    ) -> sqlx::Result<Vec<ConversationReaction>> {
        sqlx::query_as::<_, ConversationReaction>(
            "SELECT * FROM conversation_message_reactions WHERE message_id = ANY($1) ORDER BY created_at",
        )
        .bind(message_ids)
        .fetch_all(&self.pool)
        .await
    }
}
