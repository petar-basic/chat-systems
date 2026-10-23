use sqlx::types::Json;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::models::{ColumnKind, ColumnOptions, List, ListColumn, ListItem};

#[derive(Clone)]
pub struct ListRepo {
    pool: PgPool,
}

pub struct Visibility {
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub is_guest: bool,
    pub channel_id: Option<Uuid>,
}

impl ListRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_in(
        &self,
        conn: &mut PgConnection,
        workspace_id: Uuid,
        channel_id: Option<Uuid>,
        title: &str,
        description: &str,
        created_by: Uuid,
    ) -> sqlx::Result<List> {
        sqlx::query_as!(
            List,
            "INSERT INTO lists (workspace_id, channel_id, title, description, created_by)
             VALUES ($1, $2, $3, $4, $5)
             RETURNING id, workspace_id, channel_id, title, description, created_by, created_at, updated_at",
            workspace_id,
            channel_id,
            title,
            description,
            created_by
        )
        .fetch_one(conn)
        .await
    }

    pub async fn find(&self, id: Uuid) -> sqlx::Result<Option<List>> {
        sqlx::query_as!(
            List,
            "SELECT id, workspace_id, channel_id, title, description, created_by, created_at, updated_at
               FROM lists WHERE id = $1 AND deleted_at IS NULL",
            id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn lock_in(&self, conn: &mut PgConnection, id: Uuid) -> sqlx::Result<Option<List>> {
        sqlx::query_as!(
            List,
            "SELECT id, workspace_id, channel_id, title, description, created_by, created_at, updated_at
               FROM lists WHERE id = $1 AND deleted_at IS NULL
                FOR UPDATE",
            id
        )
        .fetch_optional(conn)
        .await
    }

    pub async fn share_lock_in(&self, conn: &mut PgConnection, id: Uuid) -> sqlx::Result<bool> {
        let found = sqlx::query_scalar!(
            "SELECT 1 AS \"found!\" FROM lists WHERE id = $1 AND deleted_at IS NULL FOR SHARE",
            id
        )
        .fetch_optional(conn)
        .await?;
        Ok(found.is_some())
    }

    pub async fn visible(&self, scope: &Visibility) -> sqlx::Result<Vec<List>> {
        sqlx::query_as!(
            List,
            r#"SELECT l.id, l.workspace_id, l.channel_id, l.title, l.description, l.created_by,
                      l.created_at, l.updated_at
                 FROM lists l
                WHERE l.workspace_id = $1
                  AND l.deleted_at IS NULL
                  AND (
                      (l.channel_id IS NULL AND l.created_by = $2 AND $3::uuid IS NULL)
                      OR l.channel_id IN (SELECT searchable_channels($1, $2, $3, $4, TRUE, TRUE))
                  )
                ORDER BY l.created_at DESC"#,
            scope.workspace_id,
            scope.user_id,
            scope.channel_id,
            scope.is_guest
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn update_in(
        &self,
        conn: &mut PgConnection,
        id: Uuid,
        title: &str,
        description: &str,
    ) -> sqlx::Result<List> {
        sqlx::query_as!(
            List,
            "UPDATE lists SET title = $2, description = $3, updated_at = NOW()
              WHERE id = $1
             RETURNING id, workspace_id, channel_id, title, description, created_by, created_at, updated_at",
            id,
            title,
            description
        )
        .fetch_one(conn)
        .await
    }

    pub async fn soft_delete_in(&self, conn: &mut PgConnection, id: Uuid) -> sqlx::Result<()> {
        sqlx::query!(
            "UPDATE lists SET deleted_at = NOW(), updated_at = NOW() WHERE id = $1",
            id
        )
        .execute(conn)
        .await?;
        Ok(())
    }

    pub async fn columns(&self, list_id: Uuid) -> sqlx::Result<Vec<ListColumn>> {
        let mut conn = self.pool.acquire().await?;
        self.columns_in(&mut conn, list_id).await
    }

    pub async fn columns_in(
        &self,
        conn: &mut PgConnection,
        list_id: Uuid,
    ) -> sqlx::Result<Vec<ListColumn>> {
        sqlx::query_as!(
            ListColumn,
            r#"SELECT id, list_id, position, name, kind AS "kind: ColumnKind",
                      options AS "options: Json<ColumnOptions>", hidden
                 FROM list_columns WHERE list_id = $1
                ORDER BY position, id"#,
            list_id
        )
        .fetch_all(conn)
        .await
    }

    pub async fn column_list(&self, column_id: Uuid) -> sqlx::Result<Option<Uuid>> {
        sqlx::query_scalar!(
            "SELECT c.list_id FROM list_columns c
               JOIN lists l ON l.id = c.list_id
              WHERE c.id = $1 AND l.deleted_at IS NULL",
            column_id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn insert_column_in(
        &self,
        conn: &mut PgConnection,
        list_id: Uuid,
        name: &str,
        kind: ColumnKind,
        options: &ColumnOptions,
    ) -> sqlx::Result<ListColumn> {
        let options = serde_json::to_value(options).unwrap_or_default();
        sqlx::query_as!(
            ListColumn,
            r#"INSERT INTO list_columns (list_id, position, name, kind, options)
               VALUES ($1,
                       (SELECT COALESCE(MAX(position) + 1, 0) FROM list_columns WHERE list_id = $1),
                       $2, $3, $4)
               RETURNING id, list_id, position, name, kind AS "kind: ColumnKind",
                         options AS "options: Json<ColumnOptions>", hidden"#,
            list_id,
            name,
            kind as ColumnKind,
            options
        )
        .fetch_one(conn)
        .await
    }

    pub async fn update_column_in(
        &self,
        conn: &mut PgConnection,
        id: Uuid,
        name: &str,
        kind: ColumnKind,
        options: &ColumnOptions,
        hidden: bool,
    ) -> sqlx::Result<()> {
        let options = serde_json::to_value(options).unwrap_or_default();
        sqlx::query!(
            "UPDATE list_columns SET name = $2, kind = $3, options = $4, hidden = $5 WHERE id = $1",
            id,
            name,
            kind as ColumnKind,
            options,
            hidden
        )
        .execute(conn)
        .await?;
        Ok(())
    }

    pub async fn delete_column_in(
        &self,
        conn: &mut PgConnection,
        list_id: Uuid,
        id: Uuid,
    ) -> sqlx::Result<()> {
        sqlx::query!("DELETE FROM list_columns WHERE id = $1", id)
            .execute(&mut *conn)
            .await?;
        sqlx::query!(
            r#"UPDATE list_items SET "values" = "values" - $2::text
                WHERE list_id = $1 AND "values" ? $2::text"#,
            list_id,
            id.to_string()
        )
        .execute(conn)
        .await?;
        Ok(())
    }

    pub async fn reorder_columns_in(
        &self,
        conn: &mut PgConnection,
        list_id: Uuid,
        ids: &[Uuid],
    ) -> sqlx::Result<()> {
        sqlx::query!(
            "UPDATE list_columns c SET position = (o.position - 1)::int
               FROM unnest($2::uuid[]) WITH ORDINALITY AS o(id, position)
              WHERE c.id = o.id AND c.list_id = $1",
            list_id,
            ids
        )
        .execute(conn)
        .await?;
        Ok(())
    }

    pub async fn items(&self, list_id: Uuid) -> sqlx::Result<Vec<ListItem>> {
        sqlx::query_as!(
            ListItem,
            r#"SELECT id, list_id, rank, "values" AS "values", created_by, created_at, updated_at
                 FROM list_items WHERE list_id = $1 AND deleted_at IS NULL
                ORDER BY rank, id"#,
            list_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn item_list(&self, item_id: Uuid) -> sqlx::Result<Option<Uuid>> {
        sqlx::query_scalar!(
            "SELECT i.list_id FROM list_items i
               JOIN lists l ON l.id = i.list_id
              WHERE i.id = $1 AND i.deleted_at IS NULL AND l.deleted_at IS NULL",
            item_id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn item_count_in(&self, conn: &mut PgConnection, list_id: Uuid) -> sqlx::Result<i64> {
        sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "count!" FROM list_items WHERE list_id = $1 AND deleted_at IS NULL"#,
            list_id
        )
        .fetch_one(conn)
        .await
    }

    pub async fn neighbours_in(
        &self,
        conn: &mut PgConnection,
        list_id: Uuid,
        after: Option<Uuid>,
        moving: Option<Uuid>,
    ) -> sqlx::Result<Option<(Option<String>, Option<String>)>> {
        let low = match after {
            Some(after) => {
                let rank = sqlx::query_scalar!(
                    "SELECT rank FROM list_items
                      WHERE id = $1 AND list_id = $2 AND deleted_at IS NULL",
                    after,
                    list_id
                )
                .fetch_optional(&mut *conn)
                .await?;
                match rank {
                    Some(rank) => Some(rank),
                    None => return Ok(None),
                }
            }
            None => None,
        };
        let high = sqlx::query_scalar!(
            r#"SELECT rank FROM list_items
                WHERE list_id = $1 AND deleted_at IS NULL
                  AND ($2::text IS NULL OR rank > $2)
                  AND ($3::uuid IS NULL OR id <> $3)
                  AND ($4::uuid IS NULL OR id <> $4)
                ORDER BY rank, id LIMIT 1"#,
            list_id,
            low,
            moving,
            after
        )
        .fetch_optional(&mut *conn)
        .await?;
        Ok(Some((low, high)))
    }

    pub async fn last_rank_in(
        &self,
        conn: &mut PgConnection,
        list_id: Uuid,
    ) -> sqlx::Result<Option<String>> {
        sqlx::query_scalar!(
            "SELECT rank FROM list_items WHERE list_id = $1 AND deleted_at IS NULL
              ORDER BY rank DESC, id DESC LIMIT 1",
            list_id
        )
        .fetch_optional(conn)
        .await
    }

    pub async fn insert_item_in(
        &self,
        conn: &mut PgConnection,
        list_id: Uuid,
        rank: &str,
        values: &serde_json::Value,
        created_by: Uuid,
    ) -> sqlx::Result<ListItem> {
        sqlx::query_as!(
            ListItem,
            r#"INSERT INTO list_items (list_id, rank, "values", created_by)
               VALUES ($1, $2, $3, $4)
               RETURNING id, list_id, rank, "values" AS "values", created_by, created_at, updated_at"#,
            list_id,
            rank,
            values,
            created_by
        )
        .fetch_one(conn)
        .await
    }

    pub async fn merge_item_in(
        &self,
        conn: &mut PgConnection,
        id: Uuid,
        set: &serde_json::Value,
        clear: &[String],
    ) -> sqlx::Result<Option<ListItem>> {
        sqlx::query_as!(
            ListItem,
            r#"UPDATE list_items SET "values" = ("values" || $2) - $3::text[], updated_at = NOW()
                WHERE id = $1 AND deleted_at IS NULL
               RETURNING id, list_id, rank, "values" AS "values", created_by, created_at, updated_at"#,
            id,
            set,
            clear
        )
        .fetch_optional(conn)
        .await
    }

    pub async fn move_item_in(
        &self,
        conn: &mut PgConnection,
        id: Uuid,
        rank: &str,
    ) -> sqlx::Result<ListItem> {
        sqlx::query_as!(
            ListItem,
            r#"UPDATE list_items SET rank = $2, updated_at = NOW() WHERE id = $1
               RETURNING id, list_id, rank, "values" AS "values", created_by, created_at, updated_at"#,
            id,
            rank
        )
        .fetch_one(conn)
        .await
    }

    pub async fn delete_item_in(&self, conn: &mut PgConnection, id: Uuid) -> sqlx::Result<bool> {
        let result = sqlx::query!(
            "UPDATE list_items SET deleted_at = NOW(), updated_at = NOW()
              WHERE id = $1 AND deleted_at IS NULL",
            id
        )
        .execute(conn)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn cells_in(
        &self,
        conn: &mut PgConnection,
        list_id: Uuid,
        column_id: Uuid,
    ) -> sqlx::Result<Vec<(Uuid, serde_json::Value)>> {
        let rows = sqlx::query!(
            r#"SELECT id, "values" -> $2::text AS "cell!" FROM list_items
                WHERE list_id = $1 AND deleted_at IS NULL AND "values" ? $2::text"#,
            list_id,
            column_id.to_string()
        )
        .fetch_all(conn)
        .await?;
        Ok(rows.into_iter().map(|row| (row.id, row.cell)).collect())
    }

    pub async fn rewrite_cells_in(
        &self,
        conn: &mut PgConnection,
        list_id: Uuid,
        column_id: Uuid,
        cells: &serde_json::Value,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            r#"UPDATE list_items i
                  SET "values" = CASE WHEN m.value = 'null'::jsonb THEN i."values" - $2::text
                                      ELSE jsonb_set(i."values", ARRAY[$2::text], m.value) END
                 FROM jsonb_each($3) AS m
                WHERE i.list_id = $1 AND i.id = m.key::uuid"#,
            list_id,
            column_id.to_string(),
            cells
        )
        .execute(conn)
        .await?;
        Ok(())
    }

    pub async fn exportable(
        &self,
        workspace_id: Uuid,
        include_private: bool,
    ) -> sqlx::Result<Vec<List>> {
        sqlx::query_as!(
            List,
            "SELECT l.id, l.workspace_id, l.channel_id, l.title, l.description, l.created_by,
                    l.created_at, l.updated_at
               FROM lists l
               LEFT JOIN channels c ON c.id = l.channel_id
              WHERE l.workspace_id = $1
                AND l.deleted_at IS NULL
                AND ($2::bool OR c.channel_type NOT IN ('dm', 'group_dm'))
              ORDER BY l.created_at",
            workspace_id,
            include_private
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn names(&self, user_ids: &[Uuid]) -> sqlx::Result<Vec<(Uuid, String)>> {
        let rows = sqlx::query!(
            r#"SELECT id, COALESCE(display_name, email) AS "name!" FROM users WHERE id = ANY($1)"#,
            user_ids
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|row| (row.id, row.name)).collect())
    }

    pub async fn channel_members_among(
        &self,
        channel_id: Uuid,
        user_ids: &[Uuid],
    ) -> sqlx::Result<Vec<Uuid>> {
        sqlx::query_scalar!(
            "SELECT user_id FROM channel_members WHERE channel_id = $1 AND user_id = ANY($2)",
            channel_id,
            user_ids
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn workspace_members_among(
        &self,
        workspace_id: Uuid,
        user_ids: &[Uuid],
    ) -> sqlx::Result<Vec<Uuid>> {
        sqlx::query_scalar!(
            "SELECT user_id FROM workspace_members WHERE workspace_id = $1 AND user_id = ANY($2)",
            workspace_id,
            user_ids
        )
        .fetch_all(&self.pool)
        .await
    }
}
