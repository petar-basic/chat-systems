use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::{Duration, NaiveDate, Utc};
use serde_json::{json, Value};
use tracing::{info, warn};
use uuid::Uuid;

use shared_common::errors::AppError;
use shared_events::Event;

use super::models::*;
use super::repo::{NewCall, Output, StepRow};
use crate::app_events::{self, repo::NewDelivery};
use crate::app_workflow_steps::models::{ProviderRow, StepParam, StepParamKind, STEP_TIMEOUT_SECS};
use crate::audit::ClientIp;
use crate::authz::{self, Stopped};
use crate::bots::models::AppEvent;
use crate::messaging::models::{Message, MessageField};
use crate::profile_fields::models::ProfileFieldKind;
use crate::state::AppState;
use crate::workspace::details::{self, Details};
use crate::workspace::membership::{self, stand_down};
use crate::workspace::models::{Channel, ChannelRole, ChannelType, WorkspaceMember, WorkspaceRole};
use crate::workspace::repo::WorkspaceRepo;

const TICK_SECS: u64 = 5;
const SCHEDULE_BATCH: i64 = 50;
const RUN_BATCH: i64 = 20;
const RESUME_BATCH: i64 = 50;
const LATE_LIMIT_MINUTES: i64 = 15;
const APP_TIMED_OUT: &str = "The app did not finish";

pub async fn start_runner(state: Arc<AppState>) {
    info!("Workflow runner started");
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(TICK_SECS)).await;
        if let Err(e) = queue_due_schedules(&state).await {
            warn!("Failed to queue scheduled workflows: {}", e);
        }
        if let Err(e) = resume_delays(&state).await {
            warn!("Failed to resume waiting workflows: {}", e);
        }
        if let Err(e) = run_pending(&state).await {
            warn!("Failed to run pending workflows: {}", e);
        }
        if let Err(e) = expire_forms(&state).await {
            warn!("Failed to expire workflow forms: {}", e);
        }
        if let Err(e) = expire_app_calls(&state).await {
            warn!("Failed to time out workflow app steps: {}", e);
        }
    }
}

