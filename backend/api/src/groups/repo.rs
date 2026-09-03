use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct UserGroup {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub handle: String,
    pub name: String,
    pub description: Option<String>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct UserGroupSummary {
    pub id: Uuid,
    pub handle: String,
    pub name: String,
    pub description: Option<String>,
    pub member_count: i64,
    /// Whether the person asking is in it. The client highlights a mention of
    /// their own group the way it highlights their own name, and it cannot work
    /// that out from a handle.
    pub is_member: bool,
}

#[derive(Clone)]
pub struct GroupRepo {
    pool: PgPool,
}

impl GroupRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        workspace_id: Uuid,
        handle: &str,
        name: &str,
        description: Option<&str>,
        created_by: Uuid,
    ) -> sqlx::Result<UserGroup> {
        sqlx::query_as!(
            UserGroup,
            r"
            INSERT INTO user_groups (workspace_id, handle, name, description, created_by)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, workspace_id, handle, name, description, created_by, created_at, updated_at
            ",
            workspace_id,
            handle,
            name,
            description,
            created_by
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn list(
        &self,
        workspace_id: Uuid,
        requester_id: Uuid,
    ) -> sqlx::Result<Vec<UserGroupSummary>> {
        sqlx::query_as!(
            UserGroupSummary,
            r#"
            SELECT g.id, g.handle, g.name, g.description,
                   COUNT(m.user_id) AS "member_count!",
                   BOOL_OR(m.user_id = $2) IS TRUE AS "is_member!"
              FROM user_groups g
              LEFT JOIN user_group_members m ON m.group_id = g.id
             WHERE g.workspace_id = $1
             GROUP BY g.id
             ORDER BY g.handle
            "#,
            workspace_id,
            requester_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn find(&self, id: Uuid) -> sqlx::Result<Option<UserGroup>> {
        sqlx::query_as!(
            UserGroup,
            "SELECT id, workspace_id, handle, name, description, created_by, created_at, updated_at FROM user_groups WHERE id = $1",
            id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn update(
        &self,
        id: Uuid,
        name: &str,
        description: Option<&str>,
    ) -> sqlx::Result<UserGroup> {
        sqlx::query_as!(
            UserGroup,
            r"
            UPDATE user_groups
               SET name = $2, description = $3, updated_at = NOW()
             WHERE id = $1
            RETURNING id, workspace_id, handle, name, description, created_by, created_at, updated_at
            ",
            id,
            name,
            description
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn delete(&self, id: Uuid) -> sqlx::Result<()> {
        sqlx::query!("DELETE FROM user_groups WHERE id = $1", id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn add_member(&self, group_id: Uuid, user_id: Uuid) -> sqlx::Result<()> {
        sqlx::query!(
            "INSERT INTO user_group_members (group_id, user_id) VALUES ($1, $2) \
             ON CONFLICT DO NOTHING",
            group_id,
            user_id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn remove_member(&self, group_id: Uuid, user_id: Uuid) -> sqlx::Result<()> {
        sqlx::query!(
            "DELETE FROM user_group_members WHERE group_id = $1 AND user_id = $2",
            group_id,
            user_id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_member_ids(&self, group_id: Uuid) -> sqlx::Result<Vec<Uuid>> {
        sqlx::query_scalar!(
            "SELECT user_id FROM user_group_members WHERE group_id = $1",
            group_id
        )
        .fetch_all(&self.pool)
        .await
    }

    /// One statement for however many groups a message mentions. The mention
    /// path runs on every send, so a query per handle would be an N+1 on the
    /// hottest write in the product.
    pub async fn member_ids_for_groups(
        &self,
        workspace_id: Uuid,
        group_ids: &[Uuid],
    ) -> sqlx::Result<Vec<Uuid>> {
        if group_ids.is_empty() {
            return Ok(vec![]);
        }
        sqlx::query_scalar!(
            r"
            SELECT DISTINCT m.user_id
              FROM user_group_members m
              JOIN user_groups g ON g.id = m.group_id
             WHERE g.workspace_id = $1
               AND g.id = ANY($2)
            ",
            workspace_id,
            group_ids
        )
        .fetch_all(&self.pool)
        .await
    }
}
