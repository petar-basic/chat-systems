use chrono::NaiveDate;
use sqlx::PgPool;
use uuid::Uuid;

use super::models::{DailyActivity, MemberActivity, WeeklyActivity};
use crate::workspace::models::WorkspaceRole;

#[derive(Clone)]
pub struct AnalyticsRepo {
    pool: PgPool,
}

impl AnalyticsRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn count_message(&self, event_id: Uuid, message_id: Uuid) -> sqlx::Result<()> {
        sqlx::query!(
            r#"WITH fresh AS (
                   INSERT INTO analytics_counted_events (event_id) VALUES ($1)
                   ON CONFLICT DO NOTHING
                   RETURNING event_id
               ),
               posted AS (
                   SELECT c.workspace_id, m.user_id, (m.created_at AT TIME ZONE 'UTC')::date AS day,
                          c.channel_type::text AS kind,
                          (SELECT COUNT(*) FROM files f WHERE f.message_id = m.id)::int AS files
                     FROM messages m
                     JOIN channels c ON c.id = m.channel_id
                     JOIN users u ON u.id = m.user_id
                    WHERE m.id = $2
                      AND m.created_at >= (SELECT counted_from FROM analytics_backfill)
                      AND m.metadata->>'kind' IS NULL
                      AND NOT (m.metadata ? 'bot')
                      AND NOT u.is_bot
                      AND EXISTS (SELECT 1 FROM fresh)
               )
               INSERT INTO member_daily_activity
                   (workspace_id, user_id, day, public_messages, private_messages, direct_messages, files)
               SELECT workspace_id, user_id, day,
                      (kind = 'public')::int, (kind = 'private')::int,
                      (kind IN ('dm', 'group_dm'))::int, files
                 FROM posted
               ON CONFLICT (workspace_id, user_id, day) DO UPDATE SET
                   public_messages = member_daily_activity.public_messages + EXCLUDED.public_messages,
                   private_messages = member_daily_activity.private_messages + EXCLUDED.private_messages,
                   direct_messages = member_daily_activity.direct_messages + EXCLUDED.direct_messages,
                   files = member_daily_activity.files + EXCLUDED.files"#,
            event_id,
            message_id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn count_reaction(&self, event_id: Uuid, reaction_id: Uuid) -> sqlx::Result<()> {
        sqlx::query!(
            r#"WITH fresh AS (
                   INSERT INTO analytics_counted_events (event_id) VALUES ($1)
                   ON CONFLICT DO NOTHING
                   RETURNING event_id
               )
               INSERT INTO member_daily_activity (workspace_id, user_id, day, reactions)
               SELECT c.workspace_id, r.user_id, (r.created_at AT TIME ZONE 'UTC')::date, 1
                 FROM reactions r
                 JOIN messages m ON m.id = r.message_id
                 JOIN channels c ON c.id = m.channel_id
                 JOIN users u ON u.id = r.user_id
                WHERE r.id = $2
                  AND r.created_at >= (SELECT counted_from FROM analytics_backfill)
                  AND NOT u.is_bot
                  AND EXISTS (SELECT 1 FROM fresh)
               ON CONFLICT (workspace_id, user_id, day) DO UPDATE SET
                   reactions = member_daily_activity.reactions + 1"#,
            event_id,
            reaction_id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn mark_active(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        day: NaiveDate,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            "INSERT INTO member_daily_activity (workspace_id, user_id, day)
             SELECT wm.workspace_id, wm.user_id, $3
               FROM workspace_members wm
               JOIN users u ON u.id = wm.user_id
              WHERE wm.workspace_id = $1 AND wm.user_id = $2 AND NOT u.is_bot
             ON CONFLICT DO NOTHING",
            workspace_id,
            user_id,
            day
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn forget_counted_before(&self, days: i32) -> sqlx::Result<u64> {
        let done = sqlx::query!(
            "DELETE FROM analytics_counted_events WHERE counted_at < NOW() - make_interval(days => $1)",
            days
        )
        .execute(&self.pool)
        .await?;
        Ok(done.rows_affected())
    }

    pub async fn backfill(&self) -> sqlx::Result<bool> {
        let mut tx = self.pool.begin().await?;
        let Some(counted_from) = sqlx::query_scalar!(
            "SELECT counted_from FROM analytics_backfill WHERE finished_at IS NULL FOR UPDATE SKIP LOCKED"
        )
        .fetch_optional(&mut *tx)
        .await?
        else {
            return Ok(false);
        };

        sqlx::query!(
            r#"INSERT INTO member_daily_activity
                   (workspace_id, user_id, day, public_messages, private_messages, direct_messages, files)
               SELECT c.workspace_id, m.user_id, (m.created_at AT TIME ZONE 'UTC')::date,
                      (COUNT(*) FILTER (WHERE c.channel_type = 'public'))::int,
                      (COUNT(*) FILTER (WHERE c.channel_type = 'private'))::int,
                      (COUNT(*) FILTER (WHERE c.channel_type IN ('dm', 'group_dm')))::int,
                      COALESCE(SUM(f.files), 0)::int
                 FROM messages m
                 JOIN channels c ON c.id = m.channel_id
                 JOIN users u ON u.id = m.user_id
                 LEFT JOIN LATERAL (
                     SELECT COUNT(*) AS files FROM files WHERE files.message_id = m.id
                 ) f ON TRUE
                WHERE m.created_at < $1
                  AND m.metadata->>'kind' IS NULL
                  AND NOT (m.metadata ? 'bot')
                  AND NOT u.is_bot
                GROUP BY 1, 2, 3
               ON CONFLICT (workspace_id, user_id, day) DO UPDATE SET
                   public_messages = member_daily_activity.public_messages + EXCLUDED.public_messages,
                   private_messages = member_daily_activity.private_messages + EXCLUDED.private_messages,
                   direct_messages = member_daily_activity.direct_messages + EXCLUDED.direct_messages,
                   files = member_daily_activity.files + EXCLUDED.files"#,
            counted_from
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query!(
            r#"INSERT INTO member_daily_activity (workspace_id, user_id, day, reactions)
               SELECT c.workspace_id, r.user_id, (r.created_at AT TIME ZONE 'UTC')::date, COUNT(*)::int
                 FROM reactions r
                 JOIN messages m ON m.id = r.message_id
                 JOIN channels c ON c.id = m.channel_id
                 JOIN users u ON u.id = r.user_id
                WHERE r.created_at < $1 AND NOT u.is_bot
                GROUP BY 1, 2, 3
               ON CONFLICT (workspace_id, user_id, day) DO UPDATE SET
                   reactions = member_daily_activity.reactions + EXCLUDED.reactions"#,
            counted_from
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query!("UPDATE analytics_backfill SET finished_at = NOW()")
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(true)
    }

    pub async fn daily(
        &self,
        workspace_id: Uuid,
        from: NaiveDate,
        to: NaiveDate,
        guests: bool,
    ) -> sqlx::Result<Vec<DailyActivity>> {
        sqlx::query_as!(
            DailyActivity,
            r#"WITH days AS (
                   SELECT d::date AS day FROM generate_series($2::date, $3::date, INTERVAL '1 day') d
               ),
               people AS (
                   SELECT wm.user_id, (wm.joined_at AT TIME ZONE 'UTC')::date AS joined_on
                     FROM workspace_members wm
                     JOIN users u ON u.id = wm.user_id
                    WHERE wm.workspace_id = $1 AND NOT u.is_bot AND ($4 OR wm.role <> 'guest')
               ),
               activity AS (
                   SELECT a.day, COUNT(*) AS active,
                          SUM(a.public_messages) AS public_messages,
                          SUM(a.private_messages) AS private_messages,
                          SUM(a.direct_messages) AS direct_messages,
                          SUM(a.files) AS files, SUM(a.reactions) AS reactions
                     FROM member_daily_activity a
                     LEFT JOIN workspace_members wm
                       ON wm.workspace_id = a.workspace_id AND wm.user_id = a.user_id
                    WHERE a.workspace_id = $1 AND a.day BETWEEN $2 AND $3
                      AND ($4 OR wm.role IS DISTINCT FROM 'guest')
                    GROUP BY a.day
               )
               SELECT days.day AS "day!",
                      (SELECT COUNT(*) FROM people p WHERE p.joined_on <= days.day) AS "members!",
                      COALESCE(activity.active, 0) AS "active_members!",
                      (SELECT COUNT(*) FROM people p WHERE p.joined_on = days.day) AS "joined!",
                      COALESCE(activity.public_messages, 0)::bigint AS "public_messages!",
                      COALESCE(activity.private_messages, 0)::bigint AS "private_messages!",
                      COALESCE(activity.direct_messages, 0)::bigint AS "direct_messages!",
                      COALESCE(activity.files, 0)::bigint AS "files!",
                      COALESCE(activity.reactions, 0)::bigint AS "reactions!"
                 FROM days
                 LEFT JOIN activity ON activity.day = days.day
                ORDER BY days.day"#,
            workspace_id,
            from,
            to,
            guests
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn weekly(
        &self,
        workspace_id: Uuid,
        from: NaiveDate,
        to: NaiveDate,
        guests: bool,
    ) -> sqlx::Result<Vec<WeeklyActivity>> {
        sqlx::query_as!(
            WeeklyActivity,
            r#"SELECT date_trunc('week', a.day)::date AS "week!",
                      COUNT(DISTINCT a.user_id) AS "active_members!"
                 FROM member_daily_activity a
                 LEFT JOIN workspace_members wm
                   ON wm.workspace_id = a.workspace_id AND wm.user_id = a.user_id
                WHERE a.workspace_id = $1 AND a.day BETWEEN $2 AND $3
                  AND ($4 OR wm.role IS DISTINCT FROM 'guest')
                GROUP BY 1
                ORDER BY 1"#,
            workspace_id,
            from,
            to,
            guests
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn active_between(
        &self,
        workspace_id: Uuid,
        from: NaiveDate,
        to: NaiveDate,
        guests: bool,
    ) -> sqlx::Result<i64> {
        sqlx::query_scalar!(
            r#"SELECT COUNT(DISTINCT a.user_id) AS "active!"
                 FROM member_daily_activity a
                 LEFT JOIN workspace_members wm
                   ON wm.workspace_id = a.workspace_id AND wm.user_id = a.user_id
                WHERE a.workspace_id = $1 AND a.day BETWEEN $2 AND $3
                  AND ($4 OR wm.role IS DISTINCT FROM 'guest')"#,
            workspace_id,
            from,
            to,
            guests
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn members(
        &self,
        workspace_id: Uuid,
        from: NaiveDate,
        to: NaiveDate,
        guests: bool,
    ) -> sqlx::Result<Vec<MemberActivity>> {
        sqlx::query_as!(
            MemberActivity,
            r#"SELECT wm.user_id, wm.role AS "role: WorkspaceRole",
                      COUNT(a.day) AS "days_active!",
                      COALESCE(SUM(a.public_messages + a.private_messages + a.direct_messages), 0)::bigint
                          AS "messages!",
                      (SELECT MAX(x.day) FROM member_daily_activity x
                        WHERE x.workspace_id = wm.workspace_id AND x.user_id = wm.user_id) AS last_active
                 FROM workspace_members wm
                 JOIN users u ON u.id = wm.user_id
                 LEFT JOIN member_daily_activity a
                   ON a.workspace_id = wm.workspace_id AND a.user_id = wm.user_id
                  AND a.day BETWEEN $2 AND $3
                WHERE wm.workspace_id = $1 AND NOT u.is_bot AND ($4 OR wm.role <> 'guest')
                GROUP BY wm.workspace_id, wm.user_id, wm.role
                ORDER BY 3 DESC, wm.user_id"#,
            workspace_id,
            from,
            to,
            guests
        )
        .fetch_all(&self.pool)
        .await
    }
}
