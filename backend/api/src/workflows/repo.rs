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
    pub workspace_id: Uuid,
    pub name: String,
}

pub struct StepRow {
    pub position: i32,
    pub succeeded: bool,
    pub matched: Option<bool>,
    pub message_id: Option<Uuid>,
    pub list_item_id: Option<Uuid>,
    pub channel_id: Option<Uuid>,
    pub error: Option<String>,
    pub finished_at: DateTime<Utc>,
    pub inputs: Option<serde_json::Value>,
    pub outputs: Option<serde_json::Value>,
}

#[derive(Default)]
pub struct Output {
    pub message_id: Option<Uuid>,
    pub channel_id: Option<Uuid>,
    pub matched: Option<bool>,
}

pub struct NewCall<'a> {
    pub run_id: Uuid,
    pub position: i32,
    pub app_id: Uuid,
    pub title: &'a str,
    pub inputs: &'a serde_json::Value,
    pub outputs: &'a serde_json::Value,
    pub timeout_secs: f64,
}

pub struct OpenCall {
    pub id: Uuid,
    pub run_id: Uuid,
    pub position: i32,
    pub app_id: Uuid,
    pub title: String,
    pub inputs: serde_json::Value,
    pub outputs: serde_json::Value,
    pub expires_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub run_status: RunStatus,
    pub waiting_at: Option<i32>,
    pub workspace_id: Uuid,
}

pub struct AppCall {
    pub id: Uuid,
    pub expires_at: DateTime<Utc>,
}

pub struct AppUser {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
    pub created_by: Uuid,
}

impl Output {
    pub fn message(message_id: Uuid) -> Self {
        Self {
            message_id: Some(message_id),
            ..Self::default()
        }
    }

    pub fn channel(channel_id: Uuid) -> Self {
        Self {
            channel_id: Some(channel_id),
            ..Self::default()
        }
    }
}

pub struct WakeTime {
    pub at: DateTime<Utc>,
    pub due: bool,
    pub too_far: bool,
}

pub struct Version {
    pub version: i32,
    pub definition: serde_json::Value,
}

