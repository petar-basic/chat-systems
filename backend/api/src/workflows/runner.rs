use std::collections::HashMap;
use std::sync::Arc;

use chrono::{Duration, Utc};
use tracing::{info, warn};
use uuid::Uuid;

use shared_common::errors::AppError;

use super::models::*;
use crate::authz;
use crate::messaging::models::MessageField;
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
        if let Err(e) = expire_forms(&state).await {
            warn!("Failed to expire workflow forms: {}", e);
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

pub async fn expire_forms(state: &AppState) -> sqlx::Result<usize> {
    let mut tx = state.pool.begin().await?;
    let expired = state
        .workflow_repo
        .expire_forms_in(
            &mut tx,
            &format!("Nobody filled in the form within {FORM_EXPIRY_DAYS} days"),
        )
        .await?;
    let mut staged = Vec::with_capacity(expired.len());
    for form in &expired {
        let Some(message) = state
            .message_repo
            .mark_form_in(
                &mut tx,
                form.message_id,
                "expired",
                &serde_json::json!(true),
            )
            .await?
        else {
            continue;
        };
        match state
            .publisher
            .stage_message_updated(&mut tx, &message, form.workspace_id)
            .await
        {
            Ok(event) => staged.push(event),
            Err(e) => warn!("could not stage an expired form: {}", e),
        }
    }
    tx.commit().await?;
    state.publisher.dispatch_all(staged).await;
    Ok(expired.len())
}

enum Outcome {
    Done,
    Waiting,
}

struct Context<'a> {
    workflow: &'a WorkflowRow,
    run: &'a RunRow,
    definition: &'a Definition,
    fired: Fired,
}

struct Fired {
    user: Option<Uuid>,
    values: HashMap<&'static str, String>,
}

fn text_of(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

impl Context<'_> {
    fn value(&self, name: &str) -> String {
        if let Some(variable) = name.strip_prefix("webhook.") {
            return text_of(&self.run.context["webhook"][variable]);
        }
        if let Some(field) = name.strip_prefix("form.") {
            return text_of(&self.run.context["form"][field]);
        }
        self.fired.values.get(name).cloned().unwrap_or_default()
    }

    fn trusted(&self, name: &str) -> bool {
        if name.starts_with("webhook.") || name == "trigger.message" {
            return false;
        }
        match name.strip_prefix("form.") {
            Some(field) => self
                .definition
                .fields_before(self.definition.steps.len())
                .any(|f| f.name == field && f.kind == FieldKind::Person),
            None => true,
        }
    }

    fn rendered(&self, text: &str) -> String {
        render(text, |name| self.value(name))
    }

    fn authored(&self, text: &str) -> String {
        render(text, |name| {
            if self.trusted(name) {
                self.value(name)
            } else {
                String::new()
            }
        })
    }

    fn answers(&self, position: usize, only_trusted: bool) -> Vec<MessageField> {
        self.definition
            .fields_before(position)
            .map(|field| {
                let name = format!("form.{}", field.name);
                let value = if only_trusted && !self.trusted(&name) {
                    String::new()
                } else {
                    self.value(&name)
                };
                MessageField {
                    label: field.label.trim().to_string(),
                    value,
                }
            })
            .collect()
    }

    fn metadata(&self) -> serde_json::Value {
        serde_json::json!({ "bot": { "workflow_id": self.workflow.id, "name": self.workflow.name } })
    }
}