pub async fn expire_app_calls(state: &AppState) -> sqlx::Result<usize> {
    Ok(state
        .workflow_repo
        .expire_calls(RESUME_BATCH, APP_TIMED_OUT)
        .await?
        .len())
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

pub async fn resume_delays(state: &AppState) -> sqlx::Result<usize> {
    Ok(state.workflow_repo.resume_due(RESUME_BATCH).await?.len())
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

enum Kept {
    Nothing,
    Matched(bool),
    Channel(Channel),
    Reply { channel_id: Uuid, message_id: Uuid },
    App(Vec<AppOutput>),
}

struct AppOutput {
    name: String,
    text: String,
    raw: String,
}

enum Outcome {
    Done(Kept),
    Waiting,
}

struct Context<'a> {
    workflow: &'a WorkflowRow,
    run: &'a RunRow,
    definition: &'a Definition,
    fired: Fired,
    base: String,
    outputs: HashMap<String, String>,
    raw: HashMap<String, String>,
    channels: HashMap<usize, Uuid>,
}

struct Fired {
    user: Option<Uuid>,
    channel: Option<Uuid>,
    message: Option<Uuid>,
    values: HashMap<&'static str, String>,
}

fn text_of(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn reason(error: AppError) -> String {
    match error {
        AppError::Validation(message)
        | AppError::NotFound(message)
        | AppError::Forbidden(message)
        | AppError::Conflict(message) => message,
        other => other.to_string(),
    }
}

fn internal(error: impl std::fmt::Display) -> String {
    error.to_string()
}

impl Context<'_> {
    fn value(&self, name: &str) -> String {
        if let Some(variable) = name.strip_prefix("webhook.") {
            return text_of(&self.run.context["webhook"][variable]);
        }
        if let Some(field) = name.strip_prefix("form.") {
            return text_of(&self.run.context["form"][field]);
        }
        if name.starts_with("step.") {
            return self.outputs.get(name).cloned().unwrap_or_default();
        }
        self.fired.values.get(name).cloned().unwrap_or_default()
    }

    fn typed(&self, name: &str) -> String {
        match name {
            "trigger.user" => return self.fired.user.map(|id| id.to_string()).unwrap_or_default(),
            "trigger.channel" => {
                return self
                    .fired
                    .channel
                    .map(|id| id.to_string())
                    .unwrap_or_default();
            }
            _ => {}
        }
        if let Some(raw) = self.raw.get(name) {
            return raw.clone();
        }
        let made = name
            .strip_prefix("step.")
            .and_then(|rest| rest.strip_suffix(".channel"))
            .and_then(|number| number.parse::<usize>().ok())
            .and_then(|number| number.checked_sub(1))
            .and_then(|at| self.channels.get(&at));
        match made {
            Some(id) => id.to_string(),
            None => self.value(name),
        }
    }

    fn trusted(&self, name: &str) -> bool {
        if name.starts_with("webhook.") || name.starts_with("step.") || name == "trigger.message" {
            return false;
        }
        match name.strip_prefix("form.") {
            Some(field) => self
                .definition
                .fields()
                .iter()
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
            .into_iter()
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

    fn keep(&mut self, position: usize, kept: &Kept, taken: &mut HashMap<usize, bool>) {
        let number = position + 1;
        match kept {
            Kept::Nothing => {}
            Kept::Matched(matched) => {
                taken.insert(position, *matched);
            }
            Kept::Channel(channel) => {
                self.channels.insert(position, channel.id);
                self.outputs.insert(
                    format!("step.{number}.channel"),
                    format!("#{}", channel.name.as_deref().unwrap_or_default()),
                );
                self.outputs.insert(
                    format!("step.{number}.channel_link"),
                    format!("{}/{}", self.base, channel.id),
                );
            }
            Kept::Reply {
                channel_id,
                message_id,
            } => {
                self.outputs.insert(
                    format!("step.{number}.message_link"),
                    format!("{}/{channel_id}/{message_id}", self.base),
                );
            }
            Kept::App(outputs) => {
                for output in outputs {
                    let name = format!("step.{number}.{}", output.name);
                    self.outputs.insert(name.clone(), output.text.clone());
                    self.raw.insert(name, output.raw.clone());
                }
            }
        }
    }

    async fn app_outputs(
        &self,
        state: &AppState,
        declared: &[StepParam],
        values: &Value,
    ) -> Vec<AppOutput> {
        let workspace_id = self.workflow.workspace_id;
        let mut kept = Vec::with_capacity(declared.len());
        for param in declared {
            let raw = match &values[&param.name] {
                Value::Null => String::new(),
                Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            let id = raw.parse::<Uuid>().ok();
            let text = match (param.kind, id) {
                (StepParamKind::Person, Some(id)) => mention_of(state, workspace_id, id)
                    .await
                    .ok()
                    .flatten()
                    .unwrap_or_default(),
                (StepParamKind::Channel, Some(id)) => state
                    .workspace_service
                    .repo
                    .find_channel_by_id(id)
                    .await
                    .ok()
                    .flatten()
                    .and_then(|channel| channel.name)
                    .map(|name| format!("#{name}"))
                    .unwrap_or_default(),
                (StepParamKind::Person | StepParamKind::Channel, None) => String::new(),
                _ => raw.clone(),
            };
            kept.push(AppOutput {
                name: param.name.clone(),
                text,
                raw,
            });
        }
        kept
    }

    async fn recall(&self, state: &AppState, step: Option<&Step>, row: &StepRow) -> Kept {
        if let Some(matched) = row.matched {
            return Kept::Matched(matched);
        }
        if let (Some(Step::AppStep { outputs, .. }), Some(values)) = (step, row.outputs.as_ref()) {
            return Kept::App(self.app_outputs(state, outputs, values).await);
        }
        match (step, row.channel_id, row.message_id, self.fired.channel) {
            (Some(Step::CreateChannel { .. }), Some(channel_id), _, _) => {
                match state
                    .workspace_service
                    .repo
                    .find_channel_by_id(channel_id)
                    .await
                {
                    Ok(Some(channel)) => Kept::Channel(channel),
                    _ => Kept::Nothing,
                }
            }
            (Some(Step::ReplyInThread { .. }), _, Some(message_id), Some(channel_id)) => {
                Kept::Reply {
                    channel_id,
                    message_id,
                }
            }
            _ => Kept::Nothing,
        }
    }

    async fn channel(&self, state: &AppState, channel: &ChannelRef) -> Result<Channel, String> {
        let id = match channel {
            ChannelRef::Channel { channel_id } => *channel_id,
            ChannelRef::TriggerChannel => self
                .fired
                .channel
                .ok_or_else(|| "This trigger has no channel".to_string())?,
            ChannelRef::FromStep { step } => *self
                .channels
                .get(step)
                .ok_or_else(|| "The channel that step made is gone".to_string())?,
        };
        state
            .workspace_service
            .repo
            .find_channel_by_id(id)
            .await
            .map_err(internal)?
            .filter(|channel| channel.workspace_id == self.workflow.workspace_id)
            .ok_or_else(|| "That channel was deleted".to_string())
    }

    async fn trigger_message(&self, state: &AppState) -> Result<Message, String> {
        let id = self
            .fired
            .message
            .ok_or_else(|| "This trigger has no message".to_string())?;
        state
            .message_repo
            .find_by_id(id)
            .await
            .map_err(internal)?
            .filter(|message| message.deleted_at.is_none())
            .ok_or_else(|| "The message it acts on was deleted".to_string())
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

fn channel_name(rendered: &str) -> String {
    rendered
        .trim()
        .trim_start_matches('#')
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
        .to_lowercase()
        .trim_matches('-')
        .to_string()
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
        Ok(Outcome::Done(_)) => (RunStatus::Succeeded, None),
        Err(reason) => (RunStatus::Failed, Some(reason)),
    };
    if let Err(e) = state
        .workflow_repo
        .finish(run.id, status, error.as_deref())
        .await
    {
        warn!(run_id = %run.id, "could not finish a workflow run: {}", e);
    }
}

async fn steps(state: &AppState, workflow: &WorkflowRow, run: &RunRow) -> Result<Outcome, String> {
    if !workflow.enabled {
        return Err("The workflow was turned off before it ran".into());
    }
    let definition: Definition = state
        .workflow_repo
        .definition_at(workflow.id, run.workflow_version)
        .await
        .map_err(internal)?
        .and_then(|value| serde_json::from_value(value).ok())
        .ok_or_else(|| "The workflow can't be read".to_string())?;
    let stopped = authz::still_acting(
        state,
        workflow.workspace_id,
        workflow.created_by,
        definition.trigger.channel(),
    )
    .await
    .map_err(internal)?;
    match stopped {
        None => {}
        Some(Stopped::LostChannel) => {
            return Err("Its creator can no longer see the channel it listens to".into());
        }
        Some(why) => {
            stand_down(state, Some(workflow.workspace_id), workflow.created_by, why).await;
            return Err(match why {
                Stopped::Inactive => "The person who built it is suspended".into(),
                Stopped::LeftWorkspace | Stopped::LostChannel => {
                    "The person who built it is no longer in this workspace".into()
                }
                Stopped::Demoted => "The person who built it is no longer allowed to".into(),
            });
        }
    }
    let creator = state
        .workspace_service
        .repo
        .get_member(workflow.workspace_id, workflow.created_by)
        .await
        .map_err(internal)?
        .ok_or_else(|| "The person who built it is no longer in this workspace".to_string())?;
    let done = state
        .workflow_repo
        .finished_steps(run.id)
        .await
        .map_err(internal)?;
    let flat = definition.flat();
    let mut context = Context {
        workflow,
        run,
        definition: &definition,
        fired: fired(state, workflow, run).await.map_err(internal)?,
        base: format!("{}/app/{}", state.config.public_url, workflow.workspace_id),
        outputs: HashMap::new(),
        raw: HashMap::new(),
        channels: HashMap::new(),
    };

    let mut taken = HashMap::new();
    let mut finished = HashSet::new();
    for row in &done {
        let position = row.position as usize;
        finished.insert(position);
        let kept = context
            .recall(state, flat.get(position).map(|node| node.step), row)
            .await;
        context.keep(position, &kept, &mut taken);
    }

    for (index, node) in flat.iter().enumerate() {
        if finished.contains(&index) || !node.reached(|branch| taken.get(&branch).copied()) {
            continue;
        }
        match run_step(state, &context, &creator, index, node.step).await {
            Ok(Outcome::Done(kept)) => context.keep(index, &kept, &mut taken),
            Ok(Outcome::Waiting) => return Ok(Outcome::Waiting),
            Err(reason) => {
                if let Err(e) = state
                    .workflow_repo
                    .record_step(run.id, index as i32, Err(&reason))
                    .await
                {
                    warn!(run_id = %run.id, "could not record a failed step: {}", e);
                }
                return Err(format!(
                    "Step {} ({}): {reason}",
                    index + 1,
                    node.step.label()
                ));
            }
        }
    }
    Ok(Outcome::Done(Kept::Nothing))
}

fn recipient_of(to: &Recipient, context: &Context<'_>) -> Result<Uuid, String> {
    match (to, context.fired.user) {
        (Recipient::Person { user_id }, _) => Ok(*user_id),
        (Recipient::TriggerUser, Some(user_id)) => Ok(user_id),
        (Recipient::TriggerUser, None) => Err("This trigger has no person".into()),
    }
}

async fn record(
    state: &AppState,
    context: &Context<'_>,
    position: usize,
    output: Output,
) -> Result<(), String> {
    state
        .workflow_repo
        .record_step(context.run.id, position as i32, Ok(output))
        .await
        .map_err(internal)
}

async fn run_step(
    state: &AppState,
    context: &Context<'_>,
    creator: &WorkspaceMember,
    position: usize,
    step: &Step,
) -> Result<Outcome, String> {
    let owner = creator.user_id;
    let workspace_id = context.workflow.workspace_id;
    match step {
        Step::SendMessage { channel_id, .. } => {
            let access = authz::require_channel_post(state, *channel_id, owner)
                .await
                .map_err(|_| "Its creator can no longer post in that channel".to_string())?;
            if access.channel.is_archived {
                return Err("That channel is archived".into());
            }
            post(state, context, creator, position, *channel_id, None, step).await?;
            Ok(Outcome::Done(Kept::Nothing))
        }
        Step::SendDm { to, .. } => {
            let recipient = recipient_of(to, context)?;
            if recipient == owner && *to == Recipient::TriggerUser {
                record(state, context, position, Output::default()).await?;
                return Ok(Outcome::Done(Kept::Nothing));
            }
            let channel_id = dm_channel(state, creator, recipient).await?;
            post(state, context, creator, position, channel_id, None, step).await?;
            Ok(Outcome::Done(Kept::Nothing))
        }
        Step::ReplyInThread { .. } => {
            let message = context.trigger_message(state).await?;
            let access = authz::require_channel_post(state, message.channel_id, owner)
                .await
                .map_err(|_| "Its creator can no longer post in that channel".to_string())?;
            if access.channel.is_archived {
                return Err("That channel is archived".into());
            }
            let root = crate::messaging::routes::thread_root(state, message.channel_id, message.id)
                .await
                .map_err(|_| "The message it replies to was deleted".to_string())?;
            let message_id = post(
                state,
                context,
                creator,
                position,
                message.channel_id,
                Some(root.id),
                step,
            )
            .await?;
            Ok(Outcome::Done(Kept::Reply {
                channel_id: message.channel_id,
                message_id,
            }))
        }
        Step::AddListItem { list_id, cells } => {
            write_item(state, context, owner, position, Item::New(*list_id), cells).await?;
            Ok(Outcome::Done(Kept::Nothing))
        }
        Step::UpdateListItem { from_step, cells } => {
            write_item(
                state,
                context,
                owner,
                position,
                Item::From(*from_step),
                cells,
            )
            .await?;
            Ok(Outcome::Done(Kept::Nothing))
        }
        Step::Form { to, title, fields } => {
            let recipient = recipient_of(to, context)?;
            let channel_id = if recipient == owner {
                let channel_id = context
                    .definition
                    .trigger
                    .channel()
                    .ok_or_else(|| "A workflow can't send its creator a form".to_string())?;
                authz::require_channel_post(state, channel_id, owner)
                    .await
                    .map_err(|_| "Its creator can no longer post in that channel".to_string())?;
                channel_id
            } else {
                dm_channel(state, creator, recipient).await?
            };
            post_form(
                state,
                context,
                owner,
                position as i32,
                channel_id,
                recipient,
                title.trim(),
                fields,
            )
            .await
            .map_err(internal)?;
            Ok(Outcome::Waiting)
        }
        Step::Branch { value, test, .. } => {
            let subject = subject(state, context, owner, position, value).await?;
            let matched = test.holds(&subject);
            record(
                state,
                context,
                position,
                Output {
                    matched: Some(matched),
                    ..Output::default()
                },
            )
            .await?;
            Ok(Outcome::Done(Kept::Matched(matched)))
        }
        Step::Delay { wait } => {
            let wake = state
                .workflow_repo
                .wake_time(wait)
                .await
                .map_err(internal)?;
            if wake.too_far {
                return Err(format!("It would wait more than {MAX_DELAY_DAYS} days"));
            }
            if wake.due {
                record(state, context, position, Output::default()).await?;
                return Ok(Outcome::Done(Kept::Nothing));
            }
            state
                .workflow_repo
                .sleep(context.run.id, position as i32, wake.at)
                .await
                .map_err(internal)?;
            Ok(Outcome::Waiting)
        }
        Step::AddReaction { emoji } => {
            let message = context.trigger_message(state).await?;
            let access =
                authz::require_message_access(state, message.channel_id, message.id, owner)
                    .await
                    .map_err(|_| "Its creator can no longer see that message".to_string())?;
            if access.channel.is_archived {
                return Err("That channel is archived".into());
            }
            if let Some(name) = shared_common::validation::custom_emoji_name(emoji) {
                state
                    .emoji_repo
                    .find_by_name(workspace_id, name)
                    .await
                    .map_err(internal)?
                    .ok_or_else(|| format!(":{name}: is not an emoji in this workspace"))?;
            }
            let mut tx = state.pool.begin().await.map_err(internal)?;
            match state
                .message_repo
                .add_reaction_in(&mut tx, message.id, owner, emoji)
                .await
            {
                Ok(reaction) => {
                    state
                        .workflow_repo
                        .record_step_in(
                            &mut tx,
                            context.run.id,
                            position as i32,
                            Ok(Output::default()),
                        )
                        .await
                        .map_err(internal)?;
                    let staged = state
                        .publisher
                        .stage_reaction_added(&mut tx, &reaction, &message, workspace_id)
                        .await
                        .map_err(internal)?;
                    tx.commit().await.map_err(internal)?;
                    state.publisher.dispatch(staged).await;
                }
                Err(e) if shared_common::errors::is_unique_violation(&e) => {
                    drop(tx);
                    record(state, context, position, Output::default()).await?;
                }
                Err(e) => return Err(e.to_string()),
            }
            Ok(Outcome::Done(Kept::Nothing))
        }
        Step::CreateChannel { name, private } => {
            let name = channel_name(&context.rendered(name));
            shared_common::validation::validate_channel_name(&name).map_err(reason)?;
            let kind = if *private {
                ChannelType::Private
            } else {
                ChannelType::Public
            };
            authz::require_channel_creator(state, workspace_id, owner, &kind)
                .await
                .map_err(reason)?;
            let mut tx = state.pool.begin().await.map_err(internal)?;
            let channel = WorkspaceRepo::create_channel_tx(
                &mut tx,
                workspace_id,
                &name,
                &kind,
                None,
                owner,
                false,
            )
            .await
            .map_err(|e| {
                if shared_common::errors::is_unique_violation(&e) {
                    format!("A channel named #{name} already exists")
                } else {
                    e.to_string()
                }
            })?;
            WorkspaceRepo::add_channel_member_tx(&mut tx, channel.id, owner, &ChannelRole::Admin)
                .await
                .map_err(internal)?;
            state
                .workflow_repo
                .record_step_in(
                    &mut tx,
                    context.run.id,
                    position as i32,
                    Ok(Output::channel(channel.id)),
                )
                .await
                .map_err(internal)?;
            tx.commit().await.map_err(internal)?;
            crate::workspace::routes::announce_created(state, &channel, owner, &ClientIp(None))
                .await
                .map_err(reason)?;
            Ok(Outcome::Done(Kept::Channel(channel)))
        }
        Step::AddToChannel { channel, people } => {
            let channel = context.channel(state, channel).await?;
            let mut invited = Vec::new();
            for invitee in people {
                match invitee {
                    Invitee::TriggerUser => invited.extend(context.fired.user),
                    Invitee::Answer { field } => invited.extend(super::cells::person_id(
                        &context.value(&format!("form.{field}")),
                    )),
                    Invitee::Group { group_id } => {
                        let group = state
                            .group_repo
                            .find(*group_id)
                            .await
                            .map_err(internal)?
                            .filter(|group| group.workspace_id == workspace_id)
                            .ok_or_else(|| "The group it adds was deleted".to_string())?;
                        invited.extend(
                            state
                                .group_repo
                                .list_member_ids(group.id)
                                .await
                                .map_err(internal)?,
                        );
                    }
                }
            }
            let mut seen = HashSet::new();
            invited.retain(|user_id| seen.insert(*user_id));
            for user_id in invited {
                membership::invite(state, &channel, owner, user_id, &ClientIp(None))
                    .await
                    .map_err(reason)?;
            }
            record(state, context, position, Output::channel(channel.id)).await?;
            Ok(Outcome::Done(Kept::Nothing))
        }
        Step::SetTopic { channel, topic } => {
            let channel = context.channel(state, channel).await?;
            authz::require_channel_edit(state, &channel, owner, authz::ChannelEdit::About)
                .await
                .map_err(|_| "Its creator can no longer change that channel's topic".to_string())?;
            let topic = context.rendered(topic).trim().to_string();
            shared_common::validation::validate_channel_topic(&topic).map_err(reason)?;
            details::update(
                state,
                &channel,
                owner,
                &ClientIp(None),
                Details {
                    topic: Some(&topic),
                    ..Details::default()
                },
            )
            .await
            .map_err(reason)?;
            record(state, context, position, Output::channel(channel.id)).await?;
            Ok(Outcome::Done(Kept::Nothing))
        }
        Step::AppStep {
            app_id,
            callback_id,
            title,
            inputs,
            outputs,
            values,
        } => {
            let (app, offered) = state
                .app_workflow_step_repo
                .provider(*app_id, callback_id)
                .await
                .map_err(internal)?
                .filter(|(app, _)| app.workspace_id == workspace_id)
                .ok_or_else(|| "The app that runs this step was removed".to_string())?;
            if let Some(problem) = app.problem() {
                return Err(problem);
            }
            if offered.is_none() {
                return Err(format!("{} no longer offers this step", app.name));
            }
            let mut sent = serde_json::Map::new();
            for input in inputs {
                let raw = values.get(&input.name).map_or("", String::as_str);
                sent.insert(
                    input.name.clone(),
                    input_value(state, context, input, raw).await?,
                );
            }
            call_app(
                state,
                context,
                position,
                &app,
                callback_id,
                title,
                &Value::Object(sent),
                outputs,
            )
            .await
            .map_err(internal)?;
            Ok(Outcome::Waiting)
        }
        Step::ArchiveChannel { channel } => {
            let channel = context.channel(state, channel).await?;
            authz::require_not_conversation(&channel).map_err(reason)?;
            if !authz::can_moderate_channel(state, &channel, owner).await {
                return Err("Its creator can no longer archive that channel".into());
            }
            crate::workspace::routes::set_archived(state, &channel, owner, &ClientIp(None), true)
                .await
                .map_err(reason)?;
            record(state, context, position, Output::channel(channel.id)).await?;
            Ok(Outcome::Done(Kept::Nothing))
        }
    }
}

async fn input_value(
    state: &AppState,
    context: &Context<'_>,
    input: &StepParam,
    raw: &str,
) -> Result<Value, String> {
    let label = input.title.trim();
    let raw = raw.trim();
    let text = match (input.kind, lone_placeholder(raw)) {
        (StepParamKind::Text, _) => context.rendered(raw),
        (_, Some(name)) => context.typed(name),
        (_, None) => raw.to_string(),
    };
    let text = text.trim();
    if text.is_empty() {
        if input.required {
            return Err(format!("\u{201c}{label}\u{201d} came out empty"));
        }
        return Ok(Value::Null);
    }
    let wrong = || format!("\u{201c}{label}\u{201d} isn't {}", input.kind.noun());
    let workspace_id = context.workflow.workspace_id;
    Ok(match input.kind {
        StepParamKind::Text => Value::String(text.to_string()),
        StepParamKind::Number => {
            let cleaned = text.replace(',', "");
            match cleaned.parse::<i64>() {
                Ok(whole) => json!(whole),
                Err(_) => cleaned
                    .parse::<f64>()
                    .ok()
                    .and_then(serde_json::Number::from_f64)
                    .map(Value::Number)
                    .ok_or_else(wrong)?,
            }
        }
        StepParamKind::Boolean => match text.to_lowercase().as_str() {
            "true" => Value::Bool(true),
            "false" => Value::Bool(false),
            _ => return Err(wrong()),
        },
        StepParamKind::Date => NaiveDate::parse_from_str(text, "%Y-%m-%d")
            .map(|date| Value::String(date.format("%Y-%m-%d").to_string()))
            .map_err(|_| wrong())?,
        StepParamKind::Person => {
            let id = super::cells::person_id(text).ok_or_else(wrong)?;
            state
                .workspace_service
                .repo
                .get_member(workspace_id, id)
                .await
                .map_err(internal)?
                .ok_or_else(|| {
                    format!("The person for \u{201c}{label}\u{201d} left the workspace")
                })?;
            Value::String(id.to_string())
        }
        StepParamKind::Channel => {
            let id = text.parse::<Uuid>().map_err(|_| wrong())?;
            state
                .workspace_service
                .repo
                .find_channel_by_id(id)
                .await
                .map_err(internal)?
                .filter(|channel| channel.workspace_id == workspace_id)
                .ok_or_else(|| format!("The channel for \u{201c}{label}\u{201d} was deleted"))?;
            Value::String(id.to_string())
        }
    })
}

#[allow(clippy::too_many_arguments)]
async fn call_app(
    state: &AppState,
    context: &Context<'_>,
    position: usize,
    app: &ProviderRow,
    callback_id: &str,
    title: &str,
    inputs: &Value,
    outputs: &[StepParam],
) -> shared_common::errors::AppResult<()> {
    let declared = serde_json::to_value(outputs).map_err(|e| AppError::Internal(e.to_string()))?;
    let mut tx = state.pool.begin().await?;
    let call = state
        .workflow_repo
        .call_app_in(
            &mut tx,
            NewCall {
                run_id: context.run.id,
                position: position as i32,
                app_id: app.id,
                title,
                inputs,
                outputs: &declared,
                timeout_secs: STEP_TIMEOUT_SECS,
            },
        )
        .await?;
    let source = Event {
        id: call.id,
        ..Event::new("workflow.app_step", json!({}))
    };
    let body = json!({
        "execution_id": call.id,
        "run_id": context.run.id,
        "workflow": { "id": context.workflow.id, "name": context.workflow.name },
        "step": { "callback_id": callback_id, "title": title, "position": position + 1 },
        "inputs": inputs,
        "expires_at": call.expires_at,
    });
    let envelope = app_events::envelope(
        &source,
        context.workflow.workspace_id,
        AppEvent::FunctionExecuted,
        &body,
    );
    state
        .app_event_repo
        .enqueue_in(
            &mut tx,
            NewDelivery {
                app_id: app.id,
                event_id: app_events::event_id(source.id, AppEvent::FunctionExecuted),
                channel_id: context.run.id,
                kind: app_events::KIND_EVENT,
                body: &envelope,
                ttl_secs: STEP_TIMEOUT_SECS,
            },
        )
        .await?;
    tx.commit().await?;
    Ok(())
}

async fn subject(
    state: &AppState,
    context: &Context<'_>,
    owner: Uuid,
    position: usize,
    value: &str,
) -> Result<String, String> {
    let workspace_id = context.workflow.workspace_id;
    match source(value) {
        Some(Source::Profile(field_id)) => {
            let Some(user_id) = context.fired.user else {
                return Ok(String::new());
            };
            let field = state
                .profile_field_repo
                .find(workspace_id, field_id)
                .await
                .map_err(internal)?
                .ok_or_else(|| "The profile field it tests was deleted".to_string())?;
            let raw = state
                .profile_field_repo
                .values_of(workspace_id, user_id)
                .await
                .map_err(internal)?
                .remove(&field_id)
                .unwrap_or_default();
            Ok(match field.kind {
                ProfileFieldKind::Options => raw
                    .parse::<Uuid>()
                    .ok()
                    .and_then(|id| field.options.iter().find(|option| option.id == id))
                    .map(|option| option.label.clone())
                    .unwrap_or_default(),
                _ => raw,
            })
        }
        Some(Source::Column { step, column_id }) => {
            let gone = || "The list item it tests was deleted".to_string();
            let item_id = state
                .workflow_repo
                .item_of(context.run.id, step as i32)
                .await
                .map_err(internal)?
                .ok_or_else(gone)?;
            let list_id = context
                .definition
                .list_of(step)
                .ok_or_else(|| "The step that added the item is gone".to_string())?;
            crate::lists::service::readable(state, owner, list_id)
                .await
                .map_err(|_| "Its creator can no longer see that list".to_string())?;
            let columns = state.list_repo.columns(list_id).await.map_err(internal)?;
            let column = columns
                .iter()
                .find(|column| column.id == column_id)
                .ok_or_else(|| "The column it tests was deleted".to_string())?;
            let value = state
                .workflow_repo
                .item_value(item_id, column_id)
                .await
                .map_err(internal)?
                .ok_or_else(gone)?;
            Ok(super::cells::text(column, value.as_ref()))
        }
        Some(Source::Variable(name)) => {
            let text = context.value(name);
            Ok(match context.definition.variable_kind(position, name) {
                VariableKind::Person => super::cells::person_id(&text)
                    .map(|id| id.to_string())
                    .unwrap_or_default(),
                _ => text,
            })
        }
        None => Ok(String::new()),
    }
}

async fn post(
    state: &AppState,
    context: &Context<'_>,
    creator: &WorkspaceMember,
    position: usize,
    channel_id: Uuid,
    thread: Option<Uuid>,
    step: &Step,
) -> Result<Uuid, String> {
    let text = step.text().unwrap_or_default();
    let mut metadata = context.metadata();
    let (content, authored) = if step.summary() {
        let answers = context.answers(position, false);
        let content = format!("{}\n\n{}", context.rendered(text), summary(&answers));
        let authored = format!(
            "{}\n\n{}",
            context.authored(text),
            summary(&context.answers(position, true))
        );
        metadata["fields"] = serde_json::json!(answers);
        (content, authored)
    } else {
        (context.rendered(text), context.authored(text))
    };
    let mentioned =
        crate::messaging::routes::expand_mentions(state, channel_id, creator.user_id, &authored)
            .await;

    let workspace_id = context.workflow.workspace_id;
    let mut tx = state.pool.begin().await.map_err(internal)?;
    let message = state
        .message_repo
        .create_bot_reply_in(
            &mut tx,
            channel_id,
            thread,
            creator.user_id,
            &content,
            &metadata,
            &mentioned,
        )
        .await
        .map_err(internal)?;
    state
        .workflow_repo
        .record_step_in(
            &mut tx,
            context.run.id,
            position as i32,
            Ok(Output::message(message.id)),
        )
        .await
        .map_err(internal)?;
    let staged = state
        .publisher
        .stage_message_created(&mut tx, &message, workspace_id, &mentioned)
        .await
        .map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    state.publisher.dispatch(staged).await;
    if let Some(root) = thread {
        crate::threads::routes::announce_reply(state, root, workspace_id).await;
    }
    Ok(message.id)
}

enum Item {
    New(Uuid),
    From(usize),
}

async fn write_item(
    state: &AppState,
    context: &Context<'_>,
    creator: Uuid,
    position: usize,
    target: Item,
    cells: &[CellValue],
) -> Result<(), String> {
    let (list_id, item_id) = match target {
        Item::New(list_id) => (list_id, None),
        Item::From(step) => {
            let item_id = state
                .workflow_repo
                .item_of(context.run.id, step as i32)
                .await
                .map_err(internal)?
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
    let columns = state.list_repo.columns(list_id).await.map_err(internal)?;
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

    let mut tx = state.pool.begin().await.map_err(internal)?;
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
    .map_err(reason)?;
    state
        .workflow_repo
        .record_item_step_in(&mut tx, context.run.id, position as i32, write.item.id)
        .await
        .map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    write.announce(state).await;
    Ok(())
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
    Ok(Fired {
        user,
        channel: id("channel_id"),
        message: id("message_id"),
        values,
    })
}

async fn dm_channel(
    state: &AppState,
    creator: &WorkspaceMember,
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