impl WorkflowRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn insert(
        &self,
        workspace_id: Uuid,
        created_by: Uuid,
        workflow: &CheckedWorkflow,
        webhook_token: Option<&str>,
        schedule: Option<&Schedule<'_>>,
    ) -> sqlx::Result<Uuid> {
        sqlx::query_scalar!(
            r#"
            WITH w AS (
                INSERT INTO workflows (workspace_id, created_by, name, description, definition,
                                       webhook_token, enabled, next_run_at)
                VALUES ($1, $2, $3, $4, $5, $6, TRUE,
                        CASE WHEN $7::smallint[] IS NOT NULL
                             THEN workflow_next_run($7, $8, $9, NOW()) END)
                RETURNING id, version, definition
            )
            INSERT INTO workflow_versions (workflow_id, version, definition)
            SELECT id, version, definition FROM w
            RETURNING workflow_id AS "id!"
            "#,
            workspace_id,
            created_by,
            workflow.name,
            workflow.description,
            workflow.definition,
            webhook_token,
            schedule.map(|s| s.days.clone()) as Option<Vec<i16>>,
            schedule.map(|s| s.time),
            schedule.map(|s| s.timezone)
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn update(
        &self,
        id: Uuid,
        workflow: &CheckedWorkflow,
        webhook_token: Option<&str>,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            r"
            WITH w AS (
                UPDATE workflows
                   SET name = $2, description = $3, definition = $4, webhook_token = $5,
                       version = version + 1, updated_at = NOW()
                 WHERE id = $1
                RETURNING id, version, definition
            )
            INSERT INTO workflow_versions (workflow_id, version, definition)
            SELECT id, version, definition FROM w
            ",
            id,
            workflow.name,
            workflow.description,
            workflow.definition,
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
                   NOT EXISTS (SELECT 1 FROM workspace_members wm JOIN users u ON u.id = wm.user_id
                                WHERE wm.workspace_id = w.workspace_id AND wm.user_id = w.created_by
                                  AND u.status = 'active') AS "creator_gone!",
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

    pub async fn definition_at(
        &self,
        workflow_id: Uuid,
        version: i32,
    ) -> sqlx::Result<Option<serde_json::Value>> {
        sqlx::query_scalar!(
            "SELECT definition FROM workflow_versions WHERE workflow_id = $1 AND version = $2",
            workflow_id,
            version
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn versions(
        &self,
        workflow_id: Uuid,
        versions: &[i32],
    ) -> sqlx::Result<Vec<Version>> {
        sqlx::query_as!(
            Version,
            "SELECT version, definition FROM workflow_versions
              WHERE workflow_id = $1 AND version = ANY($2)",
            workflow_id,
            versions
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn find_by_token(&self, token: &str) -> sqlx::Result<Option<WorkflowRow>> {
        sqlx::query_as!(
            WorkflowRow,
            r#"
            SELECT w.id, w.workspace_id, w.name, w.description, w.created_by, w.definition,
                   w.version, w.enabled, w.webhook_token, w.next_run_at, w.created_at, w.updated_at,
                   NOT EXISTS (SELECT 1 FROM workspace_members wm JOIN users u ON u.id = wm.user_id
                                WHERE wm.workspace_id = w.workspace_id AND wm.user_id = w.created_by
                                  AND u.status = 'active') AS "creator_gone!",
                   NULL::workflow_run_status AS "last_status?: RunStatus",
                   NULL::timestamptz AS "last_started_at?"
              FROM workflows w
             WHERE w.webhook_token = $1
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
                   NOT EXISTS (SELECT 1 FROM workspace_members wm JOIN users u ON u.id = wm.user_id
                                WHERE wm.workspace_id = w.workspace_id AND wm.user_id = w.created_by
                                  AND u.status = 'active') AS "creator_gone!",
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
                      r.context, r.error, r.started_at, r.finished_at,
                      r.waiting_at, r.resume_at AS waiting_until
            "#,
            limit
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn finished_steps(&self, run_id: Uuid) -> sqlx::Result<Vec<StepRow>> {
        sqlx::query_as!(
            StepRow,
            r"
            SELECT position, succeeded, matched, message_id, list_item_id, channel_id, error,
                   finished_at, inputs, outputs
              FROM workflow_run_steps
             WHERE run_id = $1 AND succeeded
             ORDER BY position
            ",
            run_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn wake_time(&self, wait: &Wait) -> sqlx::Result<WakeTime> {
        let moment = wait.moment();
        sqlx::query_as!(
            WakeTime,
            r#"
            SELECT at AS "at!", at <= NOW() AS "due!",
                   at > NOW() + make_interval(days => $5) AS "too_far!"
              FROM (SELECT COALESCE(($1::date + $2::time) AT TIME ZONE $3,
                                    NOW() + make_interval(mins => $4::int)) AS at) w
            "#,
            moment.map(|(date, _, _)| date),
            moment.map(|(_, time, _)| time),
            moment.map(|(_, _, timezone)| timezone),
            wait.minutes().map(|minutes| minutes as i32),
            MAX_DELAY_DAYS as i32
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn sleep(
        &self,
        run_id: Uuid,
        position: i32,
        until: DateTime<Utc>,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            r"
            UPDATE workflow_runs
               SET status = 'waiting', waiting_at = $2, resume_at = $3
             WHERE id = $1
            ",
            run_id,
            position,
            until
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn resume_due(&self, limit: i64) -> sqlx::Result<Vec<Uuid>> {
        sqlx::query_scalar!(
            r"
            WITH due AS (
                SELECT id, waiting_at FROM workflow_runs
                 WHERE status = 'waiting' AND resume_at <= NOW()
                 ORDER BY resume_at
                 LIMIT $1
                 FOR UPDATE SKIP LOCKED
            ), slept AS (
                INSERT INTO workflow_run_steps (run_id, position, succeeded)
                SELECT id, waiting_at, TRUE FROM due WHERE waiting_at IS NOT NULL
                ON CONFLICT (run_id, position) DO UPDATE
                   SET succeeded = TRUE, error = NULL, finished_at = NOW()
            )
            UPDATE workflow_runs r
               SET status = 'pending', claimed_at = NULL, waiting_at = NULL, resume_at = NULL
              FROM due
             WHERE r.id = due.id
            RETURNING r.id
            ",
            limit
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn item_value(
        &self,
        item_id: Uuid,
        column_id: Uuid,
    ) -> sqlx::Result<Option<Option<serde_json::Value>>> {
        sqlx::query_scalar!(
            r#"SELECT "values" -> $2::text AS "value" FROM list_items
                WHERE id = $1 AND deleted_at IS NULL"#,
            item_id,
            column_id.to_string()
        )
        .fetch_optional(&self.pool)
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
        outcome: Result<Output, &str>,
    ) -> sqlx::Result<()> {
        let (succeeded, output, error) = match outcome {
            Ok(output) => (true, output, None),
            Err(error) => (false, Output::default(), Some(error)),
        };
        sqlx::query!(
            r"
            INSERT INTO workflow_run_steps
                   (run_id, position, succeeded, message_id, channel_id, matched, error)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (run_id, position) DO UPDATE
               SET succeeded = EXCLUDED.succeeded, message_id = EXCLUDED.message_id,
                   channel_id = EXCLUDED.channel_id, matched = EXCLUDED.matched,
                   error = EXCLUDED.error, finished_at = NOW()
            ",
            run_id,
            position,
            succeeded,
            output.message_id,
            output.channel_id,
            output.matched,
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
        outcome: Result<Output, &str>,
    ) -> sqlx::Result<()> {
        let mut conn = self.pool.acquire().await?;
        self.record_step_in(&mut conn, run_id, position, outcome)
            .await
    }

    pub async fn finish(
        &self,
        run_id: Uuid,
        status: RunStatus,
        error: Option<&str>,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            r"
            UPDATE workflow_runs
               SET status = $2, error = $3, waiting_at = NULL, resume_at = NULL,
                   finished_at = NOW()
             WHERE id = $1
            ",
            run_id,
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
            SELECT r.id, r.workflow_id, r.workflow_version, r.status AS "status: RunStatus",
                   r.context, r.error, r.started_at, r.finished_at, r.waiting_at,
                   COALESCE(r.resume_at, f.expires_at, c.expires_at) AS waiting_until
              FROM workflow_runs r
              LEFT JOIN workflow_forms f
                     ON f.run_id = r.id AND f.position = r.waiting_at AND f.answered_at IS NULL
              LEFT JOIN workflow_app_calls c
                     ON c.run_id = r.id AND c.position = r.waiting_at AND c.finished_at IS NULL
             WHERE r.workflow_id = $1
             ORDER BY r.started_at DESC, r.id
             LIMIT $2
            "#,
            workflow_id,
            limit
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn steps_of(&self, run_ids: &[Uuid]) -> sqlx::Result<Vec<(Uuid, StepRow)>> {
        let rows = sqlx::query!(
            r"
            SELECT run_id, position, succeeded, matched, message_id, list_item_id, channel_id,
                   error, finished_at, inputs, outputs
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
                    StepRow {
                        position: row.position,
                        succeeded: row.succeeded,
                        matched: row.matched,
                        message_id: row.message_id,
                        list_item_id: row.list_item_id,
                        channel_id: row.channel_id,
                        error: row.error,
                        finished_at: row.finished_at,
                        inputs: row.inputs,
                        outputs: row.outputs,
                    },
                )
            })
            .collect())
    }

    pub async fn touching_channel(
        &self,
        workspace_id: Uuid,
        channel_id: Uuid,
    ) -> sqlx::Result<Vec<ChannelWorkflow>> {
        sqlx::query_as!(
            ChannelWorkflow,
            r"
            SELECT id, name, enabled, created_by
              FROM workflows
             WHERE workspace_id = $1
               AND jsonb_path_exists(definition, 'strict $.**.channel_id ? (@ == $c)',
                                     jsonb_build_object('c', $2::text))
             ORDER BY lower(name)
            ",
            workspace_id,
            channel_id.to_string()
        )
        .fetch_all(&self.pool)
        .await
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
            "UPDATE workflow_runs SET status = 'waiting', waiting_at = $2 WHERE id = $1",
            run_id,
            position
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
                   status = 'pending', claimed_at = NULL, waiting_at = NULL
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
               SET status = 'expired', error = $1, waiting_at = NULL, finished_at = NOW()
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
        workspace_id: Option<Uuid>,
        created_by: Uuid,
    ) -> sqlx::Result<Vec<StoppedWorkflow>> {
        sqlx::query_as!(
            StoppedWorkflow,
            r"
            UPDATE workflows
               SET enabled = false, next_run_at = NULL, updated_at = NOW()
             WHERE ($1::uuid IS NULL OR workspace_id = $1) AND created_by = $2 AND enabled
            RETURNING id, workspace_id, name
            ",
            workspace_id,
            created_by
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn set_owner(&self, id: Uuid, owner: Uuid, token: &str) -> sqlx::Result<()> {
        sqlx::query!(
            r"
            UPDATE workflows
               SET created_by = $2, enabled = false, next_run_at = NULL,
                   webhook_token = CASE WHEN webhook_token IS NULL THEN NULL ELSE $3 END,
                   updated_at = NOW()
             WHERE id = $1
            ",
            id,
            owner,
            token
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn rotate_token(&self, id: Uuid, token: &str) -> sqlx::Result<bool> {
        let result = sqlx::query!(
            "UPDATE workflows SET webhook_token = $2, updated_at = NOW() WHERE id = $1 AND webhook_token IS NOT NULL",
            id,
            token
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn call_app_in(
        &self,
        conn: &mut PgConnection,
        call: NewCall<'_>,
    ) -> sqlx::Result<AppCall> {
        let saved = sqlx::query_as!(
            AppCall,
            r"
            INSERT INTO workflow_app_calls (run_id, position, app_id, title, inputs, outputs, expires_at)
            VALUES ($1, $2, $3, $4, $5, $6, NOW() + make_interval(secs => $7))
            ON CONFLICT (run_id, position) DO UPDATE
               SET app_id = EXCLUDED.app_id, title = EXCLUDED.title, inputs = EXCLUDED.inputs,
                   outputs = EXCLUDED.outputs, expires_at = EXCLUDED.expires_at, finished_at = NULL
            RETURNING id, expires_at
            ",
            call.run_id,
            call.position,
            call.app_id,
            call.title,
            call.inputs,
            call.outputs,
            call.timeout_secs
        )
        .fetch_one(&mut *conn)
        .await?;
        sqlx::query!(
            "UPDATE workflow_runs SET status = 'waiting', waiting_at = $2 WHERE id = $1",
            call.run_id,
            call.position
        )
        .execute(&mut *conn)
        .await?;
        Ok(saved)
    }

    pub async fn call_for_update_in(
        &self,
        conn: &mut PgConnection,
        id: Uuid,
    ) -> sqlx::Result<Option<OpenCall>> {
        sqlx::query_as!(
            OpenCall,
            r#"
            SELECT c.id, c.run_id, c.position, c.app_id, c.title, c.inputs, c.outputs,
                   c.expires_at, c.finished_at, r.status AS "run_status: RunStatus",
                   r.waiting_at, w.workspace_id
              FROM workflow_app_calls c
              JOIN workflow_runs r ON r.id = c.run_id
              JOIN workflows w ON w.id = r.workflow_id
             WHERE c.id = $1
             FOR UPDATE OF c, r
            "#,
            id
        )
        .fetch_optional(&mut *conn)
        .await
    }

    pub async fn settle_call_in(
        &self,
        conn: &mut PgConnection,
        call: &OpenCall,
        outcome: Result<&serde_json::Value, &str>,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            "UPDATE workflow_app_calls SET finished_at = NOW() WHERE id = $1",
            call.id
        )
        .execute(&mut *conn)
        .await?;
        let (outputs, error) = match outcome {
            Ok(outputs) => (Some(outputs), None),
            Err(error) => (None, Some(error)),
        };
        sqlx::query!(
            r"
            INSERT INTO workflow_run_steps (run_id, position, succeeded, inputs, outputs, error)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (run_id, position) DO UPDATE
               SET succeeded = EXCLUDED.succeeded, inputs = EXCLUDED.inputs,
                   outputs = EXCLUDED.outputs, error = EXCLUDED.error, finished_at = NOW()
            ",
            call.run_id,
            call.position,
            error.is_none(),
            call.inputs,
            outputs,
            error
        )
        .execute(&mut *conn)
        .await?;
        match error {
            None => {
                sqlx::query!(
                    r"
                    UPDATE workflow_runs
                       SET status = 'pending', claimed_at = NULL, waiting_at = NULL
                     WHERE id = $1
                    ",
                    call.run_id
                )
                .execute(&mut *conn)
                .await?;
            }
            Some(error) => {
                sqlx::query!(
                    r"
                    UPDATE workflow_runs
                       SET status = 'failed', error = $2, waiting_at = NULL, finished_at = NOW()
                     WHERE id = $1
                    ",
                    call.run_id,
                    format!("Step {} ({}): {error}", call.position + 1, call.title)
                )
                .execute(&mut *conn)
                .await?;
            }
        }
        Ok(())
    }

    pub async fn expire_calls(&self, limit: i64, reason: &str) -> sqlx::Result<Vec<Uuid>> {
        sqlx::query_scalar!(
            r"
            WITH due AS (
                SELECT c.id, c.run_id, c.position, c.title, c.inputs
                  FROM workflow_app_calls c
                 WHERE c.finished_at IS NULL AND c.expires_at <= NOW()
                 ORDER BY c.expires_at
                 LIMIT $1
                 FOR UPDATE SKIP LOCKED
            ), closed AS (
                UPDATE workflow_app_calls c SET finished_at = NOW()
                  FROM due WHERE c.id = due.id
            ), failed AS (
                INSERT INTO workflow_run_steps (run_id, position, succeeded, inputs, error)
                SELECT run_id, position, FALSE, inputs, $2::text FROM due
                ON CONFLICT (run_id, position) DO UPDATE
                   SET succeeded = FALSE, inputs = EXCLUDED.inputs, outputs = NULL,
                       error = EXCLUDED.error, finished_at = NOW()
            )
            UPDATE workflow_runs r
               SET status = 'failed',
                   error = 'Step ' || (due.position + 1) || ' (' || due.title || '): ' || $2::text,
                   waiting_at = NULL, finished_at = NOW()
              FROM due
             WHERE r.id = due.run_id AND r.status = 'waiting' AND r.waiting_at = due.position
            RETURNING r.id
            ",
            limit,
            reason
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn using_app(&self, app_id: Uuid) -> sqlx::Result<Vec<AppUser>> {
        sqlx::query_as!(
            AppUser,
            r#"
            SELECT id, workspace_id, name, created_by
              FROM workflows
             WHERE enabled
               AND jsonb_path_exists(definition,
                                     'strict $.**.app_id ? (@ == $a)',
                                     jsonb_build_object('a', $1::text))
             ORDER BY lower(name)
            "#,
            app_id.to_string()
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
