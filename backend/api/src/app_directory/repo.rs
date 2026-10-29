use sqlx::PgPool;
use uuid::Uuid;

use super::models::{BotAppRow, ChannelRow, ConnectedAppRow, HookRow, WorkflowRow};
use crate::hooks::models::HookType;
use crate::workflows::models::RunStatus;

#[derive(Clone)]
pub struct AppDirectoryRepo {
    pool: PgPool,
}

impl AppDirectoryRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn connected_apps(&self, workspace_id: Uuid) -> sqlx::Result<Vec<ConnectedAppRow>> {
        sqlx::query_as!(
            ConnectedAppRow,
            r#"SELECT a.provider, a.enabled, a.configured_by,
                      COALESCE(wm.display_name, u.display_name) AS "configured_by_name?",
                      a.configured_at
                 FROM workspace_apps a
                 LEFT JOIN users u ON u.id = a.configured_by
                 LEFT JOIN workspace_members wm
                        ON wm.workspace_id = a.workspace_id AND wm.user_id = a.configured_by
                WHERE a.workspace_id = $1
                ORDER BY a.configured_at"#,
            workspace_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn bot_apps(&self, workspace_id: Uuid) -> sqlx::Result<Vec<BotAppRow>> {
        sqlx::query_as!(
            BotAppRow,
            r#"SELECT b.id, b.name, b.description, b.scopes, b.created_by,
                      COALESCE(wm.display_name, u.display_name) AS "created_by_name?",
                      b.created_at, b.failing_since, b.last_failure,
                      ARRAY(SELECT cm.channel_id
                              FROM channel_members cm
                              JOIN channels c ON c.id = cm.channel_id
                             WHERE cm.user_id = b.bot_user_id
                               AND c.channel_type IN ('public', 'private')
                               AND c.is_archived IS NOT TRUE) AS "channel_ids!"
                 FROM bot_apps b
                 LEFT JOIN users u ON u.id = b.created_by
                 LEFT JOIN workspace_members wm
                        ON wm.workspace_id = b.workspace_id AND wm.user_id = b.created_by
                WHERE b.workspace_id = $1
                ORDER BY lower(b.name)"#,
            workspace_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn hooks(&self, workspace_id: Uuid) -> sqlx::Result<Vec<HookRow>> {
        sqlx::query_as!(
            HookRow,
            r#"SELECT h.id, h.hook_type AS "hook_type: HookType", h.name, h.description, h.config,
                      COALESCE(h.is_active, true) AS "is_active!", h.disabled_reason, h.app_id,
                      h.created_by, COALESCE(wm.display_name, u.display_name) AS "created_by_name?",
                      h.created_at, h.updated_at
                 FROM hooks h
                 JOIN users u ON u.id = h.created_by
                 LEFT JOIN workspace_members wm
                        ON wm.workspace_id = h.workspace_id AND wm.user_id = h.created_by
                WHERE h.workspace_id = $1
                ORDER BY h.hook_type, lower(h.name)"#,
            workspace_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn workflows(&self, workspace_id: Uuid) -> sqlx::Result<Vec<WorkflowRow>> {
        sqlx::query_as!(
            WorkflowRow,
            r#"SELECT w.id, w.name, w.description, w.definition, w.enabled, w.created_by,
                      COALESCE(wm.display_name, u.display_name) AS "created_by_name?",
                      w.created_at,
                      r.status AS "last_status?: RunStatus",
                      r.error AS "last_error?",
                      r.started_at AS "last_started_at?"
                 FROM workflows w
                 JOIN users u ON u.id = w.created_by
                 LEFT JOIN workspace_members wm
                        ON wm.workspace_id = w.workspace_id AND wm.user_id = w.created_by
                 LEFT JOIN LATERAL (
                     SELECT status, error, started_at FROM workflow_runs
                      WHERE workflow_id = w.id ORDER BY started_at DESC LIMIT 1
                 ) r ON true
                WHERE w.workspace_id = $1
                ORDER BY lower(w.name)"#,
            workspace_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn channels(
        &self,
        workspace_id: Uuid,
        ids: &[Uuid],
        viewer: Uuid,
    ) -> sqlx::Result<Vec<ChannelRow>> {
        sqlx::query_as!(
            ChannelRow,
            r#"SELECT c.id, COALESCE(c.name, '') AS "name!",
                      (c.channel_type = 'public'
                       OR EXISTS (SELECT 1 FROM channel_members m
                                   WHERE m.channel_id = c.id AND m.user_id = $3)) AS "visible!"
                 FROM channels c
                WHERE c.workspace_id = $1
                  AND c.id = ANY($2)
                  AND c.channel_type IN ('public', 'private')
                  AND c.is_archived IS NOT TRUE
                ORDER BY c.name"#,
            workspace_id,
            ids,
            viewer
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn admins(&self, workspace_id: Uuid) -> sqlx::Result<Vec<Uuid>> {
        sqlx::query_scalar!(
            r#"SELECT wm.user_id
                 FROM workspace_members wm
                 JOIN users u ON u.id = wm.user_id
                WHERE wm.workspace_id = $1
                  AND wm.role IN ('admin', 'owner')
                  AND u.status = 'active'
                  AND NOT u.is_bot"#,
            workspace_id
        )
        .fetch_all(&self.pool)
        .await
    }
}
