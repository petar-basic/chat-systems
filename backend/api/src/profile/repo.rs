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
            SELECT u.id,
                   COALESCE(
                       (SELECT wm.display_name FROM workspace_members wm
                         WHERE wm.workspace_id = $1 AND wm.user_id = u.id),
                       u.display_name, split_part(u.email, '@', 1)
                   ) AS "name!"
              FROM users u
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

    pub async fn shares_a_workspace(&self, a: Uuid, b: Uuid) -> sqlx::Result<bool> {
        sqlx::query_scalar!(
            r#"
            SELECT EXISTS(
              SELECT 1 FROM workspace_members mine
                JOIN workspace_members theirs ON theirs.workspace_id = mine.workspace_id
               WHERE mine.user_id = $1 AND theirs.user_id = $2
            ) AS "exists!"
            "#,
            a,
            b
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn photos_outside_avatars(&self) -> sqlx::Result<Vec<(Uuid, String)>> {
        let rows = sqlx::query!(
            r#"
            SELECT id, avatar_url AS "avatar_url!" FROM users
             WHERE avatar_url LIKE '%/api/files/download/%'
               AND avatar_url NOT LIKE '%/api/files/download/avatars/%'
            "#
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| (row.id, row.avatar_url))
            .collect())
    }

    pub async fn workspace_photos(&self, user_id: Uuid) -> sqlx::Result<Vec<(Uuid, String)>> {
        let rows = sqlx::query!(
            r#"SELECT workspace_id, avatar_url AS "avatar_url!" FROM workspace_members
                WHERE user_id = $1 AND avatar_url IS NOT NULL"#,
            user_id
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| (row.workspace_id, row.avatar_url))
            .collect())
    }

    pub async fn workspace_photos_outside_avatars(
        &self,
    ) -> sqlx::Result<Vec<(Uuid, Uuid, String)>> {
        let rows = sqlx::query!(
            r#"
            SELECT workspace_id, user_id, avatar_url AS "avatar_url!" FROM workspace_members
             WHERE avatar_url LIKE '%/api/files/download/%'
               AND avatar_url NOT LIKE '%/api/files/download/avatars/%'
            "#
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| (row.workspace_id, row.user_id, row.avatar_url))
            .collect())
    }

    pub async fn move_workspace_photo(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        from: &str,
        to: &str,
    ) -> sqlx::Result<bool> {
        let done = sqlx::query!(
            "UPDATE workspace_members SET avatar_url = $4 WHERE workspace_id = $1 AND user_id = $2 AND avatar_url = $3",
            workspace_id,
            user_id,
            from,
            to
        )
        .execute(&self.pool)
        .await?;
        Ok(done.rows_affected() == 1)
    }

    pub async fn move_photo(&self, user_id: Uuid, from: &str, to: &str) -> sqlx::Result<bool> {
        let done = sqlx::query!(
            "UPDATE users SET avatar_url = $3 WHERE id = $1 AND avatar_url = $2",
            user_id,
            from,
            to
        )
        .execute(&self.pool)
        .await?;
        Ok(done.rows_affected() == 1)
    }
}
