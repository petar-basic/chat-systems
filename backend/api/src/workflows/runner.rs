use std::sync::Arc;

use chrono::{Duration, Utc};
use tracing::{info, warn};
use uuid::Uuid;

use super::models::*;
use crate::authz;
use crate::state::AppState;
use crate::workspace::models::WorkspaceRole;

const TICK_SECS: u64 = 5;
const SCHEDULE_BATCH: i64 = 50;
const RUN_BATCH: i64 = 20;
const LATE_LIMIT_MINUTES: i64 = 15;

pub async fn start_runner(state: Arc<AppState>) {
    info!("Workflow runner started");
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(TICK_SECS)).await;
        if let Err(e) = queue_due_schedules(&state).await {
            warn!("Failed to queue scheduled workflows: {}", e);
        }
        if let Err(e) = run_pending(&state).await {
            warn!("Failed to run pending workflows: {}", e);
        }
    }
}

pub async fn queue_due_schedules(state: &AppState) -> sqlx::Result<usize> {
    let mut tx = state.pool.begin().await?;
    let due = state
        .workflow_repo
        .due_schedules_in(&mut tx, SCHEDULE_BATCH)
        .await?;
    let mut queued = 0;
    let late_limit = Utc::now() - Duration::minutes(LATE_LIMIT_MINUTES);
    for workflow in &due {
        let definition: Option<Definition> =
            serde_json::from_value(workflow.definition.clone()).ok();
        let schedule = definition.as_ref().and_then(|d| d.trigger.schedule());
        if schedule.is_some() && workflow.next_run_at >= late_limit {
            state
                .workflow_repo
                .queue_run_in(
                    &mut tx,
                    workflow.id,
                    workflow.version,
                    &format!("schedule:{}", workflow.next_run_at.to_rfc3339()),
                    &serde_json::json!({}),
                )
                .await?;
            queued += 1;
        }
        state
            .workflow_repo
            .advance_in(&mut tx, workflow.id, schedule.as_ref())
            .await?;
    }
    tx.commit().await?;
    Ok(queued)
}

pub async fn run_pending(state: &AppState) -> sqlx::Result<usize> {
    let runs = state.workflow_repo.claim_runs(RUN_BATCH).await?;
    let count = runs.len();
    for run in runs {
        execute(state, &run).await;
    }
    Ok(count)
}

struct Context<'a> {
    workflow: &'a WorkflowRow,
    run: &'a RunRow,
}

impl Context<'_> {
    fn value(&self, name: &str) -> String {
        let Some(variable) = name.strip_prefix("webhook.") else {
            return String::new();
        };
        match &self.run.context["webhook"][variable] {
            serde_json::Value::String(text) => text.clone(),
            serde_json::Value::Null => String::new(),
            other => other.to_string(),
        }
    }

    fn rendered(&self, text: &str) -> String {
        render(text, |name| self.value(name))
    }

    fn authored(&self, text: &str) -> String {
        render(text, |name| {
            if name.starts_with("webhook.") {
                String::new()
            } else {
                self.value(name)
            }
        })
    }

    fn bot(&self) -> serde_json::Value {
        serde_json::json!({ "workflow_id": self.workflow.id, "name": self.workflow.name })
    }
}

pub async fn execute(state: &AppState, run: &RunRow) {
    let workflow = match state.workflow_repo.find(run.workflow_id).await {
        Ok(Some(workflow)) => workflow,
        Ok(None) => return,
        Err(e) => {
            warn!(run_id = %run.id, "could not load a workflow to run: {}", e);
            return;
        }
    };
    let outcome = steps(state, &workflow, run).await;
    let (status, error) = match &outcome {
        Ok(()) => (RunStatus::Succeeded, None),
        Err(reason) => (RunStatus::Failed, Some(reason.as_str())),
    };
    if let Err(e) = state
        .workflow_repo
        .finish(run.id, workflow.version, status, error)
        .await
    {
        warn!(run_id = %run.id, "could not finish a workflow run: {}", e);
    }
}

