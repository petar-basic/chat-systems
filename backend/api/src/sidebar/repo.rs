use sqlx::PgPool;
use uuid::Uuid;

use super::models::*;

#[derive(Clone)]
pub struct SidebarRepo {
    pool: PgPool,
}

impl SidebarRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn sections(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
    ) -> sqlx::Result<Vec<SidebarSection>> {
        sqlx::query_as!(
            SidebarSection,
            "SELECT id, name, position, collapsed FROM sidebar_sections
              WHERE user_id = $1 AND workspace_id = $2
              ORDER BY position, created_at",
            user_id,
            workspace_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn placements(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
    ) -> sqlx::Result<Vec<ChannelPlacement>> {
        sqlx::query_as!(
            ChannelPlacement,
            r#"SELECT cm.channel_id, cm.section_id AS "section_id!"
                 FROM channel_members cm
                 JOIN channels c ON c.id = cm.channel_id
                WHERE cm.user_id = $1 AND c.workspace_id = $2 AND cm.section_id IS NOT NULL"#,
            user_id,
            workspace_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn collapsed_builtins(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
    ) -> sqlx::Result<Vec<String>> {
        sqlx::query_scalar!(
            "SELECT section FROM sidebar_collapsed_builtins WHERE user_id = $1 AND workspace_id = $2",
            user_id,
            workspace_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn count(&self, user_id: Uuid, workspace_id: Uuid) -> sqlx::Result<i64> {
        sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "count!" FROM sidebar_sections WHERE user_id = $1 AND workspace_id = $2"#,
            user_id,
            workspace_id
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn create(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        name: &str,
    ) -> sqlx::Result<SidebarSection> {
        sqlx::query_as!(
            SidebarSection,
            r#"INSERT INTO sidebar_sections (user_id, workspace_id, name, position)
               SELECT $1, $2, $3, COALESCE(MAX(position) + 1, 0)
                 FROM sidebar_sections WHERE user_id = $1 AND workspace_id = $2
               RETURNING id, name, position, collapsed"#,
            user_id,
            workspace_id,
            name
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn update(
        &self,
        section_id: Uuid,
        user_id: Uuid,
        workspace_id: Uuid,
        name: Option<&str>,
        collapsed: Option<bool>,
    ) -> sqlx::Result<Option<SidebarSection>> {
        sqlx::query_as!(
            SidebarSection,
            "UPDATE sidebar_sections
                SET name = COALESCE($4, name), collapsed = COALESCE($5, collapsed)
              WHERE id = $1 AND user_id = $2 AND workspace_id = $3
              RETURNING id, name, position, collapsed",
            section_id,
            user_id,
            workspace_id,
            name,
            collapsed
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn delete(
        &self,
        section_id: Uuid,
        user_id: Uuid,
        workspace_id: Uuid,
    ) -> sqlx::Result<bool> {
        let result = sqlx::query!(
            "DELETE FROM sidebar_sections WHERE id = $1 AND user_id = $2 AND workspace_id = $3",
            section_id,
            user_id,
            workspace_id
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn reorder(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        section_ids: &[Uuid],
    ) -> sqlx::Result<()> {
        sqlx::query!(
            "UPDATE sidebar_sections s
                SET position = ordered.position - 1
               FROM unnest($3::uuid[]) WITH ORDINALITY AS ordered(id, position)
              WHERE s.id = ordered.id AND s.user_id = $1 AND s.workspace_id = $2",
            user_id,
            workspace_id,
            section_ids
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn place_channel(
        &self,
        channel_id: Uuid,
        user_id: Uuid,
        section_id: Option<Uuid>,
    ) -> sqlx::Result<bool> {
        let result = sqlx::query!(
            "UPDATE channel_members cm
                SET section_id = $3
               FROM channels c
              WHERE cm.channel_id = $1
                AND cm.user_id = $2
                AND c.id = cm.channel_id
                AND (
                    $3::uuid IS NULL
                    OR EXISTS (
                        SELECT 1 FROM sidebar_sections s
                         WHERE s.id = $3 AND s.user_id = $2 AND s.workspace_id = c.workspace_id
                    )
                )",
            channel_id,
            user_id,
            section_id
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn set_builtin_collapsed(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        section: &str,
        collapsed: bool,
    ) -> sqlx::Result<()> {
        if collapsed {
            sqlx::query!(
                "INSERT INTO sidebar_collapsed_builtins (user_id, workspace_id, section)
                 VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
                user_id,
                workspace_id,
                section
            )
            .execute(&self.pool)
            .await?;
        } else {
            sqlx::query!(
                "DELETE FROM sidebar_collapsed_builtins
                  WHERE user_id = $1 AND workspace_id = $2 AND section = $3",
                user_id,
                workspace_id,
                section
            )
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }
}
