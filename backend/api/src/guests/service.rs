use chrono::Utc;
use sqlx::PgConnection;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::repo::GuestRepo;
use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::authz;
use crate::state::AppState;
use crate::workspace::models::{
    ChannelType, CreateInviteRequest, GuestKind, GuestSeat, GuestTerms, UpdateGuestRequest,
    WorkspaceRole,
};

pub struct InviteTerms {
    pub role: WorkspaceRole,
    pub guest: Option<GuestTerms>,
    pub channel_ids: Vec<Uuid>,
}

pub async fn require_room_in(
    conn: &mut PgConnection,
    workspace_id: Uuid,
    user_ids: &[Uuid],
    channel_id: Uuid,
) -> AppResult<()> {
    let names =
        GuestRepo::single_guests_elsewhere_in(conn, workspace_id, user_ids, channel_id).await?;
    match names.as_slice() {
        [] => Ok(()),
        [one] => Err(AppError::Conflict(format!(
            "{one} is a single-channel guest in another channel. Make them a multi-channel guest first."
        ))),
        many => Err(AppError::Conflict(format!(
            "{} are single-channel guests in other channels. Make them multi-channel guests first.",
            many.join(", ")
        ))),
    }
}

pub async fn invite_terms(
    state: &AppState,
    workspace_id: Uuid,
    inviter: Uuid,
    req: &CreateInviteRequest,
) -> AppResult<InviteTerms> {
    if req.role.as_ref() != Some(&WorkspaceRole::Guest) {
        if req.guest_kind.is_some() || req.guest_until.is_some() || !req.channel_ids.is_empty() {
            return Err(AppError::Validation(
                "Only a guest invite names channels or an end date".into(),
            ));
        }
        return Ok(InviteTerms {
            role: req.role.clone().unwrap_or(WorkspaceRole::Member),
            guest: None,
            channel_ids: Vec::new(),
        });
    }
    if req.email.is_none() {
        return Err(AppError::Validation(
            "A guest is invited by email: a link cannot be held to one person".into(),
        ));
    }
    let mut channel_ids = req.channel_ids.clone();
    channel_ids.sort_unstable();
    channel_ids.dedup();
    if channel_ids.is_empty() {
        return Err(AppError::Validation(
            "A guest invite names the channels the guest joins".into(),
        ));
    }
    let kind = req.guest_kind.unwrap_or(GuestKind::Multi);
    if kind == GuestKind::Single && channel_ids.len() != 1 {
        return Err(AppError::Validation(
            "A single-channel guest is invited to one channel".into(),
        ));
    }
    require_future(req.guest_until)?;
    for id in &channel_ids {
        let channel = authz::require_channel_access(state, *id, inviter)
            .await?
            .channel;
        if channel.workspace_id != workspace_id
            || !matches!(
                channel.channel_type,
                ChannelType::Public | ChannelType::Private
            )
        {
            return Err(AppError::Validation(
                "A guest is invited to channels of this workspace".into(),
            ));
        }
        authz::require_not_archived(&channel)?;
    }
    Ok(InviteTerms {
        role: WorkspaceRole::Guest,
        guest: Some(GuestTerms {
            kind,
            until: req.guest_until,
            invited_by: Some(inviter),
        }),
        channel_ids,
    })
}

pub async fn set_seat(
    state: &AppState,
    workspace_id: Uuid,
    actor: Uuid,
    user_id: Uuid,
    req: &UpdateGuestRequest,
    ip: &ClientIp,
) -> AppResult<GuestSeat> {
    authz::require_workspace_role(state, workspace_id, actor, &WorkspaceRole::Admin).await?;
    require_future(req.guest_until)?;

    let mut tx = state.pool.begin().await?;
    GuestRepo::lock_seat_in(&mut tx, workspace_id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("No guest by that id in this workspace".into()))?;
    if req.guest_kind == GuestKind::Single
        && GuestRepo::channel_count_in(&mut tx, workspace_id, user_id).await? > 1
    {
        return Err(AppError::Conflict(
            "A single-channel guest is in one channel. Remove them from the others first.".into(),
        ));
    }
    let seat = GuestRepo::set_seat_in(
        &mut tx,
        workspace_id,
        user_id,
        req.guest_kind,
        req.guest_until,
    )
    .await?;
    tx.commit().await?;

    audit::record(
        state,
        AuditEntry::new(AuditAction::GuestUpdated, actor)
            .workspace(workspace_id)
            .resource(user_id)
            .ip(ip)
            .details(serde_json::json!({
                "guest_kind": seat.guest_kind,
                "guest_until": seat.guest_until,
            })),
    )
    .await;
    Ok(seat)
}

fn require_future(until: Option<chrono::DateTime<Utc>>) -> AppResult<()> {
    if until.is_some_and(|until| until <= Utc::now()) {
        return Err(AppError::Validation(
            "A guest's end date is in the future".into(),
        ));
    }
    Ok(())
}
