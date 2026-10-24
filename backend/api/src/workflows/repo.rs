use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::models::*;

#[derive(Clone)]
pub struct WorkflowRepo {
    pool: PgPool,
}

pub struct DueSchedule {
    pub id: Uuid,
    pub version: i32,
    pub definition: serde_json::Value,
    pub next_run_at: DateTime<Utc>,
}

pub struct Listener {
    pub id: Uuid,
    pub version: i32,
}

pub struct OpenForm {
    pub run_id: Uuid,
    pub position: i32,
    pub recipient_id: Uuid,
    pub message_id: Uuid,
    pub fields: serde_json::Value,
    pub expires_at: DateTime<Utc>,
    pub answered_at: Option<DateTime<Utc>>,
    pub run_status: RunStatus,
    pub workspace_id: Uuid,
}

pub struct ExpiredForm {
    pub message_id: Uuid,
    pub workspace_id: Uuid,
}

pub struct ShortcutRow {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
}

pub struct StoppedWorkflow {
    pub id: Uuid,
    pub name: String,
}

impl WorkflowRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn insert(
        &self,
        workspace_id: Uuid,
        created_by: Uuid,
        draft: &WorkflowDraft,
        webhook_token: Option<&str>,
    ) -> sqlx::Result<Uuid> {
        sqlx::query_scalar!(
            r"
            INSERT INTO workflows (workspace_id, created_by, name, description, definition, webhook_token)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id
            ",
            workspace_id,
            created_by,
            draft.name,
            draft.description,
            draft.definition,
            webhook_token
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn update(
        &self,
        id: Uuid,
        draft: &WorkflowDraft,
        webhook_token: Option<&str>,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            r"
            UPDATE workflows
               SET name = $2, description = $3, definition = $4, webhook_token = $5,
                   version = version + 1, updated_at = NOW()
             WHERE id = $1
            ",
            id,
            draft.name,
            draft.description,
            draft.definition,
            webhook_token
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn set_enabled(
        &self,
        id: Uuid,
        enabled: bool,
        schedule: Option<&Schedule<'_>>,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            r"
            UPDATE workflows
               SET enabled = $2,
                   next_run_at = CASE WHEN $2 AND $3::smallint[] IS NOT NULL
                                      THEN workflow_next_run($3, $4, $5, NOW()) END,
                   updated_at = NOW()
             WHERE id = $1
            ",
            id,
            enabled,
            schedule.map(|s| s.days.clone()) as Option<Vec<i16>>,
            schedule.map(|s| s.time),
            schedule.map(|s| s.timezone)
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete(&self, id: Uuid) -> sqlx::Result<()> {
        sqlx::query!("DELETE FROM workflows WHERE id = $1", id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn find(&self, id: Uuid) -> sqlx::Result<Option<WorkflowRow>> {
        sqlx::query_as!(
            WorkflowRow,
            r#"
            SELECT w.id, w.workspace_id, w.name, w.description, w.created_by, w.definition,
                   w.version, w.enabled, w.webhook_token, w.next_run_at, w.created_at, w.updated_at,
                   r.status AS "last_status?: RunStatus", r.started_at AS "last_started_at?"
              FROM workflows w
              LEFT JOIN LATERAL (
                  SELECT status, started_at FROM workflow_runs
                   WHERE workflow_id = w.id ORDER BY started_at DESC LIMIT 1
              ) r ON true
             WHERE w.id = $1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn find_by_token(&self, token: &str) -> sqlx::Result<Option<WorkflowRow>> {
        sqlx::query_as!(
            WorkflowRow,
            r#"
            SELECT id, workspace_id, name, description, created_by, definition,
                   version, enabled, webhook_token, next_run_at, created_at, updated_at,
                   NULL::workflow_run_status AS "last_status?: RunStatus",
                   NULL::timestamptz AS "last_started_at?"
              FROM workflows
             WHERE webhook_token = $1
            "#,
            token
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn list(
        &self,
        workspace_id: Uuid,
        only_by: Option<Uuid>,
    ) -> sqlx::Result<Vec<WorkflowRow>> {
        sqlx::query_as!(
            WorkflowRow,
            r#"
            SELECT w.id, w.workspace_id, w.name, w.description, w.created_by, w.definition,
                   w.version, w.enabled, w.webhook_token, w.next_run_at, w.created_at, w.updated_at,
                   r.status AS "last_status?: RunStatus", r.started_at AS "last_started_at?"
              FROM workflows w
              LEFT JOIN LATERAL (
                  SELECT status, started_at FROM workflow_runs
                   WHERE workflow_id = w.id ORDER BY started_at DESC LIMIT 1
              ) r ON true
             WHERE w.workspace_id = $1 AND ($2::uuid IS NULL OR w.created_by = $2)
             ORDER BY lower(w.name), w.created_at
            "#,
            workspace_id,
            only_by
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn timezone_exists(&self, timezone: &str) -> sqlx::Result<bool> {
        sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM pg_timezone_names WHERE name = $1) AS "exists!""#,
            timezone
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn queue_run(
        &self,
        workflow_id: Uuid,
        version: i32,
        trigger_key: &str,
        context: &serde_json::Value,
    ) -> sqlx::Result<Uuid> {
        let mut conn = self.pool.acquire().await?;
        self.queue_run_in(&mut conn, workflow_id, version, trigger_key, context)
            .await
    }

    pub async fn queue_run_in(
        &self,
        conn: &mut PgConnection,
        workflow_id: Uuid,
        version: i32,
        trigger_key: &str,
        context: &serde_json::Value,
    ) -> sqlx::Result<Uuid> {
        sqlx::query_scalar!(
            r"
            INSERT INTO workflow_runs (workflow_id, workflow_version, trigger_key, context)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (workflow_id, trigger_key) DO UPDATE SET trigger_key = EXCLUDED.trigger_key
            RETURNING id
            ",
            workflow_id,
            version,
            trigger_key,
            context
        )
        .fetch_one(&mut *conn)
        .await
    }

    pub async fn skip_run(
        &self,
        workflow_id: Uuid,
        version: i32,
        trigger_key: &str,
        context: &serde_json::Value,
        reason: &str,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            r"
            INSERT INTO workflow_runs
                   (workflow_id, workflow_version, trigger_key, context, status, error, finished_at)
            VALUES ($1, $2, $3, $4, 'failed', $5, NOW())
            ON CONFLICT (workflow_id, trigger_key) DO NOTHING
            ",
            workflow_id,
            version,
            trigger_key,
            context,
            reason
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn listening(
        &self,
        kind: &str,
        channel_id: Uuid,
        emoji: Option<&str>,
    ) -> sqlx::Result<Vec<Listener>> {
        sqlx::query_as!(
            Listener,
            r"
            SELECT id, version
              FROM workflows
             WHERE enabled
               AND definition #>> '{trigger,channel_id}' = $2
               AND definition #>> '{trigger,type}' = $1
               AND ($3::text IS NULL OR definition #>> '{trigger,emoji}' = $3)
            ",
            kind,
            channel_id.to_string(),
            emoji
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn due_schedules_in(
        &self,
        conn: &mut PgConnection,
        limit: i64,
    ) -> sqlx::Result<Vec<DueSchedule>> {
        sqlx::query_as!(
            DueSchedule,
            r#"
            SELECT id, version, definition, next_run_at AS "next_run_at!"
              FROM workflows
             WHERE enabled AND next_run_at <= NOW()
             ORDER BY next_run_at
             LIMIT $1
             FOR UPDATE SKIP LOCKED
            "#,
            limit
        )
        .fetch_all(&mut *conn)
        .await
    }

    pub async fn advance_in(
        &self,
        conn: &mut PgConnection,
        id: Uuid,
        schedule: Option<&Schedule<'_>>,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            r"
            UPDATE workflows
               SET next_run_at = CASE WHEN $2::smallint[] IS NOT NULL
                                      THEN workflow_next_run($2, $3, $4, GREATEST(next_run_at, NOW())) END
             WHERE id = $1
            ",
            id,
            schedule.map(|s| s.days.clone()) as Option<Vec<i16>>,
            schedule.map(|s| s.time),
            schedule.map(|s| s.timezone)
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    pub async fn claim_runs(&self, limit: i64) -> sqlx::Result<Vec<RunRow>> {
        sqlx::query_as!(
            RunRow,
            r#"
            UPDATE workflow_runs r
               SET status = 'running', claimed_at = NOW()
              FROM (
                  SELECT id FROM workflow_runs
                   WHERE status = 'pending'
                      OR (status = 'running' AND claimed_at < NOW() - INTERVAL '5 minutes')
                   ORDER BY started_at
                   LIMIT $1
                   FOR UPDATE SKIP LOCKED
              ) due
             WHERE r.id = due.id
            RETURNING r.id, r.workflow_id, r.workflow_version, r.status AS "status: RunStatus",
                      r.context, r.error, r.started_at, r.finished_at
            "#,
            limit
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn finished_positions(&self, run_id: Uuid) -> sqlx::Result<Vec<i32>> {
        sqlx::query_scalar!(
            "SELECT position FROM workflow_run_steps WHERE run_id = $1 AND succeeded",
            run_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn record_item_step_in(
        &self,
        conn: &mut PgConnection,
        run_id: Uuid,
        position: i32,
        item_id: Uuid,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            r"
            INSERT INTO workflow_run_steps (run_id, position, succeeded, list_item_id)
            VALUES ($1, $2, TRUE, $3)
            ON CONFLICT (run_id, position) DO UPDATE
               SET succeeded = TRUE, list_item_id = EXCLUDED.list_item_id, error = NULL,
                   finished_at = NOW()
            ",
            run_id,
            position,
            item_id
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    pub async fn item_of(&self, run_id: Uuid, position: i32) -> sqlx::Result<Option<Uuid>> {
        Ok(sqlx::query_scalar!(
            "SELECT list_item_id FROM workflow_run_steps WHERE run_id = $1 AND position = $2",
            run_id,
            position
        )
        .fetch_optional(&self.pool)
        .await?
        .flatten())
    }

    pub async fn record_step_in(
        &self,
        conn: &mut PgConnection,
        run_id: Uuid,
        position: i32,
        outcome: Result<Option<Uuid>, &str>,
    ) -> sqlx::Result<()> {
        let (succeeded, message_id, error) = match outcome {
            Ok(message_id) => (true, message_id, None),
            Err(error) => (false, None, Some(error)),
        };
        sqlx::query!(
            r"
            INSERT INTO workflow_run_steps (run_id, position, succeeded, message_id, error)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (run_id, position) DO UPDATE
               SET succeeded = EXCLUDED.succeeded, message_id = EXCLUDED.message_id,
                   error = EXCLUDED.error, finished_at = NOW()
            ",
            run_id,
            position,
            succeeded,
            message_id,
            error
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    pub async fn record_step(
        &self,
        run_id: Uuid,
        position: i32,
        outcome: Result<Option<Uuid>, &str>,
    ) -> sqlx::Result<()> {
        let mut conn = self.pool.acquire().await?;
        self.record_step_in(&mut conn, run_id, position, outcome)
            .await
    }

    pub async fn finish(
        &self,
        run_id: Uuid,
        version: i32,
        status: RunStatus,
        error: Option<&str>,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            r"
            UPDATE workflow_runs
               SET status = $3, error = $4, workflow_version = $2, finished_at = NOW()
             WHERE id = $1
            ",
            run_id,
            version,
            status as RunStatus,
            error
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn runs(&self, workflow_id: Uuid, limit: i64) -> sqlx::Result<Vec<RunRow>> {
        sqlx::query_as!(
            RunRow,
            r#"
            SELECT id, workflow_id, workflow_version, status AS "status: RunStatus",
                   context, error, started_at, finished_at
              FROM workflow_runs
             WHERE workflow_id = $1
             ORDER BY started_at DESC, id
             LIMIT $2
            "#,
            workflow_id,
            limit
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn steps_of(&self, run_ids: &[Uuid]) -> sqlx::Result<Vec<(Uuid, RunStep)>> {
        let rows = sqlx::query!(
            r"
            SELECT run_id, position, succeeded, message_id, list_item_id, error, finished_at
              FROM workflow_run_steps
             WHERE run_id = ANY($1)
             ORDER BY run_id, position
            ",
            run_ids
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                (
                    row.run_id,
                    RunStep {
                        position: row.position,
                        succeeded: row.succeeded,
                        message_id: row.message_id,
                        list_item_id: row.list_item_id,
                        error: row.error,
                        finished_at: row.finished_at,
                    },
                )
            })
            .collect())
    }

    pub async fn shortcuts(&self, channel_id: Uuid) -> sqlx::Result<Vec<ShortcutRow>> {
        sqlx::query_as!(
            ShortcutRow,
            r"
            SELECT id, name, description
              FROM workflows
             WHERE enabled
               AND definition #>> '{trigger,channel_id}' = $1
               AND definition #>> '{trigger,type}' = 'shortcut'
             ORDER BY lower(name)
            ",
            channel_id.to_string()
        )
        .fetch_all(&self.pool)
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn wait_in(
        &self,
        conn: &mut PgConnection,
        run_id: Uuid,
        position: i32,
        recipient_id: Uuid,
        message_id: Uuid,
        fields: &serde_json::Value,
        expires_at: DateTime<Utc>,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            r"
            INSERT INTO workflow_forms (run_id, position, recipient_id, message_id, fields, expires_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            ",
            run_id,
            position,
            recipient_id,
            message_id,
            fields,
            expires_at
        )
        .execute(&mut *conn)
        .await?;
        sqlx::query!(
            "UPDATE workflow_runs SET status = 'waiting' WHERE id = $1",
            run_id
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    pub async fn form_for_update_in(
        &self,
        conn: &mut PgConnection,
        run_id: Uuid,
        position: i32,
    ) -> sqlx::Result<Option<OpenForm>> {
        sqlx::query_as!(
            OpenForm,
            r#"
            SELECT f.run_id, f.position, f.recipient_id, f.message_id, f.fields, f.expires_at,
                   f.answered_at, r.status AS "run_status: RunStatus", w.workspace_id
              FROM workflow_forms f
              JOIN workflow_runs r ON r.id = f.run_id
              JOIN workflows w ON w.id = r.workflow_id
             WHERE f.run_id = $1 AND f.position = $2
             FOR UPDATE OF f, r
            "#,
            run_id,
            position
        )
        .fetch_optional(&mut *conn)
        .await
    }

    pub async fn answer_in(
        &self,
        conn: &mut PgConnection,
        run_id: Uuid,
        position: i32,
        answers: &serde_json::Value,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            r"
            UPDATE workflow_forms SET answers = $3, answered_at = NOW()
             WHERE run_id = $1 AND position = $2
            ",
            run_id,
            position,
            answers
        )
        .execute(&mut *conn)
        .await?;
        sqlx::query!(
            r"
            UPDATE workflow_runs
               SET context = jsonb_set(context, '{form}', COALESCE(context->'form', '{}'::jsonb) || $2::jsonb),
                   status = 'pending', claimed_at = NULL
             WHERE id = $1
            ",
            run_id,
            answers
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    pub async fn expire_forms_in(
        &self,
        conn: &mut PgConnection,
        reason: &str,
    ) -> sqlx::Result<Vec<ExpiredForm>> {
        sqlx::query_as!(
            ExpiredForm,
            r"
            UPDATE workflow_runs r
               SET status = 'expired', error = $1, finished_at = NOW()
              FROM workflow_forms f, workflows w
             WHERE f.run_id = r.id AND w.id = r.workflow_id
               AND r.status = 'waiting' AND f.answered_at IS NULL AND f.expires_at <= NOW()
            RETURNING f.message_id, w.workspace_id
            ",
            reason
        )
        .fetch_all(&mut *conn)
        .await
    }

    pub async fn disable_by_creator(
        &self,
        workspace_id: Uuid,
        created_by: Uuid,
    ) -> sqlx::Result<Vec<StoppedWorkflow>> {
        sqlx::query_as!(
            StoppedWorkflow,
            r"
            UPDATE workflows
               SET enabled = false, next_run_at = NULL, updated_at = NOW()
             WHERE workspace_id = $1 AND created_by = $2 AND enabled
            RETURNING id, name
            ",
            workspace_id,
            created_by
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn admin_ids(&self, workspace_id: Uuid) -> sqlx::Result<Vec<Uuid>> {
        sqlx::query_scalar!(
            r"
            SELECT user_id FROM workspace_members
             WHERE workspace_id = $1 AND role IN ('admin', 'owner')
            ",
            workspace_id
        )
        .fetch_all(&self.pool)
        .await
    }
}
