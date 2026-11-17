use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone)]
pub struct BlockRepo {
    pool: PgPool,
}

impl BlockRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn member_names(
        &self,
        workspace_id: Uuid,
        user_ids: &[Uuid],
    ) -> sqlx::Result<Vec<(Uuid, String)>> {
        let rows = sqlx::query!(
            r#"
            SELECT u.id, COALESCE(wm.display_name, u.display_name, '') AS "name!"
              FROM workspace_members wm
              JOIN users u ON u.id = wm.user_id
             WHERE wm.workspace_id = $1 AND u.id = ANY($2)
            "#,
            workspace_id,
            user_ids
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| (r.id, r.name)).collect())
    }

    pub async fn public_channel_names(
        &self,
        workspace_id: Uuid,
        channel_ids: &[Uuid],
    ) -> sqlx::Result<Vec<(Uuid, String)>> {
        let rows = sqlx::query!(
            r#"
            SELECT id, COALESCE(name, '') AS "name!"
              FROM channels
             WHERE workspace_id = $1 AND id = ANY($2) AND channel_type = 'public'
            "#,
            workspace_id,
            channel_ids
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| (r.id, r.name)).collect())
    }
}
