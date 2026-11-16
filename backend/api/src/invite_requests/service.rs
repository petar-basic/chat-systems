use garde::Validate;
use sqlx::PgConnection;
use tracing::warn;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::{
    ApproveInviteRequest, ApprovedInviteRequest, AskForInvites, DeclineInviteRequest,
    InviteRequestStatus, RequestTerms, WorkspaceInviteRequest,
};
use super::repo::{InviteRequestRepo, NewRequest};
use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::auth::service::InviteRequestEmail;
use crate::authz;
use crate::guests::service::{invite_terms, InviteTerms, TermsAsked};
use crate::notifications::models::{NotificationData, NotificationType, SystemNotice};
use crate::profile::models::Language;
use crate::state::AppState;
use crate::workspace::models::{
    CreatedInvite, InviteLifetime, Inviter, WhoMay, WorkspaceMember, WorkspaceRole,
};

pub async fn may_invite(
    state: &AppState,
    workspace_id: Uuid,
    member: &WorkspaceMember,
) -> AppResult<bool> {
    if member.role.has_at_least(&WorkspaceRole::Admin) {
        return Ok(true);
    }
    let workspace = state
        .workspace_service
        .repo
        .find_workspace_by_id(workspace_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Workspace not found".into()))?;
    Ok(WhoMay::inviters(&workspace.settings).allows(&member.role))
}

pub async fn require_inviter(
    state: &AppState,
    workspace_id: Uuid,
    member: &WorkspaceMember,
) -> AppResult<()> {
    if may_invite(state, workspace_id, member).await? {
        return Ok(());
    }
    Err(AppError::Forbidden(
        if member.role == WorkspaceRole::Guest {
            "A guest cannot invite people"
        } else {
            "Only admins invite people to this workspace. Ask one with a request instead."
        }
        .into(),
    ))
}

pub async fn ask(
    state: &AppState,
    workspace_id: Uuid,
    requester: Uuid,
    req: &AskForInvites,
    ip: &ClientIp,
) -> AppResult<WorkspaceInviteRequest> {
    let member =
        authz::require_workspace_role(state, workspace_id, requester, &WorkspaceRole::Member)
            .await?;
    if may_invite(state, workspace_id, &member).await? {
        return Err(AppError::BadRequest(
            "You can invite people yourself, without a request".into(),
        ));
    }
    req.validate()?;
    requestable(&req.terms.role)?;
    let emails = addresses(&req.terms.emails)?;
    let terms = invite_terms(state, workspace_id, requester, asked(&req.terms)).await?;
    let reason = trimmed(req.reason.as_deref());

    let mut tx = state.pool.begin().await?;
    InviteRequestRepo::lock_workspace_in(&mut tx, workspace_id).await?;
    require_newcomers(&mut tx, workspace_id, &emails).await?;
    let waiting = InviteRequestRepo::waiting_among_in(&mut tx, workspace_id, &emails).await?;
    if !waiting.is_empty() {
        return Err(AppError::Conflict(format!(
            "A request to invite {} is already waiting for an admin",
            waiting.join(", ")
        )));
    }
    let id = InviteRequestRepo::create_in(
        &mut tx,
        NewRequest {
            workspace_id,
            requested_by: requester,
            emails: &emails,
            terms: &terms,
            reason,
        },
    )
    .await?;
    tx.commit().await?;

    let request = found(state, id).await?;
    audit::record(
        state,
        AuditEntry::new(AuditAction::InviteRequestCreated, requester)
            .workspace(workspace_id)
            .resource(id)
            .ip(ip)
            .details(serde_json::json!({
                "emails": request.emails,
                "role": request.role,
                "guest_kind": request.guest_kind,
                "guest_until": request.guest_until,
                "channel_ids": request.channel_ids,
            })),
    )
    .await;
    if let Err(e) = tell_admins(state, &request).await {
        warn!("could not tell admins about an invite request: {e}");
    }
    Ok(request)
}

pub async fn list(
    state: &AppState,
    workspace_id: Uuid,
    viewer: Uuid,
) -> AppResult<Vec<WorkspaceInviteRequest>> {
    let member =
        authz::require_workspace_role(state, workspace_id, viewer, &WorkspaceRole::Member).await?;
    let only = (!member.role.has_at_least(&WorkspaceRole::Admin)).then_some(viewer);
    Ok(state
        .invite_request_repo
        .pending(workspace_id, only)
        .await?)
}

pub async fn approve(
    state: &AppState,
    workspace_id: Uuid,
    admin: Uuid,
    request_id: Uuid,
    req: &ApproveInviteRequest,
    language: Language,
    ip: &ClientIp,
) -> AppResult<ApprovedInviteRequest> {
    authz::require_workspace_role(state, workspace_id, admin, &WorkspaceRole::Admin).await?;
    req.validate()?;
    let request = waiting(state, workspace_id, request_id).await?;
    let stored = RequestTerms {
        emails: request.emails.clone(),
        role: request.role.clone(),
        guest_kind: request.guest_kind,
        guest_until: request.guest_until,
        channel_ids: request.channel_ids.clone(),
    };
    let chosen = req.terms.as_ref().unwrap_or(&stored);
    requestable(&chosen.role)?;
    let emails = addresses(&chosen.emails)?;
    let terms = invite_terms(state, workspace_id, admin, asked(chosen)).await?;

    let mut tx = state.pool.begin().await?;
    InviteRequestRepo::lock_workspace_in(&mut tx, workspace_id).await?;
    require_newcomers(&mut tx, workspace_id, &emails).await?;
    if !InviteRequestRepo::approve_in(&mut tx, request_id, admin, &emails, &terms).await? {
        return Err(answered());
    }
    tx.commit().await?;

    let inviter = Inviter {
        user_id: admin,
        language,
        on_behalf_of: Some(request.requested_by),
    };
    let mut invites = Vec::with_capacity(emails.len());
    for email in &emails {
        invites
            .push(send_invite(state, workspace_id, inviter, email, &terms, request_id, ip).await?);
    }

    let request = found(state, request_id).await?;
    audit::record(
        state,
        AuditEntry::new(AuditAction::InviteRequestApproved, admin)
            .workspace(workspace_id)
            .resource(request_id)
            .ip(ip)
            .details(serde_json::json!({
                "emails": request.emails,
                "role": request.role,
                "guest_kind": request.guest_kind,
                "guest_until": request.guest_until,
                "channel_ids": request.channel_ids,
                "edited": req.terms.is_some(),
            })),
    )
    .await;
    if let Err(e) = tell_requester(state, &request, SystemNotice::InviteRequestApproved).await {
        warn!("could not tell a requester their invite request was approved: {e}");
    }
    Ok(ApprovedInviteRequest { request, invites })
}

pub async fn decline(
    state: &AppState,
    workspace_id: Uuid,
    admin: Uuid,
    request_id: Uuid,
    req: &DeclineInviteRequest,
    ip: &ClientIp,
) -> AppResult<WorkspaceInviteRequest> {
    authz::require_workspace_role(state, workspace_id, admin, &WorkspaceRole::Admin).await?;
    req.validate()?;
    waiting(state, workspace_id, request_id).await?;
    let note = trimmed(req.note.as_deref());
    if !state
        .invite_request_repo
        .decline(request_id, admin, note)
        .await?
    {
        return Err(answered());
    }

    let request = found(state, request_id).await?;
    audit::record(
        state,
        AuditEntry::new(AuditAction::InviteRequestDeclined, admin)
            .workspace(workspace_id)
            .resource(request_id)
            .ip(ip)
            .details(serde_json::json!({ "emails": request.emails, "note": request.note })),
    )
    .await;
    if let Err(e) = tell_requester(state, &request, SystemNotice::InviteRequestDeclined).await {
        warn!("could not tell a requester their invite request was declined: {e}");
    }
    Ok(request)
}

pub async fn expire_due(state: &AppState, limit: i64) -> AppResult<usize> {
    let expired = state.invite_request_repo.expire_due(limit).await?;
    for row in &expired {
        audit::record(
            state,
            AuditEntry::machine(AuditAction::InviteRequestExpired)
                .workspace(row.workspace_id)
                .resource(row.id)
                .details(serde_json::json!({ "emails": row.emails })),
        )
        .await;
        match state.invite_request_repo.find(row.id).await {
            Ok(Some(request)) => {
                if let Err(e) =
                    tell_requester(state, &request, SystemNotice::InviteRequestExpired).await
                {
                    warn!("could not tell a requester their invite request expired: {e}");
                }
            }
            Ok(None) => {}
            Err(e) => warn!("could not read an expired invite request: {e}"),
        }
    }
    Ok(expired.len())
}

async fn send_invite(
    state: &AppState,
    workspace_id: Uuid,
    inviter: Inviter,
    email: &str,
    terms: &InviteTerms,
    request_id: Uuid,
    ip: &ClientIp,
) -> AppResult<CreatedInvite> {
    let created = state
        .workspace_service
        .create_invite(
            workspace_id,
            inviter,
            Some(email),
            InviteLifetime {
                max_uses: 1,
                expires_in_hours: InviteLifetime::MAX_HOURS,
            },
            terms.clone(),
            &state.auth_service,
        )
        .await?;
    audit::record(
        state,
        AuditEntry::new(AuditAction::InviteCreated, inviter.user_id)
            .workspace(workspace_id)
            .resource(created.invite.id)
            .ip(ip)
            .details(serde_json::json!({
                "email": created.invite.email,
                "role": created.invite.role,
                "expires_at": created.invite.expires_at,
                "email_status": created.email_status,
                "guest_kind": created.invite.guest_kind,
                "guest_until": created.invite.guest_until,
                "channel_ids": created.invite.channel_ids,
                "invite_request_id": request_id,
            })),
    )
    .await;
    Ok(created)
}

fn requestable(role: &WorkspaceRole) -> AppResult<()> {
    match role {
        WorkspaceRole::Member | WorkspaceRole::Guest => Ok(()),
        _ => Err(AppError::Forbidden(
            "A request asks for members or guests".into(),
        )),
    }
}

fn addresses(emails: &[String]) -> AppResult<Vec<String>> {
    let mut seen: Vec<String> = Vec::with_capacity(emails.len());
    for email in emails {
        let email = email.trim().to_lowercase();
        if seen.contains(&email) {
            return Err(AppError::Validation(format!("{email} is listed twice")));
        }
        seen.push(email);
    }
    Ok(seen)
}

fn asked(terms: &RequestTerms) -> TermsAsked<'_> {
    TermsAsked {
        role: Some(&terms.role),
        by_email: true,
        guest_kind: terms.guest_kind,
        guest_until: terms.guest_until,
        channel_ids: &terms.channel_ids,
    }
}

