use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::workspace::models::{GuestKind, GuestSeat};

pub struct ExpiredGuest {
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub guest_until: DateTime<Utc>,
}

pub struct EndingGuest {
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub guest_until: DateTime<Utc>,
    pub invited_by: Option<Uuid>,
}

#[derive(Clone)]
pub struct GuestRepo {
    pool: PgPool,
}

impl GuestRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn single_guests_elsewhere_in(
        conn: &mut PgConnection,
        workspace_id: Uuid,
        user_ids: &[Uuid],
        channel_id: Uuid,
    ) -> sqlx::Result<Vec<String>> {
        sqlx::query!(
            "SELECT user_id FROM workspace_members
              WHERE workspace_id = $1 AND user_id = ANY($2)
              ORDER BY user_id
                FOR UPDATE",
            workspace_id,
            user_ids
        )
        .fetch_all(&mut *conn)
        .await?;
        sqlx::query_scalar!(
            r#"SELECT COALESCE(wm.display_name, u.display_name, u.email) AS "name!"
                 FROM workspace_members wm
                 JOIN users u ON u.id = wm.user_id
                WHERE wm.workspace_id = $1
                  AND wm.user_id = ANY($2)
                  AND wm.guest_kind = 'single'
                  AND EXISTS (
                      SELECT 1
                        FROM channel_members cm
                        JOIN channels c ON c.id = cm.channel_id
                       WHERE cm.user_id = wm.user_id
                         AND c.workspace_id = $1
                         AND c.id <> $3
                         AND c.channel_type IN ('public', 'private')
                         AND NOT c.is_archived
                  )
                ORDER BY 1"#,
            workspace_id,
            user_ids,
            channel_id
        )
        .fetch_all(&mut *conn)
        .await
    }

    pub async fn lock_seat_in(
        conn: &mut PgConnection,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<Option<GuestSeat>> {
        sqlx::query_as!(
            GuestSeat,
            r#"SELECT user_id, guest_kind AS "guest_kind!: GuestKind", guest_until
                 FROM workspace_members
                WHERE workspace_id = $1 AND user_id = $2 AND role = 'guest'
                  FOR UPDATE"#,
            workspace_id,
            user_id
        )
        .fetch_optional(conn)
        .await
    }

    pub async fn channel_count_in(
        conn: &mut PgConnection,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<i64> {
        sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "count!"
                 FROM channel_members cm
                 JOIN channels c ON c.id = cm.channel_id
                WHERE cm.user_id = $2
                  AND c.workspace_id = $1
                  AND c.channel_type IN ('public', 'private')
                  AND NOT c.is_archived"#,
            workspace_id,
            user_id
        )
        .fetch_one(conn)
        .await
    }

    pub async fn set_seat_in(
        conn: &mut PgConnection,
        workspace_id: Uuid,
        user_id: Uuid,
        kind: GuestKind,
        until: Option<DateTime<Utc>>,
    ) -> sqlx::Result<GuestSeat> {
        sqlx::query_as!(
            GuestSeat,
            r#"UPDATE workspace_members
                  SET guest_kind = $3,
                      guest_until = $4,
                      guest_warned_at = CASE WHEN guest_until IS NOT DISTINCT FROM $4
                                             THEN guest_warned_at END
                WHERE workspace_id = $1 AND user_id = $2 AND role = 'guest'
            RETURNING user_id, guest_kind AS "guest_kind!: GuestKind", guest_until"#,
            workspace_id,
            user_id,
            kind as GuestKind,
            until
        )
        .fetch_one(conn)
        .await
    }

    pub async fn next_expired_in(conn: &mut PgConnection) -> sqlx::Result<Option<ExpiredGuest>> {
        sqlx::query_as!(
            ExpiredGuest,
            r#"SELECT workspace_id, user_id, guest_until AS "guest_until!"
                 FROM workspace_members
                WHERE role = 'guest' AND guest_until <= NOW()
                ORDER BY guest_until
                LIMIT 1
                  FOR UPDATE SKIP LOCKED"#
        )
        .fetch_optional(conn)
        .await
    }

    pub async fn claim_ending(&self, within_days: i32) -> sqlx::Result<Vec<EndingGuest>> {
        sqlx::query_as!(
            EndingGuest,
            r#"UPDATE workspace_members wm
                  SET guest_warned_at = NOW()
                 FROM users u
                WHERE u.id = wm.user_id
                  AND wm.role = 'guest'
                  AND wm.guest_warned_at IS NULL
                  AND wm.guest_until > NOW()
                  AND wm.guest_until <= NOW() + make_interval(days => $1)
            RETURNING wm.workspace_id, wm.user_id,
                      COALESCE(wm.display_name, u.display_name, u.email) AS "name!",
                      wm.guest_until AS "guest_until!",
                      wm.guest_invited_by AS invited_by"#,
            within_days
        )
        .fetch_all(&self.pool)
        .await
    }
}
