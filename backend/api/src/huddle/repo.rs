use sqlx::PgPool;
use uuid::Uuid;

use super::models::HuddleSession;

#[derive(Clone)]
pub struct HuddleRepo {
    pool: PgPool,
}

impl HuddleRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn start_session(
        &self,
        id: Uuid,
        workspace_id: Uuid,
        channel_id: Option<Uuid>,
        dm_partner_id: Option<Uuid>,
        initiated_by: Uuid,
    ) -> sqlx::Result<()> {
        sqlx::query(
            "INSERT INTO huddle_sessions (id, workspace_id, channel_id, dm_partner_id, initiated_by)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (id) DO NOTHING",
        )
        .bind(id)
        .bind(workspace_id)
        .bind(channel_id)
        .bind(dm_partner_id)
        .bind(initiated_by)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn record_join(&self, huddle_id: Uuid, user_id: Uuid) -> sqlx::Result<()> {
        sqlx::query(
            "INSERT INTO huddle_participants (huddle_id, user_id)
             VALUES ($1, $2)
             ON CONFLICT (huddle_id, user_id) DO UPDATE SET left_at = NULL",
        )
        .bind(huddle_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn record_leave(&self, huddle_id: Uuid, user_id: Uuid) -> sqlx::Result<i64> {
        sqlx::query(
            "UPDATE huddle_participants SET left_at = NOW()
             WHERE huddle_id = $1 AND user_id = $2 AND left_at IS NULL",
        )
        .bind(huddle_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM huddle_participants WHERE huddle_id = $1 AND left_at IS NULL",
        )
        .bind(huddle_id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn end_session(&self, huddle_id: Uuid) -> sqlx::Result<Option<HuddleSession>> {
        sqlx::query_as::<_, HuddleSession>(
            "UPDATE huddle_sessions SET ended_at = NOW()
             WHERE id = $1 AND ended_at IS NULL
             RETURNING *",
        )
        .bind(huddle_id)
        .fetch_optional(&self.pool)
        .await
    }
}
