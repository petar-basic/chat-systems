use sqlx::PgPool;
use uuid::Uuid;

use super::models::StoredHome;

#[derive(Clone)]
pub struct AppHomeRepo {
    pool: PgPool,
}

pub struct NewHome<'a> {
    pub app_id: Uuid,
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub view: &'a serde_json::Value,
    pub hash: &'a str,
    pub expected_hash: Option<&'a str>,
}

impl AppHomeRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn find(&self, app_id: Uuid, user_id: Uuid) -> sqlx::Result<Option<StoredHome>> {
        sqlx::query_as!(
            StoredHome,
            "SELECT id, app_id, workspace_id, user_id, view, hash
               FROM app_homes WHERE app_id = $1 AND user_id = $2",
            app_id,
            user_id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn publish(&self, home: NewHome<'_>) -> sqlx::Result<Option<StoredHome>> {
        sqlx::query_as!(
            StoredHome,
            "INSERT INTO app_homes (app_id, workspace_id, user_id, view, hash)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (app_id, user_id) DO UPDATE
                SET view = EXCLUDED.view, hash = EXCLUDED.hash, updated_at = NOW()
              WHERE $6::text IS NULL OR app_homes.hash = $6
             RETURNING id, app_id, workspace_id, user_id, view, hash",
            home.app_id,
            home.workspace_id,
            home.user_id,
            home.view,
            home.hash,
            home.expected_hash
        )
        .fetch_optional(&self.pool)
        .await
    }
}
