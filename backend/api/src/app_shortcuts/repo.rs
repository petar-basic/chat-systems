use sqlx::PgPool;
use uuid::Uuid;

use super::models::{AppShortcutInput, AppShortcutRow};

#[derive(Clone)]
pub struct AppShortcutRepo {
    pool: PgPool,
}

impl AppShortcutRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list(&self, app_id: Uuid) -> sqlx::Result<Vec<AppShortcutRow>> {
        sqlx::query_as!(
            AppShortcutRow,
            "SELECT id, app_id, kind, name, description, callback_id
               FROM app_shortcuts
              WHERE app_id = $1
              ORDER BY kind, position",
            app_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn replace(
        &self,
        app_id: Uuid,
        shortcuts: &[AppShortcutInput],
    ) -> sqlx::Result<Vec<AppShortcutRow>> {
        let kinds: Vec<String> = shortcuts
            .iter()
            .map(|s| s.kind.as_str().to_string())
            .collect();
        let names: Vec<String> = shortcuts
            .iter()
            .map(|s| s.name.trim().to_string())
            .collect();
        let descriptions: Vec<String> = shortcuts
            .iter()
            .map(|s| s.description.trim().to_string())
            .collect();
        let callback_ids: Vec<String> = shortcuts.iter().map(|s| s.callback_id.clone()).collect();
        let positions: Vec<i16> = (0..shortcuts.len())
            .map(|i| i16::try_from(i).unwrap_or(i16::MAX))
            .collect();

        let mut tx = self.pool.begin().await?;
        sqlx::query!(
            "DELETE FROM app_shortcuts WHERE app_id = $1 AND callback_id <> ALL($2::text[])",
            app_id,
            &callback_ids
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "INSERT INTO app_shortcuts (app_id, kind, name, description, callback_id, position)
             SELECT $1, kind, name, description, callback_id, position
               FROM UNNEST($2::text[], $3::text[], $4::text[], $5::text[], $6::smallint[])
                 AS t(kind, name, description, callback_id, position)
             ON CONFLICT (app_id, callback_id) DO UPDATE
                SET kind = EXCLUDED.kind, name = EXCLUDED.name,
                    description = EXCLUDED.description, position = EXCLUDED.position",
            app_id,
            &kinds,
            &names,
            &descriptions,
            &callback_ids,
            &positions
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        self.list(app_id).await
    }

    pub async fn find(&self, id: Uuid) -> sqlx::Result<Option<AppShortcutRow>> {
        sqlx::query_as!(
            AppShortcutRow,
            "SELECT id, app_id, kind, name, description, callback_id
               FROM app_shortcuts WHERE id = $1",
            id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn global_in_workspace(
        &self,
        workspace_id: Uuid,
    ) -> sqlx::Result<Vec<AppShortcutRow>> {
        sqlx::query_as!(
            AppShortcutRow,
            "SELECT s.id, s.app_id, s.kind, s.name, s.description, s.callback_id
               FROM app_shortcuts s
               JOIN bot_apps b ON b.id = s.app_id
              WHERE b.workspace_id = $1 AND s.kind = 'global'
                AND b.interactivity AND b.disabled_at IS NULL
                AND (b.socket_mode OR b.request_url IS NOT NULL)
              ORDER BY b.name, s.position",
            workspace_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn message_in_channel(&self, channel_id: Uuid) -> sqlx::Result<Vec<AppShortcutRow>> {
        sqlx::query_as!(
            AppShortcutRow,
            "SELECT s.id, s.app_id, s.kind, s.name, s.description, s.callback_id
               FROM app_shortcuts s
               JOIN bot_apps b ON b.id = s.app_id
               JOIN channel_members cm ON cm.user_id = b.bot_user_id AND cm.channel_id = $1
              WHERE s.kind = 'message'
                AND b.interactivity AND b.disabled_at IS NULL
                AND (b.socket_mode OR b.request_url IS NOT NULL)
              ORDER BY b.name, s.position",
            channel_id
        )
        .fetch_all(&self.pool)
        .await
    }
}
