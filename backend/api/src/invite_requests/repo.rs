use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::models::{ExpiredRequest, InviteRequestStatus, WorkspaceInviteRequest, LIFETIME_DAYS};
use crate::guests::service::InviteTerms;
use crate::workspace::models::{GuestKind, WorkspaceRole};

pub struct NewRequest<'a> {
    pub workspace_id: Uuid,
    pub requested_by: Uuid,
    pub emails: &'a [String],
    pub terms: &'a InviteTerms,
    pub reason: Option<&'a str>,
}

#[derive(Clone)]
pub struct InviteRequestRepo {
    pool: PgPool,
}

impl InviteRequestRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn lock_workspace_in(
        conn: &mut PgConnection,
        workspace_id: Uuid,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            "SELECT pg_advisory_xact_lock(hashtextextended('invite_requests:' || $1::uuid::text, 0))",
            workspace_id
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    pub async fn members_among_in(
        conn: &mut PgConnection,
        workspace_id: Uuid,
        emails: &[String],
    ) -> sqlx::Result<Vec<String>> {
        sqlx::query_scalar!(
            r#"SELECT lower(u.email) AS "email!"
                 FROM workspace_members wm
                 JOIN users u ON u.id = wm.user_id
                WHERE wm.workspace_id = $1 AND lower(u.email) = ANY($2)
                ORDER BY 1"#,
            workspace_id,
            emails
        )
        .fetch_all(&mut *conn)
        .await
    }

    pub async fn waiting_among_in(
        conn: &mut PgConnection,
        workspace_id: Uuid,
        emails: &[String],
    ) -> sqlx::Result<Vec<String>> {
        sqlx::query_scalar!(
            r#"SELECT DISTINCT e AS "email!"
                 FROM invite_requests r, unnest(r.emails) e
                WHERE r.workspace_id = $1 AND r.status = 'pending' AND e = ANY($2)
                ORDER BY 1"#,
            workspace_id,
            emails
        )
        .fetch_all(&mut *conn)
        .await
    }

    pub async fn create_in(conn: &mut PgConnection, new: NewRequest<'_>) -> sqlx::Result<Uuid> {
        let id = sqlx::query_scalar!(
            "INSERT INTO invite_requests
                (workspace_id, requested_by, emails, role, guest_kind, guest_until, reason)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             RETURNING id",
            new.workspace_id,
            new.requested_by,
            new.emails,
            new.terms.role.clone() as WorkspaceRole,
            new.terms.guest.map(|g| g.kind) as Option<GuestKind>,
            new.terms.guest.and_then(|g| g.until),
            new.reason
        )
        .fetch_one(&mut *conn)
        .await?;
        Self::set_channels_in(conn, id, &new.terms.channel_ids).await?;
        Ok(id)
    }

    async fn set_channels_in(
        conn: &mut PgConnection,
        request_id: Uuid,
        channel_ids: &[Uuid],
    ) -> sqlx::Result<()> {
        sqlx::query!(
            "DELETE FROM invite_request_channels WHERE request_id = $1",
            request_id
        )
        .execute(&mut *conn)
        .await?;
        sqlx::query!(
            "INSERT INTO invite_request_channels (request_id, channel_id)
             SELECT $1, unnest($2::uuid[]) ON CONFLICT DO NOTHING",
            request_id,
            channel_ids
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    pub async fn approve_in(
        conn: &mut PgConnection,
        request_id: Uuid,
        admin: Uuid,
        emails: &[String],
        terms: &InviteTerms,
    ) -> sqlx::Result<bool> {
        let claimed = sqlx::query!(
            "UPDATE invite_requests
                SET status = 'approved', decided_by = $2, decided_at = NOW(),
                    emails = $3, role = $4, guest_kind = $5, guest_until = $6
              WHERE id = $1 AND status = 'pending'",
            request_id,
            admin,
            emails,
            terms.role.clone() as WorkspaceRole,
            terms.guest.map(|g| g.kind) as Option<GuestKind>,
            terms.guest.and_then(|g| g.until)
        )
        .execute(&mut *conn)
        .await?
        .rows_affected()
            == 1;
        if claimed {
            Self::set_channels_in(conn, request_id, &terms.channel_ids).await?;
        }
        Ok(claimed)
    }

    pub async fn decline(
        &self,
        request_id: Uuid,
        admin: Uuid,
        note: Option<&str>,
    ) -> sqlx::Result<bool> {
        Ok(sqlx::query!(
            "UPDATE invite_requests
                SET status = 'declined', decided_by = $2, decided_at = NOW(), note = $3
              WHERE id = $1 AND status = 'pending'",
            request_id,
            admin,
            note
        )
        .execute(&self.pool)
        .await?
        .rows_affected()
            == 1)
    }

    pub async fn expire_due(&self, limit: i64) -> sqlx::Result<Vec<ExpiredRequest>> {
        sqlx::query_as!(
            ExpiredRequest,
            "UPDATE invite_requests
                SET status = 'expired', decided_at = NOW()
              WHERE id IN (
                    SELECT id FROM invite_requests
                     WHERE status = 'pending'
                       AND created_at <= NOW() - make_interval(days => $1)
                     ORDER BY created_at
                     LIMIT $2
                     FOR UPDATE SKIP LOCKED)
             RETURNING id, workspace_id, requested_by, emails",
            LIFETIME_DAYS,
            limit
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn find(&self, request_id: Uuid) -> sqlx::Result<Option<WorkspaceInviteRequest>> {
        sqlx::query_as!(
            WorkspaceInviteRequest,
            r#"SELECT r.id, r.workspace_id, r.requested_by,
                      COALESCE(NULLIF(btrim(wm.display_name), ''), NULLIF(btrim(u.display_name), ''), u.email) AS "requester_name!",
                      r.emails, r.role AS "role: WorkspaceRole",
                      r.guest_kind AS "guest_kind: GuestKind", r.guest_until,
                      ARRAY(SELECT c.id FROM invite_request_channels rc JOIN channels c ON c.id = rc.channel_id
                             WHERE rc.request_id = r.id ORDER BY c.name, c.id) AS "channel_ids!",
                      ARRAY(SELECT COALESCE(c.name, '') FROM invite_request_channels rc JOIN channels c ON c.id = rc.channel_id
                             WHERE rc.request_id = r.id ORDER BY c.name, c.id) AS "channel_names!",
                      r.reason, r.status AS "status: InviteRequestStatus",
                      r.decided_by, r.decided_at, r.note, r.created_at,
                      r.created_at + make_interval(days => $2) AS "expires_at!"
                 FROM invite_requests r
                 JOIN users u ON u.id = r.requested_by
                 LEFT JOIN workspace_members wm ON wm.workspace_id = r.workspace_id AND wm.user_id = r.requested_by
                WHERE r.id = $1"#,
            request_id,
            LIFETIME_DAYS
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn pending(
        &self,
        workspace_id: Uuid,
        requested_by: Option<Uuid>,
    ) -> sqlx::Result<Vec<WorkspaceInviteRequest>> {
        sqlx::query_as!(
            WorkspaceInviteRequest,
            r#"SELECT r.id, r.workspace_id, r.requested_by,
                      COALESCE(NULLIF(btrim(wm.display_name), ''), NULLIF(btrim(u.display_name), ''), u.email) AS "requester_name!",
                      r.emails, r.role AS "role: WorkspaceRole",
                      r.guest_kind AS "guest_kind: GuestKind", r.guest_until,
                      ARRAY(SELECT c.id FROM invite_request_channels rc JOIN channels c ON c.id = rc.channel_id
                             WHERE rc.request_id = r.id ORDER BY c.name, c.id) AS "channel_ids!",
                      ARRAY(SELECT COALESCE(c.name, '') FROM invite_request_channels rc JOIN channels c ON c.id = rc.channel_id
                             WHERE rc.request_id = r.id ORDER BY c.name, c.id) AS "channel_names!",
                      r.reason, r.status AS "status: InviteRequestStatus",
                      r.decided_by, r.decided_at, r.note, r.created_at,
                      r.created_at + make_interval(days => $3) AS "expires_at!"
                 FROM invite_requests r
                 JOIN users u ON u.id = r.requested_by
                 LEFT JOIN workspace_members wm ON wm.workspace_id = r.workspace_id AND wm.user_id = r.requested_by
                WHERE r.workspace_id = $1 AND r.status = 'pending'
                  AND ($2::uuid IS NULL OR r.requested_by = $2)
                ORDER BY r.created_at, r.id"#,
            workspace_id,
            requested_by,
            LIFETIME_DAYS
        )
        .fetch_all(&self.pool)
        .await
    }
}
