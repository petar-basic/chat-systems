use std::collections::HashSet;

use rand::seq::IndexedRandom;
use sqlx::PgConnection;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};
use shared_common::validation::validate_display_name;

use super::matcher;
use super::models::{
    CustomResponse, CustomResponses, HeardMessage, NewCustomResponse, UpdateCustomResponse,
    MAX_ANSWER, MAX_ANSWERS, MAX_RESPONSES, MAX_TRIGGER, MAX_TRIGGERS,
};
use super::repo::CustomResponseRepo;
use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::authz;
use crate::messaging::publisher::{EventPublisher, Staged};
use crate::messaging::routes::thread_root;
use crate::state::AppState;
use crate::threads::repo::ReplyFollows;
use crate::threads::routes::announce_reply;
use crate::workspace::models::{WhoMay, WorkspaceMember, WorkspaceRole};
use crate::workspace_bots::models::{WorkspaceBot, WorkspaceBotKind};
use crate::workspace_bots::repo::{BotIdentity, WorkspaceBotRepo};

const KIND: WorkspaceBotKind = WorkspaceBotKind::Responses;

async fn ensure_bot_in(
    state: &AppState,
    conn: &mut PgConnection,
    workspace_id: Uuid,
) -> AppResult<(Uuid, Option<Staged>)> {
    let identity = BotIdentity {
        name: &state.config.instance_name,
        avatar_url: state.config.instance_icon_url.as_deref(),
    };
    let bot = WorkspaceBotRepo::ensure_in(conn, workspace_id, KIND, &identity).await?;
    let joined = if bot.created {
        Some(EventPublisher::stage_workspace_member_joined(conn, workspace_id, bot.user_id).await?)
    } else {
        None
    };
    Ok((bot.user_id, joined))
}

async fn announce_bot(state: &AppState, joined: Option<Staged>) {
    if let Some(joined) = joined {
        state.publisher.dispatch(joined).await;
    }
}

