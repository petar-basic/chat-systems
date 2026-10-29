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
        sqlx::query!(
            "INSERT INTO huddle_sessions (id, workspace_id, channel_id, dm_partner_id, initiated_by)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (id) DO NOTHING",
            id,
            workspace_id,
            channel_id,
            dm_partner_id,
            initiated_by
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn record_join(&self, huddle_id: Uuid, user_id: Uuid) -> sqlx::Result<()> {
        sqlx::query!(
            "INSERT INTO huddle_participants (huddle_id, user_id)
             VALUES ($1, $2)
             ON CONFLICT (huddle_id, user_id) DO UPDATE SET left_at = NULL",
            huddle_id,
            user_id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn record_leave(&self, huddle_id: Uuid, user_id: Uuid) -> sqlx::Result<i64> {
        sqlx::query!(
            "UPDATE huddle_participants SET left_at = NOW()
             WHERE huddle_id = $1 AND user_id = $2 AND left_at IS NULL",
            huddle_id,
            user_id
        )
        .execute(&self.pool)
        .await?;

        sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "count!" FROM huddle_participants WHERE huddle_id = $1 AND left_at IS NULL"#,
            huddle_id
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn list_open_channel_sessions(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<Vec<HuddleSession>> {
        sqlx::query_as!(
            HuddleSession,
            "SELECT h.id, h.workspace_id, h.channel_id, h.dm_partner_id, h.initiated_by,
                    h.started_at, h.ended_at
               FROM huddle_sessions h
               JOIN channel_members cm ON cm.channel_id = h.channel_id AND cm.user_id = $2
              WHERE h.workspace_id = $1 AND h.ended_at IS NULL
              ORDER BY h.started_at DESC",
            workspace_id,
            user_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn find_open(
        &self,
        workspace_id: Uuid,
        huddle_id: Uuid,
    ) -> sqlx::Result<Option<HuddleSession>> {
        sqlx::query_as!(
            HuddleSession,
            "SELECT id, workspace_id, channel_id, dm_partner_id, initiated_by, started_at, ended_at
               FROM huddle_sessions
              WHERE id = $1 AND workspace_id = $2 AND ended_at IS NULL",
            huddle_id,
            workspace_id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn invite(
        &self,
        huddle_id: Uuid,
        user_id: Uuid,
        invited_by: Uuid,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            "INSERT INTO huddle_invites (huddle_id, user_id, invited_by)
             VALUES ($1, $2, $3)
             ON CONFLICT (huddle_id, user_id) DO NOTHING",
            huddle_id,
            user_id,
            invited_by
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn end_session(&self, huddle_id: Uuid) -> sqlx::Result<Option<HuddleSession>> {
        sqlx::query_as!(
            HuddleSession,
            "UPDATE huddle_sessions SET ended_at = NOW()
              WHERE id = $1 AND ended_at IS NULL
              RETURNING id, workspace_id, channel_id, dm_partner_id, initiated_by, started_at, ended_at",
            huddle_id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn ensure_livekit_room(&self, huddle_id: Uuid, room: &str) -> sqlx::Result<String> {
        sqlx::query_scalar!(
            r#"UPDATE huddle_sessions SET livekit_room = COALESCE(livekit_room, $2)
                WHERE id = $1
                RETURNING livekit_room AS "livekit_room!""#,
            huddle_id,
            room
        )
        .fetch_one(&self.pool)
        .await
    }
}
