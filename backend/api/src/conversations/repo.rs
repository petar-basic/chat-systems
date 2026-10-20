use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::models::*;
use crate::workspace::models::ChannelType;

pub struct ConversationRepo {
    pool: PgPool,
}

struct ConversationRow {
    id: Uuid,
    workspace_id: Uuid,
    channel_type: ChannelType,
    created_by: Option<Uuid>,
    last_message_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
}

impl ConversationRow {
    fn into_conversation(self) -> Option<Conversation> {
        Some(Conversation {
            id: self.id,
            workspace_id: self.workspace_id,
            kind: ConversationKind::of(&self.channel_type)?,
            created_by: self.created_by,
            last_message_at: self.last_message_at,
            created_at: self.created_at,
        })
    }
}

struct SummaryRow {
    id: Uuid,
    workspace_id: Uuid,
    channel_type: ChannelType,
    last_message_at: DateTime<Utc>,
    last_read_at: Option<DateTime<Utc>>,
    participant_ids: Vec<Uuid>,
}

impl ConversationRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// The last message decides the order of the list, the way an inbox does;
    /// with nothing said yet the channel's own creation time stands in.
    pub async fn find_by_id(&self, id: Uuid) -> sqlx::Result<Option<Conversation>> {
        let row = sqlx::query_as!(
            ConversationRow,
            r#"
            SELECT c.id, c.workspace_id, c.channel_type AS "channel_type: ChannelType", c.created_by, c.created_at,
                   COALESCE((SELECT MAX(m.created_at) FROM messages m WHERE m.channel_id = c.id AND m.deleted_at IS NULL), c.created_at) AS "last_message_at!"
              FROM channels c
             WHERE c.id = $1 AND c.channel_type IN ('dm', 'group_dm')
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.and_then(ConversationRow::into_conversation))
    }

    pub async fn find_direct(
        &self,
        workspace_id: Uuid,
        a: Uuid,
        b: Uuid,
    ) -> sqlx::Result<Option<Conversation>> {
        let row = sqlx::query_as!(
            ConversationRow,
            r#"
            SELECT c.id, c.workspace_id, c.channel_type AS "channel_type: ChannelType", c.created_by, c.created_at,
                   COALESCE((SELECT MAX(m.created_at) FROM messages m WHERE m.channel_id = c.id AND m.deleted_at IS NULL), c.created_at) AS "last_message_at!"
              FROM channels c
              JOIN channel_members p1 ON p1.channel_id = c.id AND p1.user_id = $2
              JOIN channel_members p2 ON p2.channel_id = c.id AND p2.user_id = $3
             WHERE c.workspace_id = $1 AND c.channel_type = 'dm'
             LIMIT 1
            "#,
            workspace_id,
            a,
            b
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.and_then(ConversationRow::into_conversation))
    }

    pub async fn create_in(
        &self,
        conn: &mut PgConnection,
        workspace_id: Uuid,
        kind: ConversationKind,
        created_by: Uuid,
        participants: &[Uuid],
    ) -> sqlx::Result<Conversation> {
        let row = sqlx::query_as!(
            ConversationRow,
            r#"
            INSERT INTO channels (workspace_id, channel_type, created_by)
            VALUES ($1, $2, $3)
            RETURNING id, workspace_id, channel_type AS "channel_type: ChannelType", created_by, created_at,
                      created_at AS "last_message_at!"
            "#,
            workspace_id,
            kind.channel_type() as ChannelType,
            created_by
        )
        .fetch_one(&mut *conn)
        .await?;

        for participant in participants {
            sqlx::query!(
                r"
                INSERT INTO channel_members (channel_id, user_id, role)
                VALUES ($1, $2, 'member')
                ON CONFLICT (channel_id, user_id) DO NOTHING
                ",
                row.id,
                participant
            )
            .execute(&mut *conn)
            .await?;
        }

        Ok(row
            .into_conversation()
            .expect("a channel inserted with a dm type reads back as one"))
    }

    pub async fn create(
        &self,
        workspace_id: Uuid,
        kind: ConversationKind,
        created_by: Uuid,
        participants: &[Uuid],
    ) -> sqlx::Result<Conversation> {
        let mut tx = self.pool.begin().await?;
        let conversation = self
            .create_in(&mut tx, workspace_id, kind, created_by, participants)
            .await?;
        tx.commit().await?;
        Ok(conversation)
    }

    pub async fn list_for_user(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<Vec<ConversationSummary>> {
        let rows = sqlx::query_as!(
            SummaryRow,
            r#"
            SELECT c.id,
                   c.workspace_id,
                   c.channel_type AS "channel_type: ChannelType",
                   COALESCE((SELECT MAX(m.created_at) FROM messages m WHERE m.channel_id = c.id AND m.deleted_at IS NULL), c.created_at) AS "last_message_at!",
                   mine.last_read_at,
                   ARRAY(
                       SELECT p.user_id FROM channel_members p
                       WHERE p.channel_id = c.id
                       ORDER BY p.joined_at, p.user_id
                   ) AS "participant_ids!"
              FROM channels c
              JOIN channel_members mine ON mine.channel_id = c.id AND mine.user_id = $2
             WHERE c.workspace_id = $1 AND c.channel_type IN ('dm', 'group_dm')
             ORDER BY 4 DESC
            "#,
            workspace_id,
            user_id
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .filter_map(|r| {
                Some(ConversationSummary {
                    id: r.id,
                    workspace_id: r.workspace_id,
                    kind: ConversationKind::of(&r.channel_type)?,
                    last_message_at: r.last_message_at,
                    last_read_at: r.last_read_at,
                    participant_ids: r.participant_ids,
                })
            })
            .collect())
    }

    pub async fn participant_ids(&self, conversation_id: Uuid) -> sqlx::Result<Vec<Uuid>> {
        sqlx::query_scalar!(
            "SELECT user_id FROM channel_members WHERE channel_id = $1 ORDER BY joined_at, user_id",
            conversation_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn mark_read(&self, conversation_id: Uuid, user_id: Uuid) -> sqlx::Result<()> {
        sqlx::query!(
            r"
            UPDATE channel_members
               SET last_read_at = NOW(), unread_count = 0, mention_count = 0
             WHERE channel_id = $1 AND user_id = $2
            ",
            conversation_id,
            user_id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