async fn steps(state: &AppState, workflow: &WorkflowRow, run: &RunRow) -> Result<(), String> {
    if !workflow.enabled {
        return Err("The workflow was turned off before it ran".into());
    }
    let creator = state
        .workspace_service
        .repo
        .get_member(workflow.workspace_id, workflow.created_by)
        .await
        .map_err(|e| e.to_string())?;
    let Some(creator) = creator else {
        super::service::creator_left(state, workflow.workspace_id, workflow.created_by).await;
        return Err("The person who built it is no longer in this workspace".into());
    };
    let definition = workflow
        .definition()
        .ok_or_else(|| "The workflow can't be read".to_string())?;
    let done = state
        .workflow_repo
        .finished_positions(run.id)
        .await
        .map_err(|e| e.to_string())?;
    let context = Context { workflow, run };

    for (index, step) in definition.steps.iter().enumerate() {
        let position = index as i32;
        if done.contains(&position) {
            continue;
        }
        if let Err(reason) = run_step(state, &context, &creator, position, step).await {
            if let Err(e) = state
                .workflow_repo
                .record_step(run.id, position, Err(&reason))
                .await
            {
                warn!(run_id = %run.id, "could not record a failed step: {}", e);
            }
            return Err(format!("Step {} ({}): {reason}", index + 1, step.label()));
        }
    }
    Ok(())
}

async fn run_step(
    state: &AppState,
    context: &Context<'_>,
    creator: &crate::workspace::models::WorkspaceMember,
    position: i32,
    step: &Step,
) -> Result<(), String> {
    let channel_id = match step {
        Step::SendMessage { channel_id, .. } => {
            let access = authz::require_channel_post(state, *channel_id, creator.user_id)
                .await
                .map_err(|_| "Its creator can no longer post in that channel".to_string())?;
            if access.channel.is_archived {
                return Err("That channel is archived".into());
            }
            *channel_id
        }
        Step::SendDm { to, .. } => {
            let recipient = match to {
                Recipient::Person { user_id } => *user_id,
                Recipient::TriggerUser => return Err("This trigger has no person".into()),
            };
            dm_channel(state, creator, recipient).await?
        }
    };

    let text = step.text();
    let content = context.rendered(text);
    let mentioned = crate::messaging::routes::expand_mentions(
        state,
        channel_id,
        creator.user_id,
        &context.authored(text),
    )
    .await;

    let internal = |e: sqlx::Error| e.to_string();
    let mut tx = state.pool.begin().await.map_err(internal)?;
    let message = state
        .message_repo
        .create_bot_message_in(
            &mut tx,
            channel_id,
            creator.user_id,
            &content,
            &context.bot(),
            &mentioned,
        )
        .await
        .map_err(internal)?;
    state
        .workflow_repo
        .record_step_in(&mut tx, context.run.id, position, Ok(Some(message.id)))
        .await
        .map_err(internal)?;
    let staged = state
        .publisher
        .stage_message_created(&mut tx, &message, context.workflow.workspace_id, &mentioned)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(internal)?;
    state.publisher.dispatch(staged).await;
    Ok(())
}

async fn dm_channel(
    state: &AppState,
    creator: &crate::workspace::models::WorkspaceMember,
    recipient: Uuid,
) -> Result<Uuid, String> {
    if recipient == creator.user_id {
        return Err("A workflow can't send its creator a DM".into());
    }
    let present = state
        .workspace_service
        .repo
        .get_member(creator.workspace_id, recipient)
        .await
        .map_err(|e| e.to_string())?;
    if present.is_none() {
        return Err("That person is no longer in this workspace".into());
    }
    if creator.role == WorkspaceRole::Guest {
        crate::conversations::routes::require_reachable(state, creator, recipient)
            .await
            .map_err(|_| "Its creator can no longer message that person".to_string())?;
    }
    crate::conversations::routes::open_direct(
        state,
        creator.workspace_id,
        creator.user_id,
        recipient,
    )
    .await
    .map(|conversation| conversation.id)
    .map_err(|e| e.to_string())
}
