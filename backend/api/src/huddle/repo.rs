use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::models::{HuddleSession, NewSession};

#[derive(Clone)]
pub struct HuddleRepo {
    pool: PgPool,
}

impl HuddleRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn start_session_in(
        conn: &mut PgConnection,
        session: NewSession,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            "INSERT INTO huddle_sessions (id, workspace_id, channel_id, dm_partner_id, initiated_by, message_id)
             VALUES ($1, $2, $3, $4, $5, $6)",
            session.id,
            session.workspace_id,
            session.channel_id,
            session.dm_partner_id,
            session.initiated_by,
            session.message_id
        )
        .execute(&mut *conn)
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
                    h.started_at, h.ended_at, h.message_id
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
            "SELECT id, workspace_id, channel_id, dm_partner_id, initiated_by, started_at, ended_at, message_id
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
              RETURNING id, workspace_id, channel_id, dm_partner_id, initiated_by, started_at, ended_at, message_id",
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

    pub async fn find(&self, huddle_id: Uuid) -> sqlx::Result<Option<HuddleSession>> {
        sqlx::query_as!(
            HuddleSession,
            "SELECT id, workspace_id, channel_id, dm_partner_id, initiated_by, started_at, ended_at, message_id
               FROM huddle_sessions
              WHERE id = $1",
            huddle_id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn set_followed_on_join(
        &self,
        huddle_id: Uuid,
        user_id: Uuid,
        followed: bool,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            "UPDATE huddle_participants SET followed_on_join = $3
              WHERE huddle_id = $1 AND user_id = $2",
            huddle_id,
            user_id,
            followed
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn followed_on_join(&self, huddle_id: Uuid, user_id: Uuid) -> sqlx::Result<bool> {
        sqlx::query_scalar!(
            r#"SELECT COALESCE(
                   (SELECT followed_on_join FROM huddle_participants
                     WHERE huddle_id = $1 AND user_id = $2),
                   FALSE) AS "followed!""#,
            huddle_id,
            user_id
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn participant_ids(&self, huddle_id: Uuid) -> sqlx::Result<Vec<Uuid>> {
        sqlx::query_scalar!(
            "SELECT user_id FROM huddle_participants
              WHERE huddle_id = $1
              ORDER BY joined_at, user_id",
            huddle_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn hears(
        &self,
        message_id: Uuid,
        channel_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<bool> {
        sqlx::query_scalar!(
            r#"SELECT EXISTS(
                   SELECT 1 FROM messages m
                     JOIN huddle_sessions h
                       ON h.message_id = COALESCE(m.thread_parent_id, m.id) AND h.ended_at IS NULL
                     JOIN huddle_participants p
                       ON p.huddle_id = h.id AND p.user_id = $3 AND p.left_at IS NULL
                    WHERE m.id = $1 AND m.channel_id = $2
               ) AS "hears!""#,
            message_id,
            channel_id,
            user_id
        )
        .fetch_one(&self.pool)
        .await
    }
}
