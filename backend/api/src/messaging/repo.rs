use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::models::*;

/// A message edited in a loop would otherwise accumulate versions without
/// bound. Fifty is far past what anyone reads and far below what hurts.
pub const MAX_STORED_EDITS: i64 = 50;

pub struct ImportedMessage<'a> {
    pub channel_id: Uuid,
    pub user_id: Uuid,
    pub content: &'a str,
    pub thread_parent_id: Option<Uuid>,
    pub slack_ts: &'a str,
    pub created_at: DateTime<Utc>,
}

pub struct MessageRepo {
    pool: PgPool,
}

pub struct MessageSearch<'a> {
    pub query: &'a str,
    pub workspace_id: Uuid,
    pub requester_id: Uuid,
    pub requester_is_guest: bool,
    pub channel_id: Option<Uuid>,
    pub author_id: Option<Uuid>,
    pub has: Option<SearchHas>,
    pub after: Option<DateTime<Utc>>,
    pub before: Option<DateTime<Utc>>,
    pub scope: SearchScope,
    pub limit: i64,
    pub offset: i64,
}

/// One statement for every member of the channel, in the transaction that wrote
/// the message. A per-member round trip here would be an N+1 on the hottest path
/// in the product; the index on `channel_members(channel_id)` covers this.
///
/// Muting is deliberately not consulted: it decides whether a notification is
/// delivered, not whether a message is unread. Letting it change the counter
/// would make the badge disagree with the message list.
async fn bump_unread(
    conn: &mut PgConnection,
    channel_id: Uuid,
    author_id: Uuid,
    mentioned: &[Uuid],
) -> sqlx::Result<()> {
    sqlx::query!(
        r"
        UPDATE channel_members
           SET unread_count = unread_count + 1,
               mention_count = mention_count + CASE WHEN user_id = ANY($3) THEN 1 ELSE 0 END
         WHERE channel_id = $1 AND user_id <> $2
        ",
        channel_id,
        author_id,
        mentioned
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

impl MessageRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_message_in(
        &self,
        conn: &mut PgConnection,
        new: NewMessage<'_>,
    ) -> sqlx::Result<Message> {
        let msg = sqlx::query_as!(
            Message,
            r#"
            INSERT INTO messages (client_message_id, channel_id, user_id, content, thread_parent_id)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, channel_id, user_id, client_message_id, content, metadata AS "metadata!", thread_parent_id, reply_count AS "reply_count!", is_pinned AS "is_pinned!", created_at, updated_at, deleted_at
            "#,
            new.client_message_id,
            new.channel_id,
            new.user_id,
            new.content,
            new.thread_parent_id
        )
        .fetch_one(&mut *conn)
        .await?;

        if let Some(parent_id) = new.thread_parent_id {
            sqlx::query!(
                "UPDATE messages SET reply_count = reply_count + 1 WHERE id = $1",
                parent_id
            )
            .execute(&mut *conn)
            .await?;
        }

        bump_unread(conn, new.channel_id, new.user_id, new.mentioned).await?;

        Ok(msg)
    }

    pub async fn create_message(&self, new: NewMessage<'_>) -> sqlx::Result<Message> {
        let mut tx = self.pool.begin().await?;
        let msg = self.create_message_in(&mut tx, new).await?;
        tx.commit().await?;
        Ok(msg)
    }

    /// A hook posts as itself, not as whoever created it. The identity rides in
    /// `metadata` rather than in a `users` row: a bot account would either have
    /// to join every workspace member list to render, or fail to resolve and
    /// show as nobody. The `user_id` still points at the creator, which is what
    /// keeps the audit trail and the foreign key honest.
    /// A message that already happened somewhere else. Its timestamp, author and
    /// thread position come from the export, and nothing about it is new: no
    /// unread counter moves, because nobody is being told about a conversation
    /// from two years ago.
    pub async fn insert_imported(&self, message: ImportedMessage<'_>) -> sqlx::Result<Message> {
        let mut tx = self.pool.begin().await?;

        let msg = sqlx::query_as!(
            Message,
            r#"
            INSERT INTO messages (channel_id, user_id, content, thread_parent_id, slack_ts, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $6)
            RETURNING id, channel_id, user_id, client_message_id, content, metadata AS "metadata!", thread_parent_id, reply_count AS "reply_count!", is_pinned AS "is_pinned!", created_at, updated_at, deleted_at
            "#,
            message.channel_id,
            message.user_id,
            message.content,
            message.thread_parent_id,
            message.slack_ts,
            message.created_at
        )
        .fetch_one(&mut *tx)
        .await?;

        if let Some(parent_id) = message.thread_parent_id {
            sqlx::query!(
                "UPDATE messages SET reply_count = reply_count + 1 WHERE id = $1",
                parent_id
            )
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        Ok(msg)
    }

    pub async fn create_bot_message_in(
        &self,
        conn: &mut PgConnection,
        channel_id: Uuid,
        posted_by: Uuid,
        content: &str,
        bot: &serde_json::Value,
    ) -> sqlx::Result<Message> {
        let msg = sqlx::query_as!(
            Message,
            r#"
            INSERT INTO messages (channel_id, user_id, content, metadata)
            VALUES ($1, $2, $3, jsonb_build_object('bot', $4::jsonb))
            RETURNING id, channel_id, user_id, client_message_id, content, metadata AS "metadata!", thread_parent_id, reply_count AS "reply_count!", is_pinned AS "is_pinned!", created_at, updated_at, deleted_at
            "#,
            channel_id,
            posted_by,
            content,
            bot
        )
        .fetch_one(&mut *conn)
        .await?;

        bump_unread(conn, channel_id, posted_by, &[]).await?;

        Ok(msg)
    }

    pub async fn create_bot_message(
        &self,
        channel_id: Uuid,
        posted_by: Uuid,
        content: &str,
        bot: &serde_json::Value,
    ) -> sqlx::Result<Message> {
        let mut tx = self.pool.begin().await?;
        let msg = self
            .create_bot_message_in(&mut tx, channel_id, posted_by, content, bot)
            .await?;
        tx.commit().await?;
        Ok(msg)
    }

    /// The retry half of idempotent sending, scoped to the channel by the index
    /// it reads. A client id only ever names a row in the channel it was used in.
    pub async fn find_by_client_id(
        &self,
        channel_id: Uuid,
        client_message_id: Uuid,
    ) -> sqlx::Result<Option<Message>> {
        sqlx::query_as!(
            Message,
            r#"SELECT id, channel_id, user_id, client_message_id, content, metadata AS "metadata!", thread_parent_id, reply_count AS "reply_count!", is_pinned AS "is_pinned!", created_at, updated_at, deleted_at FROM messages WHERE channel_id = $1 AND client_message_id = $2"#,
            channel_id,
            client_message_id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn create_system_message_in(
        &self,
        conn: &mut PgConnection,
        channel_id: Uuid,
        user_id: Uuid,
        content: &str,
        metadata: serde_json::Value,
    ) -> sqlx::Result<Message> {
        sqlx::query_as!(
            Message,
            r#"
            INSERT INTO messages (channel_id, user_id, content, metadata)
            VALUES ($1, $2, $3, $4)
            RETURNING id, channel_id, user_id, client_message_id, content, metadata AS "metadata!", thread_parent_id, reply_count AS "reply_count!", is_pinned AS "is_pinned!", created_at, updated_at, deleted_at
            "#,
            channel_id,
            user_id,
            content,
            metadata
        )
        .fetch_one(&mut *conn)
        .await
    }

    pub async fn create_system_message(
        &self,
        channel_id: Uuid,
        user_id: Uuid,
        content: &str,
        metadata: serde_json::Value,
    ) -> sqlx::Result<Message> {
        let mut tx = self.pool.begin().await?;
        let msg = self
            .create_system_message_in(&mut tx, channel_id, user_id, content, metadata)
            .await?;
        tx.commit().await?;
        Ok(msg)
    }

    pub async fn find_by_id(&self, id: Uuid) -> sqlx::Result<Option<Message>> {
        sqlx::query_as!(
            Message,
            r#"SELECT id, channel_id, user_id, client_message_id, content, metadata AS "metadata!", thread_parent_id, reply_count AS "reply_count!", is_pinned AS "is_pinned!", created_at, updated_at, deleted_at FROM messages WHERE id = $1 AND deleted_at IS NULL"#,
            id
        )
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn list_channel_messages(
        &self,
        channel_id: Uuid,
        limit: i64,
        before: Option<Uuid>,
    ) -> sqlx::Result<Vec<Message>> {
        if let Some(cursor) = before {
            sqlx::query_as!(
            Message,
            r#"
                SELECT id, channel_id, user_id, client_message_id, content, metadata AS "metadata!", thread_parent_id, reply_count AS "reply_count!", is_pinned AS "is_pinned!", created_at, updated_at, deleted_at FROM messages
                WHERE channel_id = $1
                  AND deleted_at IS NULL
                  AND thread_parent_id IS NULL
                  AND (created_at, id) < (SELECT created_at, id FROM messages WHERE id = $3)
                ORDER BY created_at DESC, id DESC
                LIMIT $2
                "#,
            channel_id,
            limit,
            cursor
        )
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as!(
            Message,
            r#"
                SELECT id, channel_id, user_id, client_message_id, content, metadata AS "metadata!", thread_parent_id, reply_count AS "reply_count!", is_pinned AS "is_pinned!", created_at, updated_at, deleted_at FROM messages
                WHERE channel_id = $1
                  AND deleted_at IS NULL
                  AND thread_parent_id IS NULL
                ORDER BY created_at DESC, id DESC
                LIMIT $2
                "#,
            channel_id,
            limit
        )
            .fetch_all(&self.pool)
            .await
        }
    }

    pub async fn list_thread_messages(
        &self,
        parent_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> sqlx::Result<Vec<Message>> {
        sqlx::query_as!(
            Message,
            r#"
            SELECT id, channel_id, user_id, client_message_id, content, metadata AS "metadata!", thread_parent_id, reply_count AS "reply_count!", is_pinned AS "is_pinned!", created_at, updated_at, deleted_at FROM messages
            WHERE thread_parent_id = $1 AND deleted_at IS NULL
            ORDER BY created_at ASC
            LIMIT $2 OFFSET $3
            "#,
            parent_id,
            limit,
            offset
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Keeps the pre-image in the same transaction as the update. An edit that
    /// loses its history is worse than a rejected edit, so if the insert fails
    /// the edit fails with it.
    pub async fn update_message_in(
        &self,
        conn: &mut PgConnection,
        id: Uuid,
        content: &str,
        edited_by: Uuid,
    ) -> sqlx::Result<Message> {
        sqlx::query!(
            r"
            INSERT INTO message_edits (message_id, previous_content, edited_by)
            SELECT id, content, $2 FROM messages WHERE id = $1
            ",
            id,
            edited_by
        )
        .execute(&mut *conn)
        .await?;

        // An edit loop on a long message would otherwise accumulate without
        // bound; retention (CS-030) removes the rest when the message goes.
        sqlx::query!(
            r"
            DELETE FROM message_edits
             WHERE message_id = $1
               AND id NOT IN (
                   SELECT id FROM message_edits
                    WHERE message_id = $1
                    ORDER BY edited_at DESC, id DESC
                    LIMIT $2
               )
            ",
            id,
            MAX_STORED_EDITS
        )
        .execute(&mut *conn)
        .await?;

        let message = sqlx::query_as!(
            Message,
            r#"
            UPDATE messages SET content = $2, updated_at = NOW()
            WHERE id = $1
            RETURNING id, channel_id, user_id, client_message_id, content, metadata AS "metadata!", thread_parent_id, reply_count AS "reply_count!", is_pinned AS "is_pinned!", created_at, updated_at, deleted_at
            "#,
            id,
            content
        )
        .fetch_one(&mut *conn)
        .await?;

        Ok(message)
    }

    pub async fn update_message(
        &self,
        id: Uuid,
        content: &str,
        edited_by: Uuid,
    ) -> sqlx::Result<Message> {
        let mut tx = self.pool.begin().await?;
        let message = self
            .update_message_in(&mut tx, id, content, edited_by)
            .await?;
        tx.commit().await?;
        Ok(message)
    }

    pub async fn list_edits(&self, message_id: Uuid) -> sqlx::Result<Vec<MessageEdit>> {
        sqlx::query_as!(
            MessageEdit,
            "SELECT id, message_id, previous_content, edited_by, edited_at FROM message_edits WHERE message_id = $1 ORDER BY edited_at DESC, id DESC",
            message_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn soft_delete_message_in(
        &self,
        conn: &mut PgConnection,
        id: Uuid,
    ) -> sqlx::Result<()> {
        sqlx::query!("UPDATE messages SET deleted_at = NOW() WHERE id = $1", id)
            .execute(&mut *conn)
            .await?;
        Ok(())
    }

    pub async fn soft_delete_message(&self, id: Uuid) -> sqlx::Result<()> {
        let mut tx = self.pool.begin().await?;
        self.soft_delete_message_in(&mut tx, id).await?;
        tx.commit().await
    }

    pub async fn set_pinned_in(
        &self,
        conn: &mut PgConnection,
        id: Uuid,
        pinned: bool,
    ) -> sqlx::Result<Message> {
        sqlx::query_as!(
            Message,
            r#"UPDATE messages SET is_pinned = $2 WHERE id = $1 RETURNING id, channel_id, user_id, client_message_id, content, metadata AS "metadata!", thread_parent_id, reply_count AS "reply_count!", is_pinned AS "is_pinned!", created_at, updated_at, deleted_at"#,
            id,
            pinned
        )
            .fetch_one(&mut *conn)
            .await
    }

    pub async fn set_pinned(&self, id: Uuid, pinned: bool) -> sqlx::Result<Message> {
        let mut tx = self.pool.begin().await?;
        let message = self.set_pinned_in(&mut tx, id, pinned).await?;
        tx.commit().await?;
        Ok(message)
    }

    pub async fn list_pinned(&self, channel_id: Uuid) -> sqlx::Result<Vec<Message>> {
        sqlx::query_as!(
            Message,
            r#"SELECT id, channel_id, user_id, client_message_id, content, metadata AS "metadata!", thread_parent_id, reply_count AS "reply_count!", is_pinned AS "is_pinned!", created_at, updated_at, deleted_at FROM messages WHERE channel_id = $1 AND is_pinned = true AND deleted_at IS NULL ORDER BY updated_at DESC"#,
            channel_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn add_reaction_in(
        &self,
        conn: &mut PgConnection,
        message_id: Uuid,
        user_id: Uuid,
        emoji: &str,
    ) -> sqlx::Result<Reaction> {
        sqlx::query_as!(
            Reaction,
            r"
            INSERT INTO reactions (message_id, user_id, emoji)
            VALUES ($1, $2, $3)
            RETURNING id, message_id, user_id, emoji, created_at
            ",
            message_id,
            user_id,
            emoji
        )
        .fetch_one(&mut *conn)
        .await
    }

    pub async fn add_reaction(
        &self,
        message_id: Uuid,
        user_id: Uuid,
        emoji: &str,
    ) -> sqlx::Result<Reaction> {
        let mut tx = self.pool.begin().await?;
        let reaction = self
            .add_reaction_in(&mut tx, message_id, user_id, emoji)
            .await?;
        tx.commit().await?;
        Ok(reaction)
    }

    pub async fn remove_reaction_in(
        &self,
        conn: &mut PgConnection,
        message_id: Uuid,
        user_id: Uuid,
        emoji: &str,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            "DELETE FROM reactions WHERE message_id = $1 AND user_id = $2 AND emoji = $3",
            message_id,
            user_id,
            emoji
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    pub async fn remove_reaction(
        &self,
        message_id: Uuid,
        user_id: Uuid,
        emoji: &str,
    ) -> sqlx::Result<()> {
        let mut tx = self.pool.begin().await?;
        self.remove_reaction_in(&mut tx, message_id, user_id, emoji)
            .await?;
        tx.commit().await
    }

    pub async fn list_reactions(&self, message_id: Uuid) -> sqlx::Result<Vec<Reaction>> {
        sqlx::query_as!(
            Reaction,
            "SELECT id, message_id, user_id, emoji, created_at FROM reactions WHERE message_id = $1 ORDER BY created_at",
            message_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn list_reactions_for_messages(
        &self,
        message_ids: &[Uuid],
    ) -> sqlx::Result<Vec<Reaction>> {
        if message_ids.is_empty() {
            return Ok(vec![]);
        }
        sqlx::query_as!(
            Reaction,
            "SELECT id, message_id, user_id, emoji, created_at FROM reactions WHERE message_id = ANY($1) ORDER BY created_at",
            message_ids
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn find_unfurl_target(&self, id: Uuid) -> sqlx::Result<Option<UnfurlTarget>> {
        sqlx::query_as!(
            UnfurlTarget,
            r#"
            SELECT c.workspace_id, m.user_id AS poster_id, m.content, m.metadata AS "metadata!",
                   COALESCE((w.settings->>'link_previews')::boolean, true) AS "previews_enabled!"
              FROM messages m
              JOIN channels c ON c.id = m.channel_id
              JOIN workspaces w ON w.id = c.workspace_id
             WHERE m.id = $1 AND m.deleted_at IS NULL
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn store_unfurls_in(
        &self,
        conn: &mut PgConnection,
        id: Uuid,
        content: &str,
        source: &str,
        unfurls: &serde_json::Value,
        prompts: &serde_json::Value,
    ) -> sqlx::Result<Option<Message>> {
        sqlx::query_as!(
            Message,
            r#"
            UPDATE messages
               SET metadata = COALESCE(metadata, '{}'::jsonb)
                   || jsonb_build_object('unfurls', $4::jsonb, 'unfurls_source', $3::text, 'app_prompts', $5::jsonb)
             WHERE id = $1
               AND content = $2
               AND deleted_at IS NULL
               AND COALESCE(metadata->>'unfurls_source', '') <> $3
               AND NOT COALESCE((metadata->>'unfurls_hidden')::boolean, false)
            RETURNING id, channel_id, user_id, client_message_id, content, metadata AS "metadata!", thread_parent_id, reply_count AS "reply_count!", is_pinned AS "is_pinned!", created_at, updated_at, deleted_at
            "#,
            id,
            content,
            source,
            unfurls,
            prompts
        )
        .fetch_optional(&mut *conn)
        .await
    }

    pub async fn set_attachments_in(
        &self,
        conn: &mut PgConnection,
        id: Uuid,
        attachments: &serde_json::Value,
    ) -> sqlx::Result<Message> {
        sqlx::query_as!(
            Message,
            r#"
            UPDATE messages
               SET metadata = CASE
                       WHEN jsonb_array_length($2::jsonb) = 0
                           THEN COALESCE(metadata, '{}'::jsonb) - 'attachments'
                       ELSE COALESCE(metadata, '{}'::jsonb) || jsonb_build_object('attachments', $2::jsonb)
                   END
             WHERE id = $1
            RETURNING id, channel_id, user_id, client_message_id, content, metadata AS "metadata!", thread_parent_id, reply_count AS "reply_count!", is_pinned AS "is_pinned!", created_at, updated_at, deleted_at
            "#,
            id,
            attachments
        )
        .fetch_one(&mut *conn)
        .await
    }

    pub async fn reset_unfurls_in(
        &self,
        conn: &mut PgConnection,
        id: Uuid,
    ) -> sqlx::Result<Message> {
        sqlx::query_as!(
            Message,
            r#"
            UPDATE messages
               SET metadata = COALESCE(metadata, '{}'::jsonb) - 'unfurls_source'
             WHERE id = $1
            RETURNING id, channel_id, user_id, client_message_id, content, metadata AS "metadata!", thread_parent_id, reply_count AS "reply_count!", is_pinned AS "is_pinned!", created_at, updated_at, deleted_at
            "#,
            id
        )
        .fetch_one(&mut *conn)
        .await
    }

    pub async fn hide_unfurls_in(
        &self,
        conn: &mut PgConnection,
        id: Uuid,
    ) -> sqlx::Result<Message> {
        sqlx::query_as!(
            Message,
            r#"
            UPDATE messages
               SET metadata = (COALESCE(metadata, '{}'::jsonb) - 'unfurls')
                   || '{"unfurls_hidden": true}'::jsonb
             WHERE id = $1
            RETURNING id, channel_id, user_id, client_message_id, content, metadata AS "metadata!", thread_parent_id, reply_count AS "reply_count!", is_pinned AS "is_pinned!", created_at, updated_at, deleted_at
            "#,
            id
        )
        .fetch_one(&mut *conn)
        .await
    }

    pub async fn search(&self, params: MessageSearch<'_>) -> sqlx::Result<Vec<Message>> {
        let mut tx = self.pool.begin().await?;
        sqlx::query_scalar!(
            r#"SELECT set_config('plan_cache_mode', 'force_custom_plan', true) AS "mode!""#
        )
        .fetch_one(&mut *tx)
        .await?;
        let messages = sqlx::query_as!(
            Message,
            r#"
            WITH visible AS MATERIALIZED (
              SELECT c.id FROM channels c
              WHERE c.workspace_id = $2
                AND ($4::uuid IS NULL OR c.id = $4)
                AND (
                  EXISTS (
                    SELECT 1 FROM channel_members cm
                    WHERE cm.channel_id = c.id AND cm.user_id = $3
                  )
                  -- Guests are held to explicit membership everywhere else
                  -- (`authz::require_channel_access`); search must not be the one
                  -- door that opens every public channel to them.
                  OR (NOT $8::bool AND c.channel_type = 'public')
                )
                AND CASE WHEN c.channel_type IN ('dm', 'group_dm') THEN $9::bool ELSE $10::bool END
            ),
            exact AS MATERIALIZED (
              SELECT m.id, m.created_at,
                     CASE WHEN $1 = '' THEN 0::real
                          ELSE ts_rank(m.search_vector, plainto_tsquery(search_text_config(), search_normalize($1)))
                     END AS rank
              FROM messages m
              WHERE ($1 = '' OR m.search_vector @@ plainto_tsquery(search_text_config(), search_normalize($1)))
                AND m.deleted_at IS NULL
                AND m.channel_id IN (SELECT id FROM visible)
              AND ($5::uuid IS NULL OR m.user_id = $5)
              AND ($11::text IS NULL OR CASE $11
                      WHEN 'file' THEN jsonb_path_exists(m.metadata, '$.attachments[*]')
                      WHEN 'image' THEN jsonb_path_exists(m.metadata, '$.attachments[*] ? (@.mime_type starts with "image/")')
                      ELSE m.content ~* 'https?://'
                    END)
              AND ($12::timestamptz IS NULL OR m.created_at >= $12)
              AND ($13::timestamptz IS NULL OR m.created_at < $13)
              ORDER BY m.created_at DESC
              LIMIT 1000
            ),
            fuzzy AS MATERIALIZED (
              SELECT recent.id, recent.created_at,
                     word_similarity(search_normalize($1), search_normalize(recent.content)) AS similarity
              FROM (
                SELECT m.id, m.created_at, m.content
                FROM messages m
                WHERE $1 <> ''
                  AND (SELECT count(*) FROM exact) < $6::bigint + $7::bigint
                  AND search_normalize($1) <% search_normalize(m.content)
                  AND (m.search_vector @@ plainto_tsquery(search_text_config(), search_normalize($1))) IS NOT TRUE
                  AND m.deleted_at IS NULL
                  AND m.channel_id IN (SELECT id FROM visible)
                  AND ($5::uuid IS NULL OR m.user_id = $5)
                  AND ($11::text IS NULL OR CASE $11
                      WHEN 'file' THEN jsonb_path_exists(m.metadata, '$.attachments[*]')
                      WHEN 'image' THEN jsonb_path_exists(m.metadata, '$.attachments[*] ? (@.mime_type starts with "image/")')
                      ELSE m.content ~* 'https?://'
                    END)
                  AND ($12::timestamptz IS NULL OR m.created_at >= $12)
                  AND ($13::timestamptz IS NULL OR m.created_at < $13)
                ORDER BY m.created_at DESC
                LIMIT 1000
              ) recent
            ),
            ranked AS (
              SELECT id, created_at, 0 AS tier, rank, 0::real AS similarity FROM exact
              UNION ALL
              SELECT id, created_at, 1, 0::real, similarity FROM fuzzy
            )
            SELECT m.id, m.channel_id, m.user_id, m.client_message_id, m.content, m.metadata AS "metadata!", m.thread_parent_id, m.reply_count AS "reply_count!", m.is_pinned AS "is_pinned!", m.created_at, m.updated_at, m.deleted_at
            FROM ranked r
            JOIN messages m ON m.id = r.id
            ORDER BY r.tier, r.rank DESC, r.similarity DESC, r.created_at DESC
            LIMIT $6 OFFSET $7
            "#,
            params.query,
            params.workspace_id,
            params.requester_id,
            params.channel_id,
            params.author_id,
            params.limit,
            params.offset,
            params.requester_is_guest,
            params.scope.includes_conversations(),
            params.scope.includes_channels(),
            params.has.map(SearchHas::as_str),
            params.after,
            params.before
        )
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(messages)
    }

    pub async fn list_channel_member_ids(&self, channel_id: Uuid) -> sqlx::Result<Vec<Uuid>> {
        sqlx::query_scalar!(
            "SELECT user_id FROM channel_members WHERE channel_id = $1",
            channel_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn is_channel_admin(&self, channel_id: Uuid, user_id: Uuid) -> sqlx::Result<bool> {
        sqlx::query_scalar!(
            r#"SELECT EXISTS(SELECT 1 FROM channel_members WHERE channel_id = $1 AND user_id = $2 AND role = 'admin') AS "exists!""#,
            channel_id,
            user_id
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn is_workspace_admin_for_channel(
        &self,
        channel_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<bool> {
        sqlx::query_scalar!(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM workspace_members wm
                JOIN channels c ON c.workspace_id = wm.workspace_id
                WHERE c.id = $1 AND wm.user_id = $2 AND wm.role IN ('admin', 'owner')
            ) AS "exists!"
            "#,
            channel_id,
            user_id
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn can_moderate_channel(
        &self,
        channel_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<bool> {
        let ch_admin = self.is_channel_admin(channel_id, user_id).await?;
        if ch_admin {
            return Ok(true);
        }
        self.is_workspace_admin_for_channel(channel_id, user_id)
            .await
    }

    pub async fn mark_read(
        &self,
        channel_id: Uuid,
        user_id: Uuid,
        message_id: Uuid,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            r"
            UPDATE channel_members
            SET last_read_at = NOW(),
                last_read_msg = $3,
                last_read_message_id = $3,
                unread_count = 0,
                mention_count = 0
            WHERE channel_id = $1 AND user_id = $2
            ",
            channel_id,
            user_id,
            message_id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn mark_workspace_read(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<Vec<ReadState>> {
        sqlx::query_as!(
            ReadState,
            r#"
            WITH newest AS (
                SELECT cm.channel_id,
                       (SELECT m.id FROM messages m
                         WHERE m.channel_id = cm.channel_id AND m.deleted_at IS NULL
                         ORDER BY m.created_at DESC, m.id DESC
                         LIMIT 1) AS message_id
                  FROM channel_members cm
                  JOIN channels c ON c.id = cm.channel_id
                 WHERE cm.user_id = $2
                   AND c.workspace_id = $1
                   AND (cm.unread_count > 0 OR cm.mention_count > 0)
            )
            UPDATE channel_members cm
               SET last_read_at = NOW(),
                   last_read_msg = n.message_id,
                   last_read_message_id = n.message_id,
                   unread_count = 0,
                   mention_count = 0
              FROM newest n
             WHERE cm.channel_id = n.channel_id
               AND cm.user_id = $2
               AND n.message_id IS NOT NULL
            RETURNING cm.channel_id, cm.unread_count AS "unread_count!", cm.mention_count AS "mention_count!", cm.last_read_message_id
            "#,
            workspace_id,
            user_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn mark_unread(
        &self,
        channel_id: Uuid,
        user_id: Uuid,
        message_id: Uuid,
    ) -> sqlx::Result<Option<ReadState>> {
        sqlx::query_as!(
            ReadState,
            r#"
            WITH target AS (
                SELECT id, created_at FROM messages
                 WHERE id = $3 AND channel_id = $1 AND deleted_at IS NULL
            ),
            previous AS (
                SELECT m.id FROM messages m, target t
                 WHERE m.channel_id = $1
                   AND m.deleted_at IS NULL
                   AND (m.created_at, m.id) < (t.created_at, t.id)
                 ORDER BY m.created_at DESC, m.id DESC
                 LIMIT 1
            )
            UPDATE channel_members cm
               SET last_read_at = t.created_at - interval '1 microsecond',
                   last_read_msg = (SELECT id FROM previous),
                   last_read_message_id = (SELECT id FROM previous),
                   unread_count = (
                       SELECT COUNT(*)::int FROM messages m
                        WHERE m.channel_id = $1
                          AND m.deleted_at IS NULL
                          AND m.user_id <> $2
                          AND m.created_at >= t.created_at
                   ),
                   mention_count = (
                       SELECT COUNT(*)::int FROM notifications n
                         JOIN messages m ON m.id::text = n.data->>'message_id'
                        WHERE n.user_id = $2
                          AND n.notification_type = 'mention'
                          AND m.channel_id = $1
                          AND m.deleted_at IS NULL
                          AND m.created_at >= t.created_at
                   )
              FROM target t
             WHERE cm.channel_id = $1 AND cm.user_id = $2
            RETURNING cm.channel_id, cm.unread_count AS "unread_count!", cm.mention_count AS "mention_count!", cm.last_read_message_id
            "#,
            channel_id,
            user_id,
            message_id
        )
        .fetch_optional(&self.pool)
        .await
    }

    /// Deleting a message removes it from the count of everyone who had not read
    /// past it yet. Anyone who had already read it never counted it, so their
    /// badge must not go down.
    pub async fn drop_unread_for_message(
        &self,
        channel_id: Uuid,
        message_id: Uuid,
        author_id: Uuid,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            r"
            UPDATE channel_members cm
               SET unread_count = GREATEST(cm.unread_count - 1, 0)
              FROM messages m
             WHERE m.id = $2
               AND cm.channel_id = $1
               AND cm.user_id <> $3
               AND (cm.last_read_at IS NULL OR m.created_at > cm.last_read_at)
            ",
            channel_id,
            message_id,
            author_id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// `last_read_msg` rides along because the client needs the boundary, not
    /// just the count: a channel with forty unread messages and no marker in the
    /// list leaves somebody scrolling and guessing where they stopped.
    pub async fn list_unread_channels(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> sqlx::Result<Vec<UnreadChannelRow>> {
        sqlx::query_as!(
            UnreadChannelRow,
            r#"
            SELECT c.id AS channel_id, c.name AS channel_name,
                   c.channel_type AS "channel_type: crate::workspace::models::ChannelType",
                   cm.muted AS "muted!", cm.unread_count, cm.mention_count,
                   latest.created_at AS "last_activity_at!"
              FROM channel_members cm
              JOIN channels c ON c.id = cm.channel_id
              CROSS JOIN LATERAL (
                  SELECT m.created_at FROM messages m
                   WHERE m.channel_id = c.id AND m.deleted_at IS NULL
                   ORDER BY m.created_at DESC
                   LIMIT 1
              ) latest
             WHERE c.workspace_id = $1
               AND cm.user_id = $2
               AND cm.unread_count > 0
               AND c.is_archived = false
             ORDER BY cm.muted, latest.created_at DESC, c.id
             LIMIT $3 OFFSET $4
            "#,
            workspace_id,
            user_id,
            limit,
            offset
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn newest_unread_messages(
        &self,
        channel_ids: &[Uuid],
        user_id: Uuid,
        per_channel: i64,
    ) -> sqlx::Result<Vec<Message>> {
        if channel_ids.is_empty() {
            return Ok(vec![]);
        }
        sqlx::query_as!(
            Message,
            r#"
            SELECT m.id AS "id!", m.channel_id AS "channel_id!", m.user_id AS "user_id!",
                   m.client_message_id, m.content AS "content!", m.metadata AS "metadata!",
                   m.thread_parent_id, m.reply_count AS "reply_count!", m.is_pinned AS "is_pinned!",
                   m.created_at AS "created_at!", m.updated_at AS "updated_at!", m.deleted_at
              FROM unnest($1::uuid[]) AS wanted(channel_id)
              JOIN channel_members cm ON cm.channel_id = wanted.channel_id AND cm.user_id = $2
              CROSS JOIN LATERAL (
                  SELECT * FROM messages msg
                   WHERE msg.channel_id = wanted.channel_id
                     AND msg.deleted_at IS NULL
                     AND msg.user_id <> $2
                     AND (cm.last_read_at IS NULL OR msg.created_at > cm.last_read_at)
                   ORDER BY msg.created_at DESC
                   LIMIT $3
              ) m
             ORDER BY m.channel_id, m.created_at
            "#,
            channel_ids,
            user_id,
            per_channel
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn unread_counts(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<Vec<(Uuid, i32, i32, Option<Uuid>)>> {
        let rows = sqlx::query!(
            r"
            SELECT cm.channel_id, cm.unread_count, cm.mention_count, cm.last_read_msg
              FROM channel_members cm
              JOIN channels c ON c.id = cm.channel_id
             WHERE c.workspace_id = $1 AND cm.user_id = $2 AND c.is_archived = false
            ",
            workspace_id,
            user_id
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| {
                (
                    r.channel_id,
                    r.unread_count,
                    r.mention_count,
                    r.last_read_msg,
                )
            })
            .collect())
    }

    /// Recomputes what the counters should be and reports the rows that were
    /// wrong. A denormalised counter drifts — a failed transaction, a manual fix,
    /// a restore — and a badge nobody can explain is a bug report nobody can
    /// action. This turns that class into a log line and a number.
    pub async fn reconcile_unread_counts(&self, since_hours: i64) -> sqlx::Result<u64> {
        let result = sqlx::query!(
            r"
            WITH truth AS (
                SELECT cm.channel_id,
                       cm.user_id,
                       COUNT(m.id) FILTER (
                           WHERE m.deleted_at IS NULL
                             AND m.user_id <> cm.user_id
                             AND (cm.last_read_at IS NULL OR m.created_at > cm.last_read_at)
                       )::int AS total
                  FROM channel_members cm
                  JOIN channels c ON c.id = cm.channel_id
                  LEFT JOIN messages m ON m.channel_id = cm.channel_id
                 WHERE c.id IN (
                       SELECT DISTINCT channel_id FROM messages
                        WHERE created_at > NOW() - make_interval(hours => $1)
                 )
                 GROUP BY cm.channel_id, cm.user_id
            )
            UPDATE channel_members cm
               SET unread_count = truth.total
              FROM truth
             WHERE truth.channel_id = cm.channel_id
               AND truth.user_id = cm.user_id
               AND cm.unread_count <> truth.total
            ",
            since_hours as i32
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::PgPool;

    async fn seed_channel(pool: &PgPool) -> (Uuid, Uuid) {
        let user_id = Uuid::new_v4();
        sqlx::query("INSERT INTO users (id, email, status) VALUES ($1, $2, 'active')")
            .bind(user_id)
            .bind(format!("u-{user_id}@test.local"))
            .execute(pool)
            .await
            .expect("insert user");

        let workspace_id = Uuid::new_v4();
        sqlx::query("INSERT INTO workspaces (id, name, slug, owner_id) VALUES ($1, $2, $3, $4)")
            .bind(workspace_id)
            .bind("Test WS")
            .bind(format!("ws-{workspace_id}"))
            .bind(user_id)
            .execute(pool)
            .await
            .expect("insert workspace");

        let channel_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO channels (id, workspace_id, name, channel_type, created_by) VALUES ($1, $2, $3, 'public', $4)",
        )
        .bind(channel_id)
        .bind(workspace_id)
        .bind("general")
        .bind(user_id)
        .execute(pool)
        .await
        .expect("insert channel");

        sqlx::query(
            "INSERT INTO channel_members (channel_id, user_id, role) VALUES ($1, $2, 'member')",
        )
        .bind(channel_id)
        .bind(user_id)
        .execute(pool)
        .await
        .expect("insert channel_member");

        (user_id, channel_id)
    }

    #[test_macros::db_test(migrations = "../migrations")]
    async fn create_reply_increments_parent_reply_count(pool: PgPool) {
        let (user_id, channel_id) = seed_channel(&pool).await;
        let repo = MessageRepo::new(pool);

        let parent = repo
            .create_message(NewMessage {
                channel_id,
                user_id,
                content: "parent message",
                thread_parent_id: None,
                client_message_id: None,
                mentioned: &[],
            })
            .await
            .expect("create parent");
        assert_eq!(parent.reply_count, 0, "fresh parent must have 0 replies");
        assert!(
            parent.thread_parent_id.is_none(),
            "parent must not have a thread_parent_id"
        );

        let reply = repo
            .create_message(NewMessage {
                channel_id,
                user_id,
                content: "a reply",
                thread_parent_id: Some(parent.id),
                client_message_id: None,
                mentioned: &[],
            })
            .await
            .expect("create reply");
        assert_eq!(
            reply.thread_parent_id,
            Some(parent.id),
            "reply must point at the parent"
        );

        let parent_after = repo
            .find_by_id(parent.id)
            .await
            .expect("query parent")
            .expect("parent still exists");
        assert_eq!(
            parent_after.reply_count, 1,
            "creating one thread reply must increment parent.reply_count by exactly 1"
        );
    }

    #[test_macros::db_test(migrations = "../migrations")]
    async fn soft_delete_filters_message(pool: PgPool) {
        let (user_id, channel_id) = seed_channel(&pool).await;
        let repo = MessageRepo::new(pool);

        let msg = repo
            .create_message(NewMessage {
                channel_id,
                user_id,
                content: "doomed message",
                thread_parent_id: None,
                client_message_id: None,
                mentioned: &[],
            })
            .await
            .expect("create message");

        assert!(
            repo.find_by_id(msg.id).await.expect("find").is_some(),
            "message must be visible before soft delete"
        );
        let before = repo
            .list_channel_messages(channel_id, 50, None)
            .await
            .expect("list before");
        assert!(
            before.iter().any(|m| m.id == msg.id),
            "message must appear in channel listing before soft delete"
        );

        repo.soft_delete_message(msg.id).await.expect("soft delete");

        assert!(
            repo.find_by_id(msg.id).await.expect("find after").is_none(),
            "find_by_id must return None after soft delete (deleted_at IS NOT NULL)"
        );

        let after = repo
            .list_channel_messages(channel_id, 50, None)
            .await
            .expect("list after");
        assert!(
            !after.iter().any(|m| m.id == msg.id),
            "soft-deleted message must not appear in channel listing"
        );
    }

    #[test_macros::db_test(migrations = "../migrations")]
    async fn cursor_pagination_orders_newest_first_without_overlap(pool: PgPool) {
        let (user_id, channel_id) = seed_channel(&pool).await;
        let seed_pool = pool.clone();
        let repo = MessageRepo::new(pool);

        let base = chrono::Utc::now() - chrono::Duration::minutes(10);
        let mut ids = Vec::new();
        for i in 0..5i64 {
            let id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO messages (id, channel_id, user_id, content, created_at) VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(id)
            .bind(channel_id)
            .bind(user_id)
            .bind(format!("msg {i}"))
            .bind(base + chrono::Duration::seconds(i))
            .execute(&seed_pool)
            .await
            .expect("insert message");
            ids.push(id);
        }

        let page1 = repo
            .list_channel_messages(channel_id, 2, None)
            .await
            .expect("page1");
        assert_eq!(page1.len(), 2, "first page must hold exactly the limit");
        assert_eq!(page1[0].id, ids[4], "first item must be the newest message");
        assert_eq!(page1[1].id, ids[3], "second item must be the next-newest");
        assert!(
            page1[0].created_at > page1[1].created_at,
            "results must be ordered newest-first by created_at"
        );

        let page2 = repo
            .list_channel_messages(channel_id, 2, Some(page1[1].id))
            .await
            .expect("page2");
        assert_eq!(page2.len(), 2, "second page must hold exactly the limit");
        assert_eq!(
            page2[0].id, ids[2],
            "page2 must continue strictly after the cursor"
        );
        assert_eq!(page2[1].id, ids[1]);

        for m in &page2 {
            assert!(
                !page1.iter().any(|p| p.id == m.id),
                "cursor pagination must not return overlapping rows across pages"
            );
        }

        let page3 = repo
            .list_channel_messages(channel_id, 2, Some(page2[1].id))
            .await
            .expect("page3");
        assert_eq!(
            page3.len(),
            1,
            "final page must contain only the remaining row"
        );
        assert_eq!(page3[0].id, ids[0], "final row must be the oldest message");

        let mut seen: Vec<Uuid> = page1
            .iter()
            .chain(page2.iter())
            .chain(page3.iter())
            .map(|m| m.id)
            .collect();
        seen.sort();
        let mut expected = ids.clone();
        expected.sort();
        assert_eq!(
            seen, expected,
            "pagination must cover every row exactly once"
        );
    }

    #[test_macros::db_test(migrations = "../migrations")]
    async fn cursor_pagination_survives_identical_timestamps(pool: PgPool) {
        let (user_id, channel_id) = seed_channel(&pool).await;
        let seed_pool = pool.clone();
        let repo = MessageRepo::new(pool);

        let stamp = chrono::Utc::now() - chrono::Duration::minutes(5);
        let mut ids = Vec::new();
        for i in 0..4i64 {
            let id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO messages (id, channel_id, user_id, content, created_at) VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(id)
            .bind(channel_id)
            .bind(user_id)
            .bind(format!("same-instant {i}"))
            .bind(stamp)
            .execute(&seed_pool)
            .await
            .expect("insert message");
            ids.push(id);
        }

        let page1 = repo
            .list_channel_messages(channel_id, 2, None)
            .await
            .expect("page1");
        let page2 = repo
            .list_channel_messages(channel_id, 2, Some(page1[1].id))
            .await
            .expect("page2");

        for m in &page2 {
            assert!(
                !page1.iter().any(|p| p.id == m.id),
                "rows sharing a created_at must not repeat across pages"
            );
        }

        let mut seen: Vec<Uuid> = page1.iter().chain(page2.iter()).map(|m| m.id).collect();
        seen.sort();
        let mut expected = ids.clone();
        expected.sort();
        assert_eq!(
            seen, expected,
            "every row must be paged exactly once even when created_at ties"
        );
    }
}
