use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::models::WorkspaceProfile;

#[derive(Clone)]
pub struct ProfileRepo {
    pool: PgPool,
}

impl ProfileRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn get(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<Option<WorkspaceProfile>> {
        sqlx::query_as!(
            WorkspaceProfile,
            r#"
            SELECT workspace_id, display_name, avatar_url, bio,
                   CASE WHEN status_expires_at IS NULL OR status_expires_at > NOW()
                        THEN status_emoji END AS "status_emoji?",
                   CASE WHEN status_expires_at IS NULL OR status_expires_at > NOW()
                        THEN status_text END AS "status_text?",
                   CASE WHEN status_expires_at > NOW()
                        THEN status_expires_at END AS "status_expires_at?"
              FROM workspace_members
             WHERE workspace_id = $1 AND user_id = $2
            "#,
            workspace_id,
            user_id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn update(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        display_name: Option<&str>,
        avatar_url: Option<&str>,
        bio: Option<&str>,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            r#"
            UPDATE workspace_members
               SET display_name = CASE WHEN $3::text IS NULL THEN display_name
                                       ELSE NULLIF(BTRIM($3), '') END,
                   avatar_url = CASE WHEN $4::text IS NULL THEN avatar_url
                                     ELSE NULLIF(BTRIM($4), '') END,
                   bio = CASE WHEN $5::text IS NULL THEN bio
                              ELSE NULLIF(BTRIM($5), '') END
             WHERE workspace_id = $1 AND user_id = $2
            "#,
            workspace_id,
            user_id,
            display_name,
            avatar_url,
            bio
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn set_status(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        emoji: Option<&str>,
        text: Option<&str>,
        expires_at: Option<DateTime<Utc>>,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            r#"
            UPDATE workspace_members
               SET status_emoji = $3, status_text = $4, status_expires_at = $5
             WHERE workspace_id = $1 AND user_id = $2
            "#,
            workspace_id,
            user_id,
            emoji,
            text,
            expires_at
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn names(
        &self,
        workspace_id: Uuid,
        user_ids: &[Uuid],
    ) -> sqlx::Result<Vec<(Uuid, String)>> {
        let rows = sqlx::query!(
            r#"
            SELECT u.id AS "id!",
                   COALESCE(wm.display_name, u.display_name, split_part(u.email, '@', 1)) AS "name!"
              FROM users u
              LEFT JOIN workspace_members wm ON wm.user_id = u.id AND wm.workspace_id = $1
             WHERE u.id = ANY($2)
            "#,
            workspace_id,
            user_ids
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|row| (row.id, row.name)).collect())
    }

    pub async fn name(&self, workspace_id: Uuid, user_id: Uuid) -> sqlx::Result<Option<String>> {
        Ok(self
            .names(workspace_id, &[user_id])
            .await?
            .into_iter()
            .next()
            .map(|(_, name)| name))
    }
}
