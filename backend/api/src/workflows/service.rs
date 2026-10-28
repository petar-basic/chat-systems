use std::collections::HashMap;

use rand::RngExt;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::*;
use super::routes::RUNS_PER_MINUTE;
use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::authz;
use crate::lists::models::ColumnKind;
use crate::messaging::publisher::Staged;
use crate::notifications::models::NotificationType;
use crate::rate_limit::{self, LimiterFailure};
use crate::state::AppState;
use crate::workspace::models::{ChannelType, WhoMay, WorkspaceMember, WorkspaceRole};

pub fn webhook_url(state: &AppState, token: &str) -> String {
    format!("{}/api/workflows/webhook/{token}", state.config.public_url)
}

fn generate_token() -> String {
    use base64::Engine;
    let mut bytes = [0u8; 24];
    rand::rng().fill(&mut bytes[..]);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

pub fn present(state: &AppState, row: WorkflowRow) -> AppResult<Workflow> {
    let definition = row
        .definition()
        .ok_or_else(|| AppError::Internal("a stored workflow does not deserialize".into()))?;
    Ok(Workflow {
        id: row.id,
        workspace_id: row.workspace_id,
        name: row.name,
        description: row.description,
        created_by: row.created_by,
        definition,
        version: row.version,
        enabled: row.enabled,
        creator_gone: row.creator_gone,
        webhook_url: row.webhook_token.as_deref().map(|t| webhook_url(state, t)),
        next_run_at: row.next_run_at,
        last_run: row
            .last_status
            .zip(row.last_started_at)
            .map(|(status, started_at)| LastRun { status, started_at }),
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

pub async fn require_builder(
    state: &AppState,
    workspace_id: Uuid,
    user_id: Uuid,
) -> AppResult<WorkspaceMember> {
    let member = authz::require_workspace_member(state, workspace_id, user_id).await?;
    if member.role == WorkspaceRole::Guest {
        return Err(AppError::Forbidden("Guests can't build workflows".into()));
    }
    let workspace = state
        .workspace_service
        .repo
        .find_workspace_by_id(workspace_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Workspace not found".into()))?;
    if !WhoMay::of(&workspace.settings, WhoMay::WORKFLOW_BUILDERS).allows(&member.role) {
        return Err(AppError::Forbidden(
            "Only admins can build workflows in this workspace".into(),
        ));
    }
    Ok(member)
}

async fn find_manageable(
    state: &AppState,
    id: Uuid,
    user_id: Uuid,
) -> AppResult<(WorkflowRow, WorkspaceMember)> {
    let not_found = || AppError::NotFound("Workflow not found".into());
    let row = state.workflow_repo.find(id).await?.ok_or_else(not_found)?;
    let member = state
        .workspace_service
        .repo
        .get_member(row.workspace_id, user_id)
        .await?
        .ok_or_else(not_found)?;
    if row.created_by != user_id && !member.role.has_at_least(&WorkspaceRole::Admin) {
        return Err(not_found());
    }
    Ok((row, member))
}

async fn find_own(state: &AppState, id: Uuid, user_id: Uuid) -> AppResult<WorkflowRow> {
    let (row, _) = find_manageable(state, id, user_id).await?;
    if row.created_by != user_id {
        return Err(AppError::Forbidden(
            "Only the person who built a workflow can change it".into(),
        ));
    }
    require_builder(state, row.workspace_id, user_id).await?;
    Ok(row)
}

pub async fn check_authority(
    state: &AppState,
    workspace_id: Uuid,
    creator: Uuid,
    definition: &Definition,
) -> AppResult<()> {
    if let Trigger::Schedule { timezone, .. } = &definition.trigger {
        if !state.workflow_repo.timezone_exists(timezone).await? {
            return Err(AppError::Validation(format!(
                "\"{timezone}\" isn't a timezone"
            )));
        }
    }
    let member = authz::require_workspace_member(state, workspace_id, creator).await?;
    if let Some(channel_id) = definition.trigger.channel() {
        let refused = || AppError::Validation("You can't see the channel it listens to".into());
        let access = authz::require_channel_access(state, channel_id, creator)
            .await
            .map_err(|_| refused())?;
        if access.channel.workspace_id != workspace_id
            || !matches!(
                access.channel.channel_type,
                ChannelType::Public | ChannelType::Private
            )
        {
            return Err(refused());
        }
        if access.channel.is_archived {
            return Err(AppError::Validation(
                "The channel it listens to is archived".into(),
            ));
        }
    }
    for (index, step) in definition.steps.iter().enumerate() {
        let at = format!("Step {}", index + 1);
        match step {
            Step::SendMessage {
                channel_id, text, ..
            } => {
                let refused =
                    || AppError::Validation(format!("{at}: you can't post in that channel"));
                let access = authz::require_channel_post(state, *channel_id, creator)
                    .await
                    .map_err(|_| refused())?;
                if access.channel.workspace_id != workspace_id
                    || !matches!(
                        access.channel.channel_type,
                        ChannelType::Public | ChannelType::Private
                    )
                {
                    return Err(refused());
                }
                if access.channel.is_archived {
                    return Err(AppError::Validation(format!(
                        "{at}: that channel is archived"
                    )));
                }
                if crate::messaging::routes::mentions_everyone(text)
                    && !authz::can_broadcast(state, &access.channel, creator).await?
                {
                    return Err(AppError::Validation(format!(
                        "{at}: you can't use @channel, @here or @everyone in that channel"
                    )));
                }
            }
            Step::SendDm {
                to: Recipient::Person { user_id },
                ..
            }
            | Step::Form {
                to: Recipient::Person { user_id },
                ..
            } => {
                if *user_id == creator {
                    let what = if matches!(step, Step::Form { .. }) {
                        "form"
                    } else {
                        "DM"
                    };
                    return Err(AppError::Validation(format!(
                        "{at}: a workflow can't send you a {what}; pick someone else"
                    )));
                }
                crate::conversations::routes::require_reachable(state, &member, *user_id)
                    .await
                    .map_err(|_| {
                        AppError::Validation(format!("{at}: you can't message that person"))
                    })?;
            }
            Step::SendDm {
                to: Recipient::TriggerUser,
                ..
            }
            | Step::Form {
                to: Recipient::TriggerUser,
                ..
            } => {}
            Step::AddListItem { list_id, cells } => {
                let refused = || AppError::Validation(format!("{at}: you can't add to that list"));
                let list = crate::lists::service::writable(state, creator, *list_id)
                    .await
                    .map_err(|_| refused())?;
                if list.workspace_id != workspace_id {
                    return Err(refused());
                }
                check_cells(
                    state,
                    workspace_id,
                    definition,
                    index,
                    *list_id,
                    cells,
                    false,
                )
                .await?;
            }
            Step::UpdateListItem { cells, .. } => {
                let list_id = definition.list_of(index).ok_or_else(|| {
                    AppError::Validation(format!(
                        "{at}: pick an earlier step that adds a list item"
                    ))
                })?;
                check_cells(state, workspace_id, definition, index, list_id, cells, true).await?;
            }
        }
    }
    Ok(())
}

async fn check_cells(
    state: &AppState,
    workspace_id: Uuid,
    definition: &Definition,
    index: usize,
    list_id: Uuid,
    cells: &[CellValue],
    updating: bool,
) -> AppResult<()> {
    let at = format!("Step {}", index + 1);
    let invalid = |message: String| AppError::Validation(format!("{at}: {message}"));
    let columns = state.list_repo.columns(list_id).await?;
    for cell in cells {
        let column = columns
            .iter()
            .find(|column| column.id == cell.column_id)
            .ok_or_else(|| invalid("that list has no such column".into()))?;
        if updating
            && !matches!(
                column.kind,
                ColumnKind::Person | ColumnKind::Select | ColumnKind::Checkbox
            )
        {
            return Err(invalid(format!(
                "\u{201c}{}\u{201d} isn't a person, choice or checkbox column",
                column.name
            )));
        }
        super::cells::check(column, &cell.value, |name| {
            definition.variable_kind(index, name)
        })
        .map_err(invalid)?;
        if column.kind == ColumnKind::Person && placeholders(&cell.value).is_empty() {
            let person = cell.value.trim().parse::<Uuid>().ok();
            let member = match person {
                Some(person) => state
                    .workspace_service
                    .repo
                    .get_member(workspace_id, person)
                    .await?
                    .is_some(),
                None => false,
            };
            if !member {
                return Err(invalid(format!(
                    "\u{201c}{}\u{201d} must be someone in this workspace",
                    column.name
                )));
            }
        }
    }
    Ok(())
}

pub async fn list(state: &AppState, workspace_id: Uuid, user_id: Uuid) -> AppResult<Vec<Workflow>> {
    let member = authz::require_workspace_member(state, workspace_id, user_id).await?;
    let only_by = (!member.role.has_at_least(&WorkspaceRole::Admin)).then_some(user_id);
    state
        .workflow_repo
        .list(workspace_id, only_by)
        .await?
        .into_iter()
        .map(|row| present(state, row))
        .collect()
}

pub async fn get(state: &AppState, id: Uuid, user_id: Uuid) -> AppResult<Workflow> {
    let (row, _) = find_manageable(state, id, user_id).await?;
    present(state, row)
}

pub async fn take_over(
    state: &AppState,
    id: Uuid,
    user_id: Uuid,
    ip: &ClientIp,
) -> AppResult<Workflow> {
    let row = state
        .workflow_repo
        .find(id)
        .await?
        .ok_or_else(|| AppError::NotFound("Workflow not found".into()))?;
    authz::require_workspace_role(state, row.workspace_id, user_id, &WorkspaceRole::Admin).await?;
    if !row.creator_gone {
        return Err(AppError::Conflict(
            "Only a workflow whose creator left or was suspended can be taken over".into(),
        ));
    }
    state
        .workflow_repo
        .set_owner(id, user_id, &generate_token())
        .await?;
    audit::record(
        state,
        AuditEntry::new(AuditAction::WorkflowTakenOver, user_id)
            .workspace(row.workspace_id)
            .resource(id)
            .ip(ip)
            .details(serde_json::json!({ "name": row.name, "from": row.created_by })),
    )
    .await;
    reload(state, id).await
}

pub async fn rotate_link(
    state: &AppState,
    id: Uuid,
    user_id: Uuid,
    ip: &ClientIp,
) -> AppResult<Workflow> {
    let (row, _) = find_manageable(state, id, user_id).await?;
    if !state
        .workflow_repo
        .rotate_token(id, &generate_token())
        .await?
    {
        return Err(AppError::Conflict(
            "Only a workflow started by a webhook has a link to rotate".into(),
        ));
    }
    audit::record(
        state,
        audit_entry(
            AuditAction::WorkflowLinkRotated,
            user_id,
            row.workspace_id,
            id,
            &row.name,
        )
        .ip(ip),
    )
    .await;
    reload(state, id).await
}

async fn reload(state: &AppState, id: Uuid) -> AppResult<Workflow> {
    let row = state
        .workflow_repo
        .find(id)
        .await?
        .ok_or_else(|| AppError::NotFound("Workflow not found".into()))?;
    present(state, row)
}

fn audit_entry(action: AuditAction, actor: Uuid, row_ws: Uuid, id: Uuid, name: &str) -> AuditEntry {
    AuditEntry::new(action, actor)
        .workspace(row_ws)
        .resource(id)
        .details(serde_json::json!({ "name": name }))
}

pub async fn create(
    state: &AppState,
    workspace_id: Uuid,
    user_id: Uuid,
    ip: &ClientIp,
    req: &SaveWorkflowRequest,
) -> AppResult<Workflow> {
    require_builder(state, workspace_id, user_id).await?;
    let checked = req.checked()?;
    check_authority(state, workspace_id, user_id, &req.definition).await?;
    let token = matches!(req.definition.trigger, Trigger::Webhook { .. }).then(generate_token);
    let id = state
        .workflow_repo
        .insert(
            workspace_id,
            user_id,
            &checked,
            token.as_deref(),
            req.definition.trigger.schedule().as_ref(),
        )
        .await?;
    audit::record(
        state,
        audit_entry(
            AuditAction::WorkflowCreated,
            user_id,
            workspace_id,
            id,
            &checked.name,
        )
        .ip(ip),
    )
    .await;
    reload(state, id).await
}

pub async fn update(
    state: &AppState,
    id: Uuid,
    user_id: Uuid,
    ip: &ClientIp,
    req: &SaveWorkflowRequest,
) -> AppResult<Workflow> {
    let row = find_own(state, id, user_id).await?;
    let checked = req.checked()?;
    check_authority(state, row.workspace_id, user_id, &req.definition).await?;
    let token = match req.definition.trigger {
        Trigger::Webhook { .. } => Some(row.webhook_token.clone().unwrap_or_else(generate_token)),
        _ => None,
    };
    state
        .workflow_repo
        .update(id, &checked, token.as_deref())
        .await?;
    state
        .workflow_repo
        .set_enabled(id, row.enabled, req.definition.trigger.schedule().as_ref())
        .await?;
    audit::record(
        state,
        audit_entry(
            AuditAction::WorkflowUpdated,
            user_id,
            row.workspace_id,
            id,
            &checked.name,
        )
        .ip(ip),
    )
    .await;
    reload(state, id).await
}

pub async fn enable(
    state: &AppState,
    id: Uuid,
    user_id: Uuid,
    ip: &ClientIp,
) -> AppResult<Workflow> {
    let row = find_own(state, id, user_id).await?;
    let definition = row
        .definition()
        .ok_or_else(|| AppError::Internal("a stored workflow does not deserialize".into()))?;
    check_authority(state, row.workspace_id, user_id, &definition).await?;
    state
        .workflow_repo
        .set_enabled(id, true, definition.trigger.schedule().as_ref())
        .await?;
    audit::record(
        state,
        audit_entry(
            AuditAction::WorkflowEnabled,
            user_id,
            row.workspace_id,
            id,
            &row.name,
        )
        .ip(ip),
    )
    .await;
    reload(state, id).await
}

pub async fn disable(
    state: &AppState,
    id: Uuid,
    user_id: Uuid,
    ip: &ClientIp,
) -> AppResult<Workflow> {
    let (row, _) = find_manageable(state, id, user_id).await?;
    state.workflow_repo.set_enabled(id, false, None).await?;
    audit::record(
        state,
        audit_entry(
            AuditAction::WorkflowDisabled,
            user_id,
            row.workspace_id,
            id,
            &row.name,
        )
        .ip(ip),
    )
    .await;
    reload(state, id).await
}

pub async fn delete(state: &AppState, id: Uuid, user_id: Uuid, ip: &ClientIp) -> AppResult<()> {
    let (row, _) = find_manageable(state, id, user_id).await?;
    state.workflow_repo.delete(id).await?;
    audit::record(
        state,
        audit_entry(
            AuditAction::WorkflowDeleted,
            user_id,
            row.workspace_id,
            id,
            &row.name,
        )
        .ip(ip),
    )
    .await;
    Ok(())
}

pub async fn runs(state: &AppState, id: Uuid, user_id: Uuid) -> AppResult<Vec<WorkflowRun>> {
    find_manageable(state, id, user_id).await?;
    let runs = state.workflow_repo.runs(id, HISTORY_LIMIT).await?;
    let ids: Vec<Uuid> = runs.iter().map(|run| run.id).collect();
    let mut steps: HashMap<Uuid, Vec<RunStep>> = HashMap::new();
    for (run_id, step) in state.workflow_repo.steps_of(&ids).await? {
        steps.entry(run_id).or_default().push(step);
    }
    Ok(runs
        .into_iter()
        .map(|run| WorkflowRun {
            steps: steps.remove(&run.id).unwrap_or_default(),
            id: run.id,
            workflow_version: run.workflow_version,
            status: run.status,
            error: run.error,
            started_at: run.started_at,
            finished_at: run.finished_at,
        })
        .collect())
}

pub async fn shortcuts(
    state: &AppState,
    channel_id: Uuid,
    user_id: Uuid,
) -> AppResult<Vec<Shortcut>> {
    authz::require_channel_post(state, channel_id, user_id).await?;
    in_channel(state, channel_id).await
}

pub async fn in_channel(state: &AppState, channel_id: Uuid) -> AppResult<Vec<Shortcut>> {
    Ok(state
        .workflow_repo
        .shortcuts(channel_id)
        .await?
        .into_iter()
        .map(|row| Shortcut {
            id: row.id,
            name: row.name,
            description: row.description,
        })
        .collect())
}

pub async fn start_shortcut(
    state: &AppState,
    id: Uuid,
    user_id: Uuid,
    request_id: Option<&str>,
) -> AppResult<Uuid> {
    let not_found = || AppError::NotFound("Workflow not found".into());
    let workflow = state.workflow_repo.find(id).await?.ok_or_else(not_found)?;
    let Some(Trigger::Shortcut { channel_id }) = workflow.definition().map(|d| d.trigger) else {
        return Err(not_found());
    };
    authz::require_channel_post(state, channel_id, user_id)
        .await
        .map_err(|_| not_found())?;
    if !workflow.enabled {
        return Err(AppError::Conflict("This workflow is turned off".into()));
    }

    let mut conn = state.redis.clone();
    rate_limit::enforce(
        &mut conn,
        &format!("rate_limit:workflow:{}", workflow.id),
        RUNS_PER_MINUTE,
        60,
        LimiterFailure::Open,
    )
    .await?;

    let key = match request_id {
        Some(request_id) => format!("shortcut:{user_id}:{request_id}"),
        None => format!("shortcut:{}", Uuid::new_v4()),
    };
    let context = serde_json::json!({
        "trigger": { "user_id": user_id, "channel_id": channel_id, "message_id": null }
    });
    Ok(state
        .workflow_repo
        .queue_run(workflow.id, workflow.version, &key, &context)
        .await?)
}

pub async fn submit_form(
    state: &AppState,
    run_id: Uuid,
    position: i32,
    user_id: Uuid,
    answers: &HashMap<String, String>,
) -> AppResult<()> {
    let mut tx = state.pool.begin().await?;
    match answer_form(state, &mut tx, run_id, position, user_id, answers).await {
        Ok(staged) => {
            tx.commit().await?;
            if let Some(staged) = staged {
                state.publisher.dispatch(staged).await;
            }
            Ok(())
        }
        Err(e) => {
            tx.rollback().await?;
            Err(e)
        }
    }
}

async fn answer_form(
    state: &AppState,
    tx: &mut sqlx::PgConnection,
    run_id: Uuid,
    position: i32,
    user_id: Uuid,
    answers: &HashMap<String, String>,
) -> AppResult<Option<Staged>> {
    let form = state
        .workflow_repo
        .form_for_update_in(tx, run_id, position)
        .await?
        .ok_or_else(|| AppError::NotFound("Form not found".into()))?;
    if form.recipient_id != user_id {
        return Err(AppError::Forbidden(
            "This form was sent to someone else".into(),
        ));
    }
    if form.answered_at.is_some() {
        return Err(AppError::Conflict("This form was already filled in".into()));
    }
    if form.run_status == RunStatus::Expired || form.expires_at <= chrono::Utc::now() {
        return Err(AppError::Conflict("This form has expired".into()));
    }
    if form.run_status != RunStatus::Waiting {
        return Err(AppError::Conflict(
            "This workflow is no longer waiting for the form".into(),
        ));
    }
    let fields: Vec<FormField> =
        serde_json::from_value(form.fields).map_err(|e| AppError::Internal(e.to_string()))?;
    let answers = checked_answers(state, form.workspace_id, &fields, answers).await?;

    state
        .workflow_repo
        .answer_in(tx, run_id, position, &answers)
        .await?;
    state
        .workflow_repo
        .record_step_in(tx, run_id, position, Ok(Some(form.message_id)))
        .await?;
    let message = state
        .message_repo
        .mark_form_in(
            tx,
            form.message_id,
            "submitted_at",
            &serde_json::json!(chrono::Utc::now()),
        )
        .await?;
    let staged = match message {
        Some(message) => Some(
            state
                .publisher
                .stage_message_updated(tx, &message, form.workspace_id)
                .await?,
        ),
        None => None,
    };
    Ok(staged)
}

async fn checked_answers(
    state: &AppState,
    workspace_id: Uuid,
    fields: &[FormField],
    answers: &HashMap<String, String>,
) -> AppResult<serde_json::Value> {
    if let Some(name) = answers
        .keys()
        .find(|name| !fields.iter().any(|field| &field.name == *name))
    {
        return Err(AppError::Validation(format!(
            "\"{name}\" is not a question in this form"
        )));
    }
    let mut checked = serde_json::Map::new();
    for field in fields {
        let label = field.label.trim();
        let answer = answers
            .get(&field.name)
            .map(|a| a.trim())
            .unwrap_or_default();
        if answer.is_empty() {
            if field.required {
                return Err(AppError::Validation(format!(
                    "\u{201c}{label}\u{201d} needs an answer"
                )));
            }
            checked.insert(field.name.clone(), serde_json::Value::String(String::new()));
            continue;
        }
        let value = match field.kind {
            FieldKind::ShortText | FieldKind::LongText => {
                let (limit, single_line) = match field.kind {
                    FieldKind::ShortText => (SHORT_ANSWER_MAX_CHARS, true),
                    _ => (LONG_ANSWER_MAX_CHARS, false),
                };
                if answer.chars().count() > limit {
                    return Err(AppError::Validation(format!(
                        "\u{201c}{label}\u{201d} can be at most {limit} characters"
                    )));
                }
                if single_line && answer.contains('\n') {
                    return Err(AppError::Validation(format!(
                        "\u{201c}{label}\u{201d} takes one line"
                    )));
                }
                answer.to_string()
            }
            FieldKind::Select => {
                if !field.options.iter().any(|option| option.trim() == answer) {
                    return Err(AppError::Validation(format!(
                        "\u{201c}{label}\u{201d} has no choice \u{201c}{answer}\u{201d}"
                    )));
                }
                answer.to_string()
            }
            FieldKind::Person => {
                let not_a_member = || {
                    AppError::Validation(format!(
                        "\u{201c}{label}\u{201d} must be someone in this workspace"
                    ))
                };
                let person: Uuid = answer.parse().map_err(|_| not_a_member())?;
                state
                    .workspace_service
                    .repo
                    .get_member(workspace_id, person)
                    .await?
                    .ok_or_else(not_a_member)?;
                super::runner::mention_of(state, workspace_id, person)
                    .await?
                    .ok_or_else(not_a_member)?
            }
            FieldKind::Date => chrono::NaiveDate::parse_from_str(answer, "%Y-%m-%d")
                .map_err(|_| AppError::Validation(format!("\u{201c}{label}\u{201d} needs a date")))?
                .format("%Y-%m-%d")
                .to_string(),
        };
        checked.insert(field.name.clone(), serde_json::Value::String(value));
    }
    Ok(serde_json::Value::Object(checked))
}

pub async fn stop_creators_workflows(
    state: &AppState,
    workspace_id: Option<Uuid>,
    user_id: Uuid,
    because: &str,
) -> Vec<super::repo::StoppedWorkflow> {
    let stopped = match state
        .workflow_repo
        .disable_by_creator(workspace_id, user_id)
        .await
    {
        Ok(stopped) => stopped,
        Err(e) => {
            tracing::warn!(
                "could not turn off the workflows of a stopped creator: {}",
                e
            );
            return Vec::new();
        }
    };
    let mut admins_of: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    for workflow in &stopped {
        if let std::collections::hash_map::Entry::Vacant(slot) =
            admins_of.entry(workflow.workspace_id)
        {
            let admins = state
                .workflow_repo
                .admin_ids(workflow.workspace_id)
                .await
                .unwrap_or_else(|e| {
                    tracing::warn!(
                        "could not find the admins to tell about stopped workflows: {}",
                        e
                    );
                    Vec::new()
                });
            slot.insert(admins);
        }
        let body = format!(
            "\u{201c}{}\u{201d} stopped because {because}.",
            workflow.name
        );
        for admin in &admins_of[&workflow.workspace_id] {
            tell(state, *admin, workflow.workspace_id, workflow.id, &body).await;
        }
    }
    stopped
}

async fn tell(state: &AppState, user_id: Uuid, workspace_id: Uuid, workflow_id: Uuid, body: &str) {
    const TITLE: &str = "A workflow was turned off";
    let data = serde_json::json!({ "workflow_id": workflow_id });
    if let Err(e) = state
        .notification_repo
        .create(
            user_id,
            workspace_id,
            &NotificationType::System,
            TITLE,
            Some(body),
            &data,
        )
        .await
    {
        tracing::warn!("could not tell an admin a workflow stopped: {}", e);
        return;
    }
    let event = serde_json::json!({
        "user_id": user_id.to_string(),
        "workspace_id": workspace_id.to_string(),
        "title": TITLE,
        "body": body,
        "kind": "system",
    });
    let _ = state.publisher.publish("notification.push", event).await;
}
