use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::models::{AttachmentList, DueReminder, LaterAudience, LaterOwner, LaterRow};

#[derive(Clone)]
pub struct LaterRepo {
    pool: PgPool,
}

pub struct NewReminder<'a> {
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub created_by: Uuid,
    pub channel_id: Option<Uuid>,
    pub text: &'a str,
    pub remind_at: DateTime<Utc>,
}

pub struct LaterChange<'a> {
    pub status: Option<&'a str>,
    pub set_reminder: bool,
    pub remind_at: Option<DateTime<Utc>>,
    pub set_note: bool,
    pub note: Option<&'a str>,
}

impl LaterRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        status: &str,
    ) -> sqlx::Result<Vec<LaterRow>> {
        sqlx::query_as!(
            LaterRow,
            r#"
            SELECT li.id, li.workspace_id, li.created_by, li.channel_id, li.message_id, li.text, li.note,
                   li.status, li.remind_at, li.reminded_at, li.completed_at, li.created_at,
                   m.user_id AS "message_author_id?",
                   m.content AS "message_content?",
                   m.created_at AS "message_sent_at?",
                   COALESCE(m.metadata->'attachments', '[]'::jsonb) AS "message_attachments!: AttachmentList",
                   li.list_item_id,
                   l.id AS "list_id?",
                   l.title AS "list_title?"
              FROM later_items li
              LEFT JOIN messages m ON m.id = li.message_id
              LEFT JOIN list_items i ON i.id = li.list_item_id
              LEFT JOIN lists l ON l.id = i.list_id
             WHERE li.user_id = $1
               AND li.workspace_id = $2
               AND li.status = $3
               AND (li.message_id IS NULL OR m.deleted_at IS NULL)
               AND (li.list_item_id IS NULL OR (i.deleted_at IS NULL AND l.deleted_at IS NULL))
             ORDER BY CASE WHEN $3 = 'in_progress' THEN li.remind_at END ASC NULLS LAST,
                      li.updated_at DESC
             LIMIT 500
            "#,
            user_id,
            workspace_id,
            status
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn owner(&self, id: Uuid) -> sqlx::Result<Option<LaterOwner>> {
        sqlx::query_as!(
            LaterOwner,
            "SELECT id, workspace_id, user_id, list_item_id FROM later_items WHERE id = $1",
            id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn save_message(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        message_id: Uuid,
        channel_id: Uuid,
        remind_at: Option<DateTime<Utc>>,
    ) -> sqlx::Result<Uuid> {
        sqlx::query_scalar!(
            r#"
            INSERT INTO later_items (workspace_id, user_id, created_by, message_id, channel_id, remind_at)
            VALUES ($1, $2, $2, $3, $4, $5)
            ON CONFLICT (user_id, message_id) WHERE message_id IS NOT NULL
            DO UPDATE SET status = 'in_progress',
                          completed_at = NULL,
                          remind_at = COALESCE(EXCLUDED.remind_at, later_items.remind_at),
                          reminded_at = CASE WHEN EXCLUDED.remind_at IS NULL
                                             THEN later_items.reminded_at END,
                          updated_at = NOW()
            RETURNING id
            "#,
            workspace_id,
            user_id,
            message_id,
            channel_id,
            remind_at
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn create_reminder(&self, reminder: NewReminder<'_>) -> sqlx::Result<Uuid> {
        sqlx::query_scalar!(
            "INSERT INTO later_items (workspace_id, user_id, created_by, channel_id, text, remind_at)
             VALUES ($1, $2, $3, $4, $5, $6)
             RETURNING id",
            reminder.workspace_id,
            reminder.user_id,
            reminder.created_by,
            reminder.channel_id,
            reminder.text,
            reminder.remind_at
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn update(&self, id: Uuid, change: LaterChange<'_>) -> sqlx::Result<()> {
        sqlx::query!(
            r#"
            UPDATE later_items
               SET status = COALESCE($2, status),
                   completed_at = CASE WHEN $2 = 'completed' THEN NOW()
                                       WHEN $2 IS NOT NULL THEN NULL
                                       ELSE completed_at END,
                   remind_at = CASE WHEN $3 THEN $4 ELSE remind_at END,
                   reminded_at = CASE WHEN $3 THEN NULL ELSE reminded_at END,
                   note = CASE WHEN $5 THEN $6 ELSE note END,
                   updated_at = NOW()
             WHERE id = $1
            "#,
            id,
            change.status,
            change.set_reminder,
            change.remind_at,
            change.set_note,
            change.note
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete(&self, id: Uuid) -> sqlx::Result<()> {
        sqlx::query!("DELETE FROM later_items WHERE id = $1", id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn claim_due(&self) -> sqlx::Result<Vec<DueReminder>> {
        sqlx::query_as!(
            DueReminder,
            r#"
            UPDATE later_items
               SET reminded_at = NOW()
             WHERE id IN (
                 SELECT id FROM later_items
                  WHERE remind_at <= NOW() AND reminded_at IS NULL AND status = 'in_progress'
                  ORDER BY remind_at
                  FOR UPDATE SKIP LOCKED
                  LIMIT 100
             )
            RETURNING id, workspace_id, user_id, channel_id, message_id, text
            "#
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn sync_list_due(&self, only_list: Option<Uuid>) -> sqlx::Result<Vec<LaterAudience>> {
        sqlx::query_as!(
            LaterAudience,
            r#"
            WITH due AS MATERIALIZED (
                SELECT * FROM list_due_assignments($1)
            ),
            added AS (
                INSERT INTO later_items
                       (workspace_id, user_id, created_by, channel_id, list_item_id, text, remind_at)
                SELECT workspace_id, user_id, user_id, channel_id, item_id, title, remind_at
                  FROM due
                 WHERE due >= today
                ON CONFLICT (user_id, list_item_id) WHERE list_item_id IS NOT NULL DO NOTHING
                RETURNING user_id, workspace_id
            ),
            removed AS (
                DELETE FROM later_items li
                 WHERE li.list_item_id IS NOT NULL
                   AND li.status = 'in_progress'
                   AND ($1::uuid IS NULL
                        OR li.list_item_id IN (SELECT id FROM list_items WHERE list_id = $1))
                   AND NOT EXISTS (
                       SELECT 1 FROM due d
                        WHERE d.item_id = li.list_item_id AND d.user_id = li.user_id
                   )
                RETURNING li.user_id, li.workspace_id
            )
            SELECT user_id AS "user_id!", workspace_id AS "workspace_id!" FROM added
            UNION
            SELECT user_id, workspace_id FROM removed
            "#,
            only_list
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn release(&self, id: Uuid) -> sqlx::Result<()> {
        sqlx::query!(
            "UPDATE later_items SET reminded_at = NULL WHERE id = $1",
            id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
