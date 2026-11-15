use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::models::Draft;

#[derive(Clone)]
pub struct DraftRepo {
    pool: PgPool,
}

pub struct DraftWrite<'a> {
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub channel_id: Uuid,
    pub thread_parent_id: Option<Uuid>,
    pub content: &'a str,
    pub files_left_behind: bool,
    pub updated_at: DateTime<Utc>,
}

impl DraftRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list(&self, user_id: Uuid, workspace_id: Uuid) -> sqlx::Result<Vec<Draft>> {
        sqlx::query_as!(
            Draft,
            r#"
            SELECT d.id, d.workspace_id, d.channel_id, d.thread_parent_id, d.content,
                   d.files_left_behind, d.updated_at
              FROM drafts d
              JOIN channel_members cm ON cm.channel_id = d.channel_id AND cm.user_id = d.user_id
              LEFT JOIN messages root ON root.id = d.thread_parent_id
             WHERE d.user_id = $1
               AND d.workspace_id = $2
               AND d.content <> ''
               AND (d.thread_parent_id IS NULL OR root.deleted_at IS NULL OR root.reply_count > 0)
             ORDER BY d.updated_at DESC
            "#,
            user_id,
            workspace_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn save_if_newer(&self, write: DraftWrite<'_>) -> sqlx::Result<Draft> {
        let saved = sqlx::query_as!(
            Draft,
            r#"
            INSERT INTO drafts (workspace_id, user_id, channel_id, thread_parent_id, content,
                                files_left_behind, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, LEAST($7, NOW()))
            ON CONFLICT ON CONSTRAINT drafts_place DO UPDATE
               SET content = EXCLUDED.content,
                   files_left_behind = EXCLUDED.files_left_behind,
                   updated_at = EXCLUDED.updated_at
             WHERE drafts.updated_at < EXCLUDED.updated_at
            RETURNING id, workspace_id, channel_id, thread_parent_id, content,
                      files_left_behind, updated_at AS "updated_at!"
            "#,
            write.workspace_id,
            write.user_id,
            write.channel_id,
            write.thread_parent_id,
            write.content,
            write.files_left_behind,
            write.updated_at
        )
        .fetch_optional(&self.pool)
        .await?;
        match saved {
            Some(draft) => Ok(draft),
            None => {
                self.at(write.user_id, write.channel_id, write.thread_parent_id)
                    .await
            }
        }
    }

    async fn at(
        &self,
        user_id: Uuid,
        channel_id: Uuid,
        thread_parent_id: Option<Uuid>,
    ) -> sqlx::Result<Draft> {
        sqlx::query_as!(
            Draft,
            r#"
            SELECT id, workspace_id, channel_id, thread_parent_id, content,
                   files_left_behind, updated_at
              FROM drafts
             WHERE user_id = $1 AND channel_id = $2 AND thread_parent_id IS NOT DISTINCT FROM $3
            "#,
            user_id,
            channel_id,
            thread_parent_id
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn clear_on_send_in(
        &self,
        conn: &mut PgConnection,
        user_id: Uuid,
        channel_id: Uuid,
        thread_parent_id: Option<Uuid>,
    ) -> sqlx::Result<Option<Draft>> {
        sqlx::query_as!(
            Draft,
            r#"
            UPDATE drafts
               SET content = '', files_left_behind = FALSE, updated_at = NOW()
             WHERE user_id = $1
               AND channel_id = $2
               AND thread_parent_id IS NOT DISTINCT FROM $3
               AND content <> ''
            RETURNING id, workspace_id, channel_id, thread_parent_id, content,
                      files_left_behind, updated_at
            "#,
            user_id,
            channel_id,
            thread_parent_id
        )
        .fetch_optional(conn)
        .await
    }

    pub async fn owned_by(&self, user_id: Uuid) -> sqlx::Result<Vec<serde_json::Value>> {
        sqlx::query_scalar!(
            r#"
            SELECT jsonb_build_object(
                       'workspace_id', d.workspace_id,
                       'channel_id', d.channel_id,
                       'channel_name', c.name,
                       'thread_parent_id', d.thread_parent_id,
                       'content', d.content,
                       'files_left_behind', d.files_left_behind,
                       'updated_at', d.updated_at
                   ) AS "row!"
              FROM drafts d
              JOIN channels c ON c.id = d.channel_id
             WHERE d.user_id = $1 AND d.content <> ''
             ORDER BY d.updated_at
            "#,
            user_id
        )
        .fetch_all(&self.pool)
        .await
    }
}
