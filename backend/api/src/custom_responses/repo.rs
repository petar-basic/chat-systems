use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::models::{CustomResponse, HeardMessage, COOL_DOWN_SECONDS};

#[derive(Clone)]
pub struct CustomResponseRepo {
    pool: PgPool,
}

impl CustomResponseRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn lock_workspace_in(
        conn: &mut PgConnection,
        workspace_id: Uuid,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            "SELECT pg_advisory_xact_lock(hashtextextended('custom_responses:' || $1::uuid::text, 0))",
            workspace_id
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    pub async fn list(&self, workspace_id: Uuid) -> sqlx::Result<Vec<CustomResponse>> {
        sqlx::query_as!(
            CustomResponse,
            "SELECT id, workspace_id, triggers, answers, created_by, created_at, updated_at
               FROM custom_responses
              WHERE workspace_id = $1
              ORDER BY created_at, id",
            workspace_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn find(&self, workspace_id: Uuid, id: Uuid) -> sqlx::Result<Option<CustomResponse>> {
        sqlx::query_as!(
            CustomResponse,
            "SELECT id, workspace_id, triggers, answers, created_by, created_at, updated_at
               FROM custom_responses
              WHERE workspace_id = $1 AND id = $2",
            workspace_id,
            id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn count_in(conn: &mut PgConnection, workspace_id: Uuid) -> sqlx::Result<i64> {
        sqlx::query_scalar!(
            r#"SELECT count(*) AS "count!" FROM custom_responses WHERE workspace_id = $1"#,
            workspace_id
        )
        .fetch_one(&mut *conn)
        .await
    }

    pub async fn create_in(
        conn: &mut PgConnection,
        workspace_id: Uuid,
        triggers: &[String],
        answers: &[String],
        created_by: Uuid,
    ) -> sqlx::Result<CustomResponse> {
        sqlx::query_as!(
            CustomResponse,
            "INSERT INTO custom_responses (workspace_id, triggers, answers, created_by)
             VALUES ($1, $2, $3, $4)
             RETURNING id, workspace_id, triggers, answers, created_by, created_at, updated_at",
            workspace_id,
            triggers,
            answers,
            created_by
        )
        .fetch_one(&mut *conn)
        .await
    }

    pub async fn update(
        &self,
        workspace_id: Uuid,
        id: Uuid,
        triggers: &[String],
        answers: &[String],
    ) -> sqlx::Result<Option<CustomResponse>> {
        sqlx::query_as!(
            CustomResponse,
            "UPDATE custom_responses
                SET triggers = $3, answers = $4, updated_at = NOW()
              WHERE workspace_id = $1 AND id = $2
              RETURNING id, workspace_id, triggers, answers, created_by, created_at, updated_at",
            workspace_id,
            id,
            triggers,
            answers
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn delete(&self, workspace_id: Uuid, id: Uuid) -> sqlx::Result<bool> {
        let done = sqlx::query!(
            "DELETE FROM custom_responses WHERE workspace_id = $1 AND id = $2",
            workspace_id,
            id
        )
        .execute(&self.pool)
        .await?;
        Ok(done.rows_affected() > 0)
    }

    pub async fn heard(&self, message_id: Uuid) -> sqlx::Result<Option<HeardMessage>> {
        sqlx::query_as!(
            HeardMessage,
            r#"SELECT m.id AS message_id, c.workspace_id, m.channel_id, m.user_id AS author_id,
                      m.content, m.thread_parent_id,
                      (u.is_bot OR m.metadata ? 'bot' OR m.metadata ? 'kind') AS "automated!"
                 FROM messages m
                 JOIN channels c ON c.id = m.channel_id
                 JOIN users u ON u.id = m.user_id
                WHERE m.id = $1
                  AND m.deleted_at IS NULL
                  AND m.created_at > NOW() - INTERVAL '15 minutes'"#,
            message_id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn cooling(&self, channel_id: Uuid) -> sqlx::Result<Vec<Uuid>> {
        sqlx::query_scalar!(
            r#"SELECT DISTINCT response_id AS "response_id!"
                 FROM custom_response_replies
                WHERE channel_id = $1
                  AND response_id IS NOT NULL
                  AND answered_at > NOW() - make_interval(secs => $2)"#,
            channel_id,
            f64::from(COOL_DOWN_SECONDS)
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn claim_in(
        conn: &mut PgConnection,
        message_id: Uuid,
        response_id: Uuid,
        channel_id: Uuid,
    ) -> sqlx::Result<Option<Vec<String>>> {
        let Some(answers) = sqlx::query_scalar!(
            "SELECT answers FROM custom_responses WHERE id = $1 FOR UPDATE",
            response_id
        )
        .fetch_optional(&mut *conn)
        .await?
        else {
            return Ok(None);
        };
        let claimed = sqlx::query!(
            "INSERT INTO custom_response_replies (message_id, response_id, channel_id)
             SELECT $1, $2, $3
              WHERE NOT EXISTS (
                    SELECT 1 FROM custom_response_replies
                     WHERE response_id = $2 AND channel_id = $3
                       AND answered_at > NOW() - make_interval(secs => $4))
             ON CONFLICT (message_id) DO NOTHING",
            message_id,
            response_id,
            channel_id,
            f64::from(COOL_DOWN_SECONDS)
        )
        .execute(&mut *conn)
        .await?;
        Ok((claimed.rows_affected() == 1).then_some(answers))
    }

    pub async fn record_reply_in(
        conn: &mut PgConnection,
        message_id: Uuid,
        reply_id: Uuid,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            "UPDATE custom_response_replies SET reply_id = $2 WHERE message_id = $1",
            message_id,
            reply_id
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }
}