fn trimmed(text: Option<&str>) -> Option<&str> {
    text.map(str::trim).filter(|t| !t.is_empty())
}

fn answered() -> AppError {
    AppError::Conflict("This request was already answered".into())
}

async fn require_newcomers(
    conn: &mut PgConnection,
    workspace_id: Uuid,
    emails: &[String],
) -> AppResult<()> {
    let members = InviteRequestRepo::members_among_in(conn, workspace_id, emails).await?;
    match members.as_slice() {
        [] => Ok(()),
        [one] => Err(AppError::Conflict(format!(
            "{one} is already in this workspace"
        ))),
        many => Err(AppError::Conflict(format!(
            "{} are already in this workspace",
            many.join(", ")
        ))),
    }
}

async fn found(state: &AppState, request_id: Uuid) -> AppResult<WorkspaceInviteRequest> {
    state
        .invite_request_repo
        .find(request_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Invite request not found".into()))
}

async fn waiting(
    state: &AppState,
    workspace_id: Uuid,
    request_id: Uuid,
) -> AppResult<WorkspaceInviteRequest> {
    let request = state
        .invite_request_repo
        .find(request_id)
        .await?
        .filter(|r| r.workspace_id == workspace_id)
        .ok_or_else(|| AppError::NotFound("Invite request not found".into()))?;
    if request.status != InviteRequestStatus::Pending {
        return Err(answered());
    }
    Ok(request)
}

async fn notify(
    state: &AppState,
    user_id: Uuid,
    workspace_id: Uuid,
    body: &str,
    data: &NotificationData,
) -> AppResult<()> {
    state
        .notification_repo
        .create(
            user_id,
            workspace_id,
            &NotificationType::System,
            Some(body),
            data,
        )
        .await?;
    let event = crate::notifications::push_event(user_id, workspace_id, "system", Some(body), data);
    let _ = state.publisher.publish("notification.push", event).await;
    Ok(())
}

async fn tell_admins(state: &AppState, request: &WorkspaceInviteRequest) -> AppResult<()> {
    let workspace = state
        .workspace_service
        .repo
        .find_workspace_by_id(request.workspace_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Workspace not found".into()))?;
    let emails = request.emails.join(", ");
    let body = match &request.reason {
        Some(reason) => format!("{emails}: {reason}"),
        None => emails,
    };
    let data = NotificationData {
        notice: Some(SystemNotice::InviteRequested),
        invite_request_id: Some(request.id),
        invite_emails: Some(request.emails.clone()),
        requested_by: Some(request.requested_by),
        sender_name: Some(request.requester_name.clone()),
        ..Default::default()
    };
    let url = format!(
        "{}/app/{}?settings=members",
        state.config.public_url, request.workspace_id
    );
    for admin in state
        .app_directory_repo
        .admins(request.workspace_id)
        .await?
    {
        notify(state, admin, request.workspace_id, &body, &data).await?;
        let Some(user) = state.auth_service.repo().find_by_id(admin).await? else {
            continue;
        };
        let language = state.auth_service.language_for(admin, None).await?;
        state
            .auth_service
            .send_invite_request_email(
                &user.email,
                InviteRequestEmail {
                    workspace_name: &workspace.name,
                    requester: &request.requester_name,
                    emails: &request.emails,
                    as_guests: request.role == WorkspaceRole::Guest,
                    reason: request.reason.as_deref(),
                    url: &url,
                },
                language,
            )
            .await?;
    }
    Ok(())
}

async fn tell_requester(
    state: &AppState,
    request: &WorkspaceInviteRequest,
    notice: SystemNotice,
) -> AppResult<()> {
    if !state
        .workspace_service
        .is_workspace_member(request.workspace_id, request.requested_by)
        .await?
    {
        return Ok(());
    }
    let decider = match request.decided_by {
        Some(admin) => {
            state
                .workspace_service
                .repo
                .inviter_name(request.workspace_id, admin)
                .await?
        }
        None => None,
    };
    let data = NotificationData {
        notice: Some(notice),
        invite_request_id: Some(request.id),
        invite_emails: Some(request.emails.clone()),
        sender_name: decider,
        note: request.note.clone(),
        ..Default::default()
    };
    let body = request.emails.join(", ");
    notify(
        state,
        request.requested_by,
        request.workspace_id,
        &body,
        &data,
    )
    .await
}