fn summary(answers: &[MessageField]) -> String {
    answers
        .iter()
        .map(|answer| {
            let value = if answer.value.trim().is_empty() {
                "\u{2014}".to_string()
            } else {
                answer.value.replace('\n', "  \n")
            };
            format!("**{}**  \n{value}", answer.label)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
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
    let (status, error) = match steps(state, &workflow, run).await {
        Ok(Outcome::Waiting) => return,
        Ok(Outcome::Done) => (RunStatus::Succeeded, None),
        Err(reason) => (RunStatus::Failed, Some(reason)),
    };
    if let Err(e) = state
        .workflow_repo
        .finish(run.id, workflow.version, status, error.as_deref())
        .await
    {
        warn!(run_id = %run.id, "could not finish a workflow run: {}", e);
    }
}

async fn steps(state: &AppState, workflow: &WorkflowRow, run: &RunRow) -> Result<Outcome, String> {
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
    if let Some(channel_id) = definition.trigger.channel() {
        authz::require_channel_access(state, channel_id, creator.user_id)
            .await
            .map_err(|_| "Its creator can no longer see the channel it listens to".to_string())?;
    }
    let done = state
        .workflow_repo
        .finished_positions(run.id)
        .await
        .map_err(|e| e.to_string())?;
    if !done.is_empty() && workflow.version != run.workflow_version {
        return Err("The workflow was edited while this run was under way".into());
    }
    let context = Context {
        workflow,
        run,
        definition: &definition,
        fired: fired(state, workflow, run)
            .await
            .map_err(|e| e.to_string())?,
    };

    for (index, step) in definition.steps.iter().enumerate() {
        let position = index as i32;
        if done.contains(&position) {
            continue;
        }
        match run_step(state, &context, &creator, position, step).await {
            Ok(Outcome::Done) => {}
            Ok(Outcome::Waiting) => return Ok(Outcome::Waiting),
            Err(reason) => {
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
    }
    Ok(Outcome::Done)
}

fn recipient_of(to: &Recipient, context: &Context<'_>) -> Result<Uuid, String> {
    match (to, context.fired.user) {
        (Recipient::Person { user_id }, _) => Ok(*user_id),
        (Recipient::TriggerUser, Some(user_id)) => Ok(user_id),
        (Recipient::TriggerUser, None) => Err("This trigger has no person".into()),
    }
}

async fn run_step(
    state: &AppState,
    context: &Context<'_>,
    creator: &crate::workspace::models::WorkspaceMember,
    position: i32,
    step: &Step,
) -> Result<Outcome, String> {
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
            let recipient = recipient_of(to, context)?;
            if recipient == creator.user_id && *to == Recipient::TriggerUser {
                state
                    .workflow_repo
                    .record_step(context.run.id, position, Ok(None))
                    .await
                    .map_err(|e| e.to_string())?;
                return Ok(Outcome::Done);
            }
            dm_channel(state, creator, recipient).await?
        }
        Step::AddListItem { list_id, cells } => {
            return write_item(
                state,
                context,
                creator.user_id,
                position,
                Item::New(*list_id),
                cells,
            )
            .await;
        }
        Step::UpdateListItem { from_step, cells } => {
            return write_item(
                state,
                context,
                creator.user_id,
                position,
                Item::From(*from_step),
                cells,
            )
            .await;
        }
        Step::Form { to, title, fields } => {
            let recipient = recipient_of(to, context)?;
            let channel_id = if recipient == creator.user_id {
                let channel_id = context
                    .definition
                    .trigger
                    .channel()
                    .ok_or_else(|| "A workflow can't send its creator a form".to_string())?;
                authz::require_channel_post(state, channel_id, creator.user_id)
                    .await
                    .map_err(|_| "Its creator can no longer post in that channel".to_string())?;
                channel_id
            } else {
                dm_channel(state, creator, recipient).await?
            };
            post_form(
                state,
                context,
                creator.user_id,
                position,
                channel_id,
                recipient,
                title.trim(),
                fields,
            )
            .await
            .map_err(|e| e.to_string())?;
            return Ok(Outcome::Waiting);
        }
    };

    let text = step.text().unwrap_or_default();
    let mut metadata = context.metadata();
    let (content, authored) = if step.summary() {
        let at = position as usize;
        let answers = context.answers(at, false);
        let content = format!("{}\n\n{}", context.rendered(text), summary(&answers));
        let authored = format!(
            "{}\n\n{}",
            context.authored(text),
            summary(&context.answers(at, true))
        );
        metadata["fields"] = serde_json::json!(answers);
        (content, authored)
    } else {
        (context.rendered(text), context.authored(text))
    };
    let mentioned =
        crate::messaging::routes::expand_mentions(state, channel_id, creator.user_id, &authored)
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
            &metadata,
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
    Ok(Outcome::Done)
}

enum Item {
    New(Uuid),
    From(usize),
}

fn list_error(error: AppError) -> String {
    match error {
        AppError::Validation(message)
        | AppError::NotFound(message)
        | AppError::Forbidden(message)
        | AppError::Conflict(message) => message,
        other => other.to_string(),
    }
}

async fn write_item(
    state: &AppState,
    context: &Context<'_>,
    creator: Uuid,
    position: i32,
    target: Item,
    cells: &[CellValue],
) -> Result<Outcome, String> {
    let (list_id, item_id) = match target {
        Item::New(list_id) => (list_id, None),
        Item::From(step) => {
            let item_id = state
                .workflow_repo
                .item_of(context.run.id, step as i32)
                .await
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "The item it updates was deleted".to_string())?;
            let list_id = context
                .definition
                .list_of(step)
                .ok_or_else(|| "The step that added the item is gone".to_string())?;
            (list_id, Some(item_id))
        }
    };
    let list = crate::lists::service::writable(state, creator, list_id)
        .await
        .map_err(|_| "Its creator can no longer add to that list".to_string())?;
    let columns = state
        .list_repo
        .columns(list_id)
        .await
        .map_err(|e| e.to_string())?;
    let mut values = serde_json::Map::new();
    for cell in cells {
        let column = columns
            .iter()
            .find(|column| column.id == cell.column_id)
            .ok_or_else(|| "A column it fills was deleted".to_string())?;
        if let Some(value) = super::cells::value(column, &context.rendered(&cell.value))? {
            values.insert(column.id.to_string(), value);
        }
    }

    let mut tx = state.pool.begin().await.map_err(|e| e.to_string())?;
    let write = match item_id {
        None => {
            crate::lists::service::create_item_in(state, &mut tx, creator, list_id, &values, None)
                .await
        }
        Some(item_id) => {
            crate::lists::service::update_item_in(state, &mut tx, creator, &list, item_id, &values)
                .await
        }
    }
    .map_err(list_error)?;
    state
        .workflow_repo
        .record_item_step_in(&mut tx, context.run.id, position, write.item.id)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    write.announce(state).await;
    Ok(Outcome::Done)
}

#[allow(clippy::too_many_arguments)]
async fn post_form(
    state: &AppState,
    context: &Context<'_>,
    creator: Uuid,
    position: i32,
    channel_id: Uuid,
    recipient: Uuid,
    title: &str,
    fields: &[FormField],
) -> shared_common::errors::AppResult<()> {
    let expires_at = Utc::now() + Duration::days(FORM_EXPIRY_DAYS);
    let form = FormRequest {
        run_id: context.run.id,
        position,
        title: title.to_string(),
        recipient_id: recipient,
        fields: fields.to_vec(),
        expires_at,
        submitted_at: None,
        expired: false,
    };
    let form = serde_json::to_value(&form)
        .map_err(|e| shared_common::errors::AppError::Internal(e.to_string()))?;
    let fields = serde_json::to_value(fields)
        .map_err(|e| shared_common::errors::AppError::Internal(e.to_string()))?;

    let mut tx = state.pool.begin().await?;
    let message = state
        .message_repo
        .create_bot_message_in(
            &mut tx,
            channel_id,
            creator,
            title,
            &context.metadata(),
            &[],
        )
        .await?;
    let message = state
        .message_repo
        .set_form_in(&mut tx, message.id, &form)
        .await?;
    state
        .workflow_repo
        .wait_in(
            &mut tx,
            context.run.id,
            position,
            recipient,
            message.id,
            &fields,
            expires_at,
        )
        .await?;
    let staged = state
        .publisher
        .stage_message_created(&mut tx, &message, context.workflow.workspace_id, &[])
        .await?;
    tx.commit().await?;
    state.publisher.dispatch(staged).await;
    Ok(())
}

pub(super) async fn mention_of(
    state: &AppState,
    workspace_id: Uuid,
    user_id: Uuid,
) -> sqlx::Result<Option<String>> {
    Ok(state
        .profile_repo
        .name(workspace_id, user_id)
        .await?
        .map(|name| format!("@[{name}]({user_id})")))
}

async fn fired(state: &AppState, workflow: &WorkflowRow, run: &RunRow) -> sqlx::Result<Fired> {
    let facts = &run.context["trigger"];
    let id = |key: &str| facts.get(key)?.as_str()?.parse::<Uuid>().ok();
    let mut values = HashMap::new();
    let user = id("user_id");
    if let Some(user_id) = user {
        let mention = mention_of(state, workflow.workspace_id, user_id)
            .await?
            .unwrap_or_else(|| format!("@[someone]({user_id})"));
        values.insert("trigger.user", mention);
    }
    if let Some(channel_id) = id("channel_id") {
        let channel = state
            .workspace_service
            .repo
            .find_channel_by_id(channel_id)
            .await?;
        if let Some(name) = channel.and_then(|channel| channel.name) {
            values.insert("trigger.channel", format!("#{name}"));
        }
        if let Some(message_id) = id("message_id") {
            let message = state.message_repo.find_by_id(message_id).await?;
            values.insert(
                "trigger.message",
                message
                    .filter(|message| message.deleted_at.is_none())
                    .map(|message| message.content)
                    .unwrap_or_default(),
            );
            values.insert(
                "trigger.message_link",
                format!(
                    "{}/app/{}/{channel_id}/{message_id}",
                    state.config.public_url, workflow.workspace_id
                ),
            );
        }
    }
    Ok(Fired { user, values })
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
