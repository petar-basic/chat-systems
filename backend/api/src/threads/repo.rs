use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::models::{FollowedThread, FollowerState, ThreadState};
use crate::messaging::models::Message;

pub struct ReplyFollows<'a> {
    pub root_message_id: Uuid,
    pub workspace_id: Uuid,
    pub replier_id: Uuid,
    pub root_author_id: Uuid,
    pub mentioned: &'a [Uuid],
    pub replied_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct ThreadRepo {
    pool: PgPool,
}

impl ThreadRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn follow_on_reply_in(
        &self,
        conn: &mut PgConnection,
        follows: ReplyFollows<'_>,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO thread_follows (user_id, workspace_id, root_message_id, following, last_read_at)
            SELECT $1, $2, $3, TRUE, $4
              FROM workspace_members WHERE workspace_id = $2 AND user_id = $1
            ON CONFLICT (user_id, root_message_id) DO UPDATE
               SET following = TRUE,
                   last_read_at = GREATEST(thread_follows.last_read_at, EXCLUDED.last_read_at)
            "#,
            follows.replier_id,
            follows.workspace_id,
            follows.root_message_id,
            follows.replied_at
        )
        .execute(&mut *conn)
        .await?;

        sqlx::query!(
            r#"
            INSERT INTO thread_follows (user_id, workspace_id, root_message_id, following, last_read_at)
            SELECT wm.user_id, $2, $3, TRUE, $4::timestamptz - INTERVAL '1 microsecond'
              FROM workspace_members wm
             WHERE wm.workspace_id = $2 AND wm.user_id = $1 AND wm.user_id <> $5
            ON CONFLICT (user_id, root_message_id) DO NOTHING
            "#,
            follows.root_author_id,
            follows.workspace_id,
            follows.root_message_id,
            follows.replied_at,
            follows.replier_id
        )
        .execute(&mut *conn)
        .await?;

        sqlx::query!(
            r#"
            INSERT INTO thread_follows (user_id, workspace_id, root_message_id, following, last_read_at)
            SELECT wm.user_id, $2, $3, TRUE, $4::timestamptz - INTERVAL '1 microsecond'
              FROM workspace_members wm
             WHERE wm.workspace_id = $2 AND wm.user_id = ANY($1) AND wm.user_id <> $5
            ON CONFLICT (user_id, root_message_id) DO UPDATE SET following = TRUE
            "#,
            follows.mentioned,
            follows.workspace_id,
            follows.root_message_id,
            follows.replied_at,
            follows.replier_id
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    pub async fn follower_states(&self, root_message_id: Uuid) -> sqlx::Result<Vec<FollowerState>> {
        sqlx::query_as!(
            FollowerState,
            r#"
            SELECT f.user_id, f.following,
                   (SELECT count(*) FROM messages m
                     WHERE m.thread_parent_id = f.root_message_id AND m.deleted_at IS NULL
                       AND m.user_id <> f.user_id AND m.created_at > f.last_read_at) AS "unread_count!"
              FROM thread_follows f
             WHERE f.root_message_id = $1 AND f.following
            "#,
            root_message_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn reply_recipients(
        &self,
        root_message_id: Uuid,
        sender_id: Uuid,
    ) -> sqlx::Result<Vec<Uuid>> {
        sqlx::query_scalar!(
            r#"
            SELECT f.user_id
              FROM thread_follows f
              JOIN messages r ON r.id = f.root_message_id
              JOIN channels c ON c.id = r.channel_id
              JOIN workspace_members wm ON wm.workspace_id = f.workspace_id AND wm.user_id = f.user_id
             WHERE f.root_message_id = $1
               AND f.following
               AND f.user_id <> $2
               AND (EXISTS (SELECT 1 FROM channel_members cm
                             WHERE cm.channel_id = r.channel_id AND cm.user_id = f.user_id)
                    OR (c.channel_type = 'public' AND wm.role <> 'guest'))
            "#,
            root_message_id,
            sender_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn mark_read(
        &self,
        user_id: Uuid,
        root_message_id: Uuid,
    ) -> sqlx::Result<Option<ThreadState>> {
        sqlx::query_as!(
            ThreadState,
            r#"
            UPDATE thread_follows SET last_read_at = GREATEST(last_read_at, NOW())
             WHERE user_id = $1 AND root_message_id = $2
            RETURNING root_message_id, following, 0::bigint AS "unread_count!"
            "#,
            user_id,
            root_message_id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn set_following(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        root_message_id: Uuid,
        following: bool,
    ) -> sqlx::Result<ThreadState> {
        sqlx::query_as!(
            ThreadState,
            r#"
            WITH saved AS (
                INSERT INTO thread_follows (user_id, workspace_id, root_message_id, following)
                VALUES ($1, $2, $3, $4)
                ON CONFLICT (user_id, root_message_id) DO UPDATE SET following = EXCLUDED.following
                RETURNING user_id, root_message_id, following, last_read_at
            )
            SELECT s.root_message_id, s.following,
                   CASE WHEN s.following THEN
                       (SELECT count(*) FROM messages m
                         WHERE m.thread_parent_id = s.root_message_id AND m.deleted_at IS NULL
                           AND m.user_id <> s.user_id AND m.created_at > s.last_read_at)
                   ELSE 0 END AS "unread_count!"
              FROM saved s
            "#,
            user_id,
            workspace_id,
            root_message_id,
            following
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn list_followed(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        cursor: Option<DateTime<Utc>>,
        limit: i64,
    ) -> sqlx::Result<Vec<FollowedThread>> {
        sqlx::query_as!(
            FollowedThread,
            r#"
            SELECT f.root_message_id, f.last_read_at, latest.at AS "last_reply_at!",
                   (SELECT count(*) FROM messages m
                     WHERE m.thread_parent_id = f.root_message_id AND m.deleted_at IS NULL
                       AND m.user_id <> f.user_id AND m.created_at > f.last_read_at) AS "unread_count!"
              FROM thread_follows f
              JOIN messages r ON r.id = f.root_message_id AND r.deleted_at IS NULL
              JOIN channels c ON c.id = r.channel_id
              JOIN workspace_members wm ON wm.workspace_id = f.workspace_id AND wm.user_id = f.user_id
             CROSS JOIN LATERAL (
                    SELECT max(m.created_at) AS at FROM messages m
                     WHERE m.thread_parent_id = r.id AND m.deleted_at IS NULL
                   ) latest
             WHERE f.user_id = $1
               AND f.workspace_id = $2
               AND f.following
               AND latest.at IS NOT NULL
               AND ($3::timestamptz IS NULL OR latest.at < $3)
               AND (EXISTS (SELECT 1 FROM channel_members cm
                             WHERE cm.channel_id = r.channel_id AND cm.user_id = f.user_id)
                    OR (c.channel_type = 'public' AND wm.role <> 'guest'))
             ORDER BY latest.at DESC
             LIMIT $4
            "#,
            user_id,
            workspace_id,
            cursor,
            limit
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn unread_thread_count(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
    ) -> sqlx::Result<i64> {
        sqlx::query_scalar!(
            r#"
            SELECT count(*) AS "count!"
              FROM thread_follows f
              JOIN messages r ON r.id = f.root_message_id AND r.deleted_at IS NULL
              JOIN channels c ON c.id = r.channel_id
              JOIN workspace_members wm ON wm.workspace_id = f.workspace_id AND wm.user_id = f.user_id
             WHERE f.user_id = $1
               AND f.workspace_id = $2
               AND f.following
               AND EXISTS (SELECT 1 FROM messages m
                            WHERE m.thread_parent_id = f.root_message_id AND m.deleted_at IS NULL
                              AND m.user_id <> f.user_id AND m.created_at > f.last_read_at)
               AND (EXISTS (SELECT 1 FROM channel_members cm
                             WHERE cm.channel_id = r.channel_id AND cm.user_id = f.user_id)
                    OR (c.channel_type = 'public' AND wm.role <> 'guest'))
            "#,
            user_id,
            workspace_id
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn messages_by_ids(&self, ids: &[Uuid]) -> sqlx::Result<Vec<Message>> {
        sqlx::query_as!(
            Message,
            r#"
            SELECT id, channel_id, user_id, client_message_id, content, metadata AS "metadata!", thread_parent_id, reply_count AS "reply_count!", is_pinned AS "is_pinned!", created_at, updated_at, deleted_at
              FROM messages WHERE id = ANY($1)
            "#,
            ids
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn latest_replies(
        &self,
        root_ids: &[Uuid],
        per_thread: i64,
    ) -> sqlx::Result<Vec<Message>> {
        sqlx::query_as!(
            Message,
            r#"
            SELECT id AS "id!", channel_id AS "channel_id!", user_id AS "user_id!", client_message_id, content AS "content!", metadata AS "metadata!", thread_parent_id, reply_count AS "reply_count!", is_pinned AS "is_pinned!", created_at AS "created_at!", updated_at AS "updated_at!", deleted_at
              FROM (
                SELECT m.*, row_number() OVER (PARTITION BY m.thread_parent_id ORDER BY m.created_at DESC, m.id DESC) AS rn
                  FROM messages m
                 WHERE m.thread_parent_id = ANY($1) AND m.deleted_at IS NULL
              ) recent
             WHERE rn <= $2
             ORDER BY created_at ASC, id ASC
            "#,
            root_ids,
            per_thread
        )
        .fetch_all(&self.pool)
        .await
    }
}
