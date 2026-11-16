use sqlx::types::Json;
use sqlx::PgPool;
use uuid::Uuid;

use super::models::{RecentSearch, SearchFilters, SearchTab, KEPT};

#[derive(Clone)]
pub struct SearchRepo {
    pool: PgPool,
}

impl SearchRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn recent(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
    ) -> sqlx::Result<Vec<RecentSearch>> {
        sqlx::query_as!(
            RecentSearch,
            r#"
            SELECT id AS "id!", query AS "query!", tab AS "tab!: SearchTab",
                   filters AS "filters!: Json<SearchFilters>", searched_at AS "searched_at!"
              FROM (SELECT DISTINCT ON (lower(query), tab, filters)
                           id, query, tab, filters, searched_at
                      FROM recent_searches_seen
                     WHERE user_id = $1 AND workspace_id = $2
                     ORDER BY lower(query), tab, filters, searched_at DESC, id DESC) once
             ORDER BY searched_at DESC, id DESC
             LIMIT $3
            "#,
            user_id,
            workspace_id,
            KEPT
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn record(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        query: &str,
        tab: SearchTab,
        filters: &SearchFilters,
    ) -> sqlx::Result<RecentSearch> {
        let mut tx = self.pool.begin().await?;
        let search = sqlx::query_as!(
            RecentSearch,
            r#"
            INSERT INTO recent_searches (workspace_id, user_id, query, tab, filters)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (user_id, workspace_id, lower(query), tab, filters)
            DO UPDATE SET query = EXCLUDED.query, searched_at = NOW()
            RETURNING id, query, tab AS "tab: SearchTab",
                      filters AS "filters!: Json<SearchFilters>", searched_at
            "#,
            workspace_id,
            user_id,
            query,
            tab as SearchTab,
            Json(filters) as _
        )
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query!(
            r#"
            DELETE FROM recent_searches
             WHERE id IN (SELECT id FROM recent_searches
                           WHERE user_id = $1 AND workspace_id = $2
                           ORDER BY searched_at DESC, id DESC
                          OFFSET $3)
            "#,
            user_id,
            workspace_id,
            KEPT
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(search)
    }

    pub async fn forget(&self, id: Uuid, user_id: Uuid, workspace_id: Uuid) -> sqlx::Result<bool> {
        let done = sqlx::query!(
            r#"
            DELETE FROM recent_searches
             WHERE id IN (SELECT twin.id
                            FROM recent_searches_seen picked
                            JOIN recent_searches_seen twin
                              ON twin.user_id = picked.user_id
                             AND twin.workspace_id = picked.workspace_id
                             AND lower(twin.query) = lower(picked.query)
                             AND twin.tab = picked.tab
                             AND twin.filters = picked.filters
                           WHERE picked.id = $1 AND picked.user_id = $2 AND picked.workspace_id = $3)
            "#,
            id,
            user_id,
            workspace_id
        )
        .execute(&self.pool)
        .await?;
        Ok(done.rows_affected() > 0)
    }

    pub async fn clear(&self, user_id: Uuid, workspace_id: Uuid) -> sqlx::Result<u64> {
        let done = sqlx::query!(
            "DELETE FROM recent_searches WHERE user_id = $1 AND workspace_id = $2",
            user_id,
            workspace_id
        )
        .execute(&self.pool)
        .await?;
        Ok(done.rows_affected())
    }

    pub async fn owned_by(&self, user_id: Uuid) -> sqlx::Result<Vec<serde_json::Value>> {
        sqlx::query_scalar!(
            r#"
            SELECT jsonb_build_object(
                       'workspace_id', workspace_id,
                       'query', query,
                       'tab', tab,
                       'filters', filters,
                       'searched_at', searched_at
                   ) AS "row!"
              FROM recent_searches
             WHERE user_id = $1
             ORDER BY searched_at
            "#,
            user_id
        )
        .fetch_all(&self.pool)
        .await
    }
}
