use sqlx::PgPool;
use uuid::Uuid;

use super::models::{AppWorkflowStepRow, ProviderRow, WorkflowStepInput};

#[derive(Clone)]
pub struct AppWorkflowStepRepo {
    pool: PgPool,
}

impl AppWorkflowStepRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list(&self, app_id: Uuid) -> sqlx::Result<Vec<AppWorkflowStepRow>> {
        sqlx::query_as!(
            AppWorkflowStepRow,
            "SELECT id, app_id, callback_id, title, description, inputs, outputs
               FROM app_workflow_steps
              WHERE app_id = $1
              ORDER BY position",
            app_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn replace(
        &self,
        app_id: Uuid,
        steps: &[WorkflowStepInput],
    ) -> sqlx::Result<Vec<AppWorkflowStepRow>> {
        let callback_ids: Vec<String> = steps.iter().map(|s| s.callback_id.clone()).collect();
        let titles: Vec<String> = steps.iter().map(|s| s.title.trim().to_string()).collect();
        let descriptions: Vec<String> = steps
            .iter()
            .map(|s| s.description.trim().to_string())
            .collect();
        let inputs: Vec<serde_json::Value> = steps
            .iter()
            .map(|s| serde_json::to_value(trimmed(&s.inputs)).unwrap_or_default())
            .collect();
        let outputs: Vec<serde_json::Value> = steps
            .iter()
            .map(|s| serde_json::to_value(trimmed(&s.outputs)).unwrap_or_default())
            .collect();
        let positions: Vec<i16> = (0..steps.len())
            .map(|i| i16::try_from(i).unwrap_or(i16::MAX))
            .collect();

        let mut tx = self.pool.begin().await?;
        sqlx::query!(
            "DELETE FROM app_workflow_steps WHERE app_id = $1 AND callback_id <> ALL($2::text[])",
            app_id,
            &callback_ids
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "INSERT INTO app_workflow_steps (app_id, callback_id, title, description, inputs, outputs, position)
             SELECT $1, callback_id, title, description, inputs, outputs, position
               FROM UNNEST($2::text[], $3::text[], $4::text[], $5::jsonb[], $6::jsonb[], $7::smallint[])
                 AS t(callback_id, title, description, inputs, outputs, position)
             ON CONFLICT (app_id, callback_id) DO UPDATE
                SET title = EXCLUDED.title, description = EXCLUDED.description,
                    inputs = EXCLUDED.inputs, outputs = EXCLUDED.outputs,
                    position = EXCLUDED.position",
            app_id,
            &callback_ids,
            &titles,
            &descriptions,
            &inputs,
            &outputs,
            &positions
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        self.list(app_id).await
    }

    pub async fn providers(
        &self,
        workspace_id: Uuid,
    ) -> sqlx::Result<(Vec<ProviderRow>, Vec<AppWorkflowStepRow>)> {
        let apps = sqlx::query_as!(
            ProviderRow,
            r#"SELECT b.id, b.workspace_id, b.name, b.icon_url,
                      b.disabled_at IS NOT NULL AS "disabled!",
                      'workflow.steps:execute' = ANY(b.scopes) AS "scoped!",
                      'function_executed' = ANY(b.events) AS "listening!",
                      (b.socket_mode OR b.request_url IS NOT NULL) AS "reachable!"
                 FROM bot_apps b
                WHERE b.workspace_id = $1
                  AND EXISTS (SELECT 1 FROM app_workflow_steps s WHERE s.app_id = b.id)
                ORDER BY lower(b.name), b.id"#,
            workspace_id
        )
        .fetch_all(&self.pool)
        .await?;
        let ids: Vec<Uuid> = apps.iter().map(|app| app.id).collect();
        let steps = sqlx::query_as!(
            AppWorkflowStepRow,
            "SELECT id, app_id, callback_id, title, description, inputs, outputs
               FROM app_workflow_steps
              WHERE app_id = ANY($1)
              ORDER BY app_id, position",
            &ids
        )
        .fetch_all(&self.pool)
        .await?;
        Ok((apps, steps))
    }

    pub async fn provider(
        &self,
        app_id: Uuid,
        callback_id: &str,
    ) -> sqlx::Result<Option<(ProviderRow, Option<AppWorkflowStepRow>)>> {
        let Some(app) = sqlx::query_as!(
            ProviderRow,
            r#"SELECT b.id, b.workspace_id, b.name, b.icon_url,
                      b.disabled_at IS NOT NULL AS "disabled!",
                      'workflow.steps:execute' = ANY(b.scopes) AS "scoped!",
                      'function_executed' = ANY(b.events) AS "listening!",
                      (b.socket_mode OR b.request_url IS NOT NULL) AS "reachable!"
                 FROM bot_apps b
                WHERE b.id = $1"#,
            app_id
        )
        .fetch_optional(&self.pool)
        .await?
        else {
            return Ok(None);
        };
        let step = sqlx::query_as!(
            AppWorkflowStepRow,
            "SELECT id, app_id, callback_id, title, description, inputs, outputs
               FROM app_workflow_steps
              WHERE app_id = $1 AND callback_id = $2",
            app_id,
            callback_id
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(Some((app, step)))
    }
}

fn trimmed(params: &[super::models::StepParam]) -> Vec<super::models::StepParam> {
    params
        .iter()
        .map(|param| super::models::StepParam {
            title: param.title.trim().to_string(),
            ..param.clone()
        })
        .collect()
}