async fn require_viewer(
    state: &AppState,
    workspace_id: Uuid,
    user_id: Uuid,
) -> AppResult<(WorkspaceMember, bool)> {
    let member = authz::require_workspace_member(state, workspace_id, user_id).await?;
    if member.role == WorkspaceRole::Guest {
        return Err(AppError::Forbidden(
            "Guests can't see custom responses".into(),
        ));
    }
    let workspace = state
        .workspace_service
        .repo
        .find_workspace_by_id(workspace_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Workspace not found".into()))?;
    let can_edit = WhoMay::of(&workspace.settings, WhoMay::CUSTOM_RESPONDERS).allows(&member.role);
    Ok((member, can_edit))
}

async fn require_editor(state: &AppState, workspace_id: Uuid, user_id: Uuid) -> AppResult<()> {
    let (_, can_edit) = require_viewer(state, workspace_id, user_id).await?;
    if !can_edit {
        return Err(AppError::Forbidden(
            "Only admins can edit custom responses in this workspace".into(),
        ));
    }
    Ok(())
}

pub async fn list(
    state: &AppState,
    workspace_id: Uuid,
    user_id: Uuid,
) -> AppResult<CustomResponses> {
    let (member, can_edit) = require_viewer(state, workspace_id, user_id).await?;
    let bot = match state.workspace_bot_repo.find(workspace_id, KIND).await? {
        Some(bot) => bot,
        None => WorkspaceBot {
            user_id: None,
            name: state.config.instance_name.clone(),
            avatar_url: state.config.instance_icon_url.clone(),
        },
    };
    Ok(CustomResponses {
        data: state.custom_response_repo.list(workspace_id).await?,
        can_edit,
        can_rename: member.role.has_at_least(&WorkspaceRole::Admin),
        bot,
    })
}

fn triggers_of(raw: &[String]) -> AppResult<Vec<String>> {
    let mut seen = HashSet::new();
    let mut triggers = Vec::new();
    for phrase in raw {
        let phrase = phrase.split_whitespace().collect::<Vec<_>>().join(" ");
        if phrase.is_empty() {
            continue;
        }
        if phrase.chars().count() > MAX_TRIGGER {
            return Err(AppError::Validation(format!(
                "A trigger phrase is at most {MAX_TRIGGER} characters"
            )));
        }
        if seen.insert(matcher::fold(&phrase)) {
            triggers.push(phrase);
        }
    }
    if triggers.is_empty() {
        return Err(AppError::Validation(
            "A response needs at least one trigger phrase".into(),
        ));
    }
    if triggers.len() > MAX_TRIGGERS {
        return Err(AppError::Validation(format!(
            "A response has at most {MAX_TRIGGERS} trigger phrases"
        )));
    }
    Ok(triggers)
}

fn answers_of(raw: &[String]) -> AppResult<Vec<String>> {
    let answers: Vec<String> = raw
        .iter()
        .map(|answer| answer.trim().to_string())
        .filter(|answer| !answer.is_empty())
        .collect();
    if answers.is_empty() {
        return Err(AppError::Validation(
            "A response needs at least one answer".into(),
        ));
    }
    if answers.len() > MAX_ANSWERS {
        return Err(AppError::Validation(format!(
            "A response has at most {MAX_ANSWERS} answers"
        )));
    }
    if answers.iter().any(|answer| answer.len() > MAX_ANSWER) {
        return Err(AppError::Validation(format!(
            "An answer is at most {MAX_ANSWER} characters"
        )));
    }
    Ok(answers)
}

pub async fn create(
    state: &AppState,
    workspace_id: Uuid,
    actor: Uuid,
    req: &NewCustomResponse,
    ip: &ClientIp,
) -> AppResult<CustomResponse> {
    require_editor(state, workspace_id, actor).await?;
    let triggers = triggers_of(&req.triggers)?;
    let answers = answers_of(&req.answers)?;

    let mut tx = state.pool.begin().await?;
    CustomResponseRepo::lock_workspace_in(&mut tx, workspace_id).await?;
    if CustomResponseRepo::count_in(&mut tx, workspace_id).await? >= MAX_RESPONSES {
        return Err(AppError::Conflict(format!(
            "A workspace has at most {MAX_RESPONSES} custom responses"
        )));
    }
    let (_, joined) = ensure_bot_in(state, &mut tx, workspace_id).await?;
    let response =
        CustomResponseRepo::create_in(&mut tx, workspace_id, &triggers, &answers, actor).await?;
    tx.commit().await?;
    announce_bot(state, joined).await;

    audit::record(
        state,
        AuditEntry::new(AuditAction::CustomResponseCreated, actor)
            .workspace(workspace_id)
            .resource(response.id)
            .ip(ip)
            .details(serde_json::json!({ "triggers": response.triggers })),
    )
    .await;
    Ok(response)
}

pub async fn update(
    state: &AppState,
    workspace_id: Uuid,
    actor: Uuid,
    id: Uuid,
    req: &UpdateCustomResponse,
    ip: &ClientIp,
) -> AppResult<CustomResponse> {
    require_editor(state, workspace_id, actor).await?;
    let not_found = || AppError::NotFound("Custom response not found".into());
    let current = state
        .custom_response_repo
        .find(workspace_id, id)
        .await?
        .ok_or_else(not_found)?;
    let triggers = match &req.triggers {
        Some(raw) => triggers_of(raw)?,
        None => current.triggers,
    };
    let answers = match &req.answers {
        Some(raw) => answers_of(raw)?,
        None => current.answers,
    };
    let response = state
        .custom_response_repo
        .update(workspace_id, id, &triggers, &answers)
        .await?
        .ok_or_else(not_found)?;

    audit::record(
        state,
        AuditEntry::new(AuditAction::CustomResponseUpdated, actor)
            .workspace(workspace_id)
            .resource(id)
            .ip(ip)
            .details(serde_json::json!({ "triggers": response.triggers })),
    )
    .await;
    Ok(response)
}

pub async fn delete(
    state: &AppState,
    workspace_id: Uuid,
    actor: Uuid,
    id: Uuid,
    ip: &ClientIp,
) -> AppResult<()> {
    require_editor(state, workspace_id, actor).await?;
    let response = state
        .custom_response_repo
        .find(workspace_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Custom response not found".into()))?;
    state.custom_response_repo.delete(workspace_id, id).await?;

    audit::record(
        state,
        AuditEntry::new(AuditAction::CustomResponseDeleted, actor)
            .workspace(workspace_id)
            .resource(id)
            .ip(ip)
            .details(serde_json::json!({ "triggers": response.triggers })),
    )
    .await;
    Ok(())
}

pub async fn rename_bot(
    state: &AppState,
    workspace_id: Uuid,
    actor: Uuid,
    name: &str,
    ip: &ClientIp,
) -> AppResult<WorkspaceBot> {
    authz::require_workspace_role(state, workspace_id, actor, &WorkspaceRole::Admin).await?;
    validate_display_name(name)?;
    let name = name.trim();

    let mut tx = state.pool.begin().await?;
    let (user_id, joined) = ensure_bot_in(state, &mut tx, workspace_id).await?;
    WorkspaceBotRepo::rename_in(&mut tx, user_id, name).await?;
    tx.commit().await?;
    announce_bot(state, joined).await;

    audit::record(
        state,
        AuditEntry::new(AuditAction::WorkspaceBotRenamed, actor)
            .workspace(workspace_id)
            .resource(user_id)
            .ip(ip)
            .details(serde_json::json!({ "kind": KIND.as_str(), "name": name })),
    )
    .await;
    state
        .workspace_bot_repo
        .find(workspace_id, KIND)
        .await?
        .ok_or_else(|| AppError::Internal("The bot was not saved".into()))
}

pub async fn answer(
    state: &AppState,
    heard: &HeardMessage,
    response_id: Uuid,
) -> AppResult<Option<Uuid>> {
    let root = match heard.thread_parent_id {
        Some(parent) => Some(thread_root(state, heard.channel_id, parent).await?),
        None => None,
    };

    let mut tx = state.pool.begin().await?;
    let Some(answers) =
        CustomResponseRepo::claim_in(&mut tx, heard.message_id, response_id, heard.channel_id)
            .await?
    else {
        return Ok(None);
    };
    let Some(text) = answers.choose(&mut rand::rng()) else {
        return Ok(None);
    };
    let (bot_id, joined) = ensure_bot_in(state, &mut tx, heard.workspace_id).await?;
    let reply = state
        .message_repo
        .create_bot_reply_in(
            &mut tx,
            heard.channel_id,
            root.as_ref().map(|root| root.id),
            bot_id,
            text,
            &serde_json::json!({}),
            &[],
        )
        .await?;
    if let Some(root) = &root {
        state
            .thread_repo
            .follow_on_reply_in(
                &mut tx,
                ReplyFollows {
                    root_message_id: root.id,
                    workspace_id: heard.workspace_id,
                    replier_id: bot_id,
                    root_author_id: root.user_id,
                    mentioned: &[],
                    replied_at: reply.created_at,
                },
            )
            .await?;
    }
    CustomResponseRepo::record_reply_in(&mut tx, heard.message_id, reply.id).await?;
    let staged = state
        .publisher
        .stage_message_created(&mut tx, &reply, heard.workspace_id, &[])
        .await?;
    tx.commit().await?;
    announce_bot(state, joined).await;
    state.publisher.dispatch(staged).await;
    if let Some(root) = &root {
        announce_reply(state, root.id, heard.workspace_id).await;
    }
    Ok(Some(reply.id))
}
