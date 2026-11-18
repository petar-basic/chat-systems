use sqlx::PgPool;
use uuid::Uuid;

use super::models::{InteractiveApp, StoredView, Trigger, UsedInteraction};

pub const MAX_STACK: i64 = 3;

pub struct Delivered {
    pub done: bool,
    pub response: Option<serde_json::Value>,
}

#[derive(Clone)]
pub struct InteractionRepo {
    pool: PgPool,
}

impl InteractionRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn app_of_message(&self, message_id: Uuid) -> sqlx::Result<Option<Uuid>> {
        sqlx::query_scalar!(
            r#"SELECT b.id
                 FROM messages m
                 JOIN bot_apps b
                   ON b.bot_user_id = m.user_id
                   OR b.id = (SELECT h.app_id FROM hooks h
                               WHERE h.id::text = m.metadata->'bot'->>'hook_id')
                WHERE m.id = $1 AND m.deleted_at IS NULL
                LIMIT 1"#,
            message_id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn interactive_apps(&self, workspace_id: Uuid) -> sqlx::Result<Vec<InteractiveApp>> {
        sqlx::query_as!(
            InteractiveApp,
            r#"SELECT b.id AS app_id, b.bot_user_id,
                      ARRAY(SELECT h.id FROM hooks h WHERE h.app_id = b.id ORDER BY h.id) AS "hook_ids!"
                 FROM bot_apps b
                WHERE b.workspace_id = $1 AND b.interactivity AND b.disabled_at IS NULL
                  AND (b.socket_mode OR b.request_url IS NOT NULL)
                ORDER BY b.id"#,
            workspace_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn display_name(&self, user_id: Uuid) -> sqlx::Result<Option<String>> {
        Ok(
            sqlx::query_scalar!("SELECT display_name FROM users WHERE id = $1", user_id)
                .fetch_optional(&self.pool)
                .await?
                .flatten(),
        )
    }

    pub async fn create_trigger(
        &self,
        app_id: Uuid,
        user_id: Uuid,
        stack_id: Option<Uuid>,
        invocation_id: Option<Uuid>,
        ttl_secs: f64,
    ) -> sqlx::Result<Uuid> {
        sqlx::query_scalar!(
            "INSERT INTO app_triggers (app_id, user_id, stack_id, invocation_id, expires_at)
             VALUES ($1, $2, $3, $4, NOW() + make_interval(secs => $5))
             RETURNING id",
            app_id,
            user_id,
            stack_id,
            invocation_id,
            ttl_secs
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn use_trigger(&self, id: Uuid, app_id: Uuid) -> sqlx::Result<Option<Trigger>> {
        sqlx::query_as!(
            Trigger,
            "UPDATE app_triggers SET used_at = NOW()
              WHERE id = $1 AND app_id = $2 AND used_at IS NULL AND expires_at > NOW()
             RETURNING user_id, stack_id, invocation_id",
            id,
            app_id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn create_interaction(
        &self,
        id: Uuid,
        app_id: Uuid,
        user_id: Uuid,
        message_id: Uuid,
        uses: i32,
        ttl_secs: f64,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            "INSERT INTO app_interactions (id, app_id, user_id, message_id, uses_left, expires_at)
             VALUES ($1, $2, $3, $4, $5, NOW() + make_interval(secs => $6))",
            id,
            app_id,
            user_id,
            message_id,
            uses,
            ttl_secs
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn use_interaction(&self, id: Uuid) -> sqlx::Result<Option<UsedInteraction>> {
        sqlx::query_as!(
            UsedInteraction,
            "UPDATE app_interactions SET uses_left = uses_left - 1
              WHERE id = $1 AND uses_left > 0 AND expires_at > NOW()
             RETURNING app_id, user_id, message_id",
            id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn interaction_exists(&self, id: Uuid) -> sqlx::Result<bool> {
        sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM app_interactions WHERE id = $1) AS "exists!""#,
            id
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn enqueue_for_socket(
        &self,
        app_id: Uuid,
        body: &serde_json::Value,
        ttl_secs: f64,
    ) -> sqlx::Result<Uuid> {
        sqlx::query_scalar!(
            "INSERT INTO app_deliveries (app_id, event_id, channel_id, kind, body, expires_at)
             VALUES ($1, gen_random_uuid(), gen_random_uuid(), 'interactive', $2,
                     NOW() + make_interval(secs => $3))
             RETURNING id",
            app_id,
            body,
            ttl_secs
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn delivered(&self, id: Uuid) -> sqlx::Result<Option<Delivered>> {
        sqlx::query_as!(
            Delivered,
            r#"SELECT delivered_at IS NOT NULL AS "done!", response
                 FROM app_deliveries WHERE id = $1"#,
            id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn forget_delivery(&self, id: Uuid) -> sqlx::Result<()> {
        sqlx::query!("DELETE FROM app_deliveries WHERE id = $1", id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn find_view(&self, id: Uuid) -> sqlx::Result<Option<StoredView>> {
        sqlx::query_as!(
            StoredView,
            "SELECT id, stack_id, position, app_id, workspace_id, user_id, view, hash
               FROM app_views WHERE id = $1",
            id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn find_external(
        &self,
        app_id: Uuid,
        external_id: &str,
    ) -> sqlx::Result<Option<StoredView>> {
        sqlx::query_as!(
            StoredView,
            "SELECT id, stack_id, position, app_id, workspace_id, user_id, view, hash
               FROM app_views WHERE app_id = $1 AND view->>'external_id' = $2
              ORDER BY updated_at DESC LIMIT 1",
            app_id,
            external_id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn stack(&self, stack_id: Uuid) -> sqlx::Result<Vec<StoredView>> {
        sqlx::query_as!(
            StoredView,
            "SELECT id, stack_id, position, app_id, workspace_id, user_id, view, hash
               FROM app_views WHERE stack_id = $1 ORDER BY position",
            stack_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn open(
        &self,
        app_id: Uuid,
        workspace_id: Uuid,
        user_id: Uuid,
        view: &serde_json::Value,
        hash: &str,
    ) -> sqlx::Result<StoredView> {
        let mut tx = self.pool.begin().await?;
        sqlx::query!(
            "DELETE FROM app_views WHERE user_id = $1 AND workspace_id = $2",
            user_id,
            workspace_id
        )
        .execute(&mut *tx)
        .await?;
        let opened = sqlx::query_as!(
            StoredView,
            "INSERT INTO app_views (stack_id, position, app_id, workspace_id, user_id, view, hash)
             VALUES (gen_random_uuid(), 0, $1, $2, $3, $4, $5)
             RETURNING id, stack_id, position, app_id, workspace_id, user_id, view, hash",
            app_id,
            workspace_id,
            user_id,
            view,
            hash
        )
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(opened)
    }

    pub async fn push(
        &self,
        stack_id: Uuid,
        view: &serde_json::Value,
        hash: &str,
    ) -> sqlx::Result<Option<StoredView>> {
        let pushed = sqlx::query_as!(
            StoredView,
            r#"INSERT INTO app_views (stack_id, position, app_id, workspace_id, user_id, view, hash)
               SELECT stack_id, MAX(position) + 1, app_id, workspace_id, user_id, $2, $3
                 FROM app_views WHERE stack_id = $1
                GROUP BY stack_id, app_id, workspace_id, user_id
               HAVING COUNT(*) < $4
               ON CONFLICT (stack_id, position) DO NOTHING
               RETURNING id, stack_id, position, app_id, workspace_id, user_id, view, hash"#,
            stack_id,
            view,
            hash,
            MAX_STACK
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(pushed)
    }

    pub async fn replace(
        &self,
        id: Uuid,
        view: &serde_json::Value,
        hash: &str,
        expected_hash: Option<&str>,
    ) -> sqlx::Result<Option<StoredView>> {
        sqlx::query_as!(
            StoredView,
            "UPDATE app_views SET view = $2, hash = $3, updated_at = NOW()
              WHERE id = $1 AND ($4::text IS NULL OR hash = $4)
             RETURNING id, stack_id, position, app_id, workspace_id, user_id, view, hash",
            id,
            view,
            hash,
            expected_hash
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn close_from(&self, stack_id: Uuid, position: i16) -> sqlx::Result<()> {
        sqlx::query!(
            "DELETE FROM app_views WHERE stack_id = $1 AND position >= $2",
            stack_id,
            position
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn purge(&self) -> sqlx::Result<()> {
        sqlx::query!("DELETE FROM app_triggers WHERE expires_at < NOW() - INTERVAL '1 hour'")
            .execute(&self.pool)
            .await?;
        sqlx::query!("DELETE FROM app_interactions WHERE expires_at < NOW()")
            .execute(&self.pool)
            .await?;
        sqlx::query!("DELETE FROM app_views WHERE updated_at < NOW() - INTERVAL '1 day'")
            .execute(&self.pool)
            .await?;
        sqlx::query!(
            "DELETE FROM app_deliveries
              WHERE kind = 'interactive' AND expires_at < NOW() - INTERVAL '1 minute'"
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
