use sqlx::PgPool;
use uuid::Uuid;

pub async fn may_join_huddle(db: &PgPool, huddle_id: Uuid, user_id: Uuid) -> sqlx::Result<bool> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS(
               SELECT 1 FROM huddle_sessions h
                WHERE h.id = $1 AND h.ended_at IS NULL
                  AND EXISTS(SELECT 1 FROM workspace_members wm
                              WHERE wm.workspace_id = h.workspace_id AND wm.user_id = $2)
                  AND (
                      (h.channel_id IS NOT NULL AND EXISTS(
                          SELECT 1 FROM channel_members cm
                           WHERE cm.channel_id = h.channel_id AND cm.user_id = $2))
                      OR (h.channel_id IS NULL AND $2 IN (h.initiated_by, h.dm_partner_id))
                      OR EXISTS(SELECT 1 FROM huddle_invites i
                                 WHERE i.huddle_id = h.id AND i.user_id = $2)
                  )
           ) AS "exists!""#,
        huddle_id,
        user_id
    )
    .fetch_one(db)
    .await
}
