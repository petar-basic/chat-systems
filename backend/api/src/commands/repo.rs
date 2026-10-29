use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Invocation {
    pub id: Uuid,
    pub hook_id: Uuid,
    pub workspace_id: Uuid,
    pub channel_id: Uuid,
    pub user_id: Uuid,
    pub command: String,
    pub payload: serde_json::Value,
    pub attempts: i32,
    pub answer_message_id: Option<Uuid>,
}

pub struct NewInvocation<'a> {
    pub id: Uuid,
    pub hook_id: Uuid,
    pub workspace_id: Uuid,
    pub channel_id: Uuid,
    pub user_id: Uuid,
    pub command: &'a str,
    pub payload: &'a serde_json::Value,
    pub uses: i32,
    pub ttl_secs: f64,
}

#[derive(Clone)]
pub struct CommandRepo {
    pool: PgPool,
}

impl CommandRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, new: NewInvocation<'_>) -> sqlx::Result<()> {
        sqlx::query!(
            "INSERT INTO command_invocations
                 (id, hook_id, workspace_id, channel_id, user_id, command, payload, uses_left, expires_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW() + make_interval(secs => $9))",
            new.id,
            new.hook_id,
            new.workspace_id,
            new.channel_id,
            new.user_id,
            new.command,
            new.payload,
            new.uses,
            new.ttl_secs
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn schedule_retry(
        &self,
        id: Uuid,
        attempts: i32,
        delay_secs: f64,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            "UPDATE command_invocations
                SET attempts = $2, retry_at = NOW() + make_interval(secs => $3)
              WHERE id = $1",
            id,
            attempts,
            delay_secs
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn settle(&self, id: Uuid, attempts: i32) -> sqlx::Result<()> {
        sqlx::query!(
            "UPDATE command_invocations SET attempts = $2, retry_at = NULL WHERE id = $1",
            id,
            attempts
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn claim_due_retries(&self, lease_secs: f64) -> sqlx::Result<Vec<Invocation>> {
        sqlx::query_as!(
            Invocation,
            "UPDATE command_invocations
                SET retry_at = NOW() + make_interval(secs => $1)
              WHERE id IN (
                  SELECT id FROM command_invocations
                   WHERE retry_at <= NOW()
                   ORDER BY retry_at
                   FOR UPDATE SKIP LOCKED
                   LIMIT 20
              )
          RETURNING id, hook_id, workspace_id, channel_id, user_id, command, payload, attempts,
                    answer_message_id",
            lease_secs
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn use_response_url(&self, id: Uuid) -> sqlx::Result<Option<Invocation>> {
        sqlx::query_as!(
            Invocation,
            "UPDATE command_invocations
                SET uses_left = uses_left - 1
              WHERE id = $1 AND uses_left > 0 AND expires_at > NOW()
          RETURNING id, hook_id, workspace_id, channel_id, user_id, command, payload, attempts,
                    answer_message_id",
            id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn exists(&self, id: Uuid) -> sqlx::Result<bool> {
        sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM command_invocations WHERE id = $1) AS "exists!""#,
            id
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn record_answer(&self, id: Uuid, message_id: Uuid) -> sqlx::Result<()> {
        sqlx::query!(
            "UPDATE command_invocations SET answer_message_id = $2 WHERE id = $1",
            id,
            message_id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn purge_expired(&self) -> sqlx::Result<u64> {
        let result = sqlx::query!(
            "DELETE FROM command_invocations WHERE expires_at < NOW() AND retry_at IS NULL"
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }
}
