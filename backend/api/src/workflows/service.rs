use std::collections::HashMap;

use rand::RngExt;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::*;
use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::authz;
use crate::notifications::models::NotificationType;
use crate::state::AppState;
use crate::workspace::models::{ChannelType, WorkflowBuilders, WorkspaceMember, WorkspaceRole};

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
    if WorkflowBuilders::of(&workspace.settings) == WorkflowBuilders::Admins
        && !member.role.has_at_least(&WorkspaceRole::Admin)
    {
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
            Step::SendMessage { channel_id, .. } => {
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
            }
            Step::SendDm {
                to: Recipient::Person { user_id },
                ..
            } => {
                if *user_id == creator {
                    return Err(AppError::Validation(format!(
                        "{at}: a workflow can't send you a DM; pick someone else"
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
            } => {}
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
    let draft = req.draft()?;
    check_authority(state, workspace_id, user_id, &req.definition).await?;
    let token = matches!(req.definition.trigger, Trigger::Webhook { .. }).then(generate_token);
    let id = state
        .workflow_repo
        .insert(workspace_id, user_id, &draft, token.as_deref())
        .await?;
    audit::record(
        state,
        audit_entry(
            AuditAction::WorkflowCreated,
            user_id,
            workspace_id,
            id,
            &draft.name,
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
    let draft = req.draft()?;
    check_authority(state, row.workspace_id, user_id, &req.definition).await?;
    let token = match req.definition.trigger {
        Trigger::Webhook { .. } => Some(row.webhook_token.clone().unwrap_or_else(generate_token)),
        _ => None,
    };
    state
        .workflow_repo
        .update(id, &draft, token.as_deref())
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
            &draft.name,
        )
        .ip(ip),
    )
    .await;
    reload(state, id).await
}

pub async fn publish(
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

pub async fn creator_left(state: &AppState, workspace_id: Uuid, user_id: Uuid) {
    let stopped = match state
        .workflow_repo
        .disable_by_creator(workspace_id, user_id)
        .await
    {
        Ok(stopped) => stopped,
        Err(e) => {
            tracing::warn!(
                "could not turn off the workflows of a departed member: {}",
                e
            );
            return;
        }
    };
    if stopped.is_empty() {
        return;
    }
    let admins = match state.workflow_repo.admin_ids(workspace_id).await {
        Ok(admins) => admins,
        Err(e) => {
            tracing::warn!(
                "could not find the admins to tell about stopped workflows: {}",
                e
            );
            return;
        }
    };
    for workflow in &stopped {
        let body = format!(
            "\u{201c}{}\u{201d} stopped because the person who built it left the workspace.",
            workflow.name
        );
        for admin in &admins {
            tell(state, *admin, workspace_id, workflow.id, &body).await;
        }
    }
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
