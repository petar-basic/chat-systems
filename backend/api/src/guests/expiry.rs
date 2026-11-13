use std::sync::Arc;
use std::time::Duration;

use shared_events::frames::Departure;
use tracing::{info, warn};

use shared_common::errors::AppResult;

use super::repo::{EndingGuest, GuestRepo};
use crate::audit::{self, AuditAction, AuditEntry};
use crate::notifications::models::{NotificationData, NotificationType, SystemNotice};
use crate::state::AppState;
use crate::workspace::membership;

const CHECK_EVERY: Duration = Duration::from_secs(30);
const EXPIRE_PER_RUN: usize = 200;
pub const WARN_DAYS: i32 = 3;

pub async fn start(state: Arc<AppState>) {
    info!("Guest expiry started");
    loop {
        tokio::time::sleep(CHECK_EVERY).await;
        run(&state).await;
    }
}

pub async fn run(state: &AppState) {
    let expired = expire(state).await;
    if expired > 0 {
        info!(expired, "Removed guests whose access ended");
    }
    warn_ending(state).await;
}

async fn expire(state: &AppState) -> usize {
    let mut expired = 0;
    while expired < EXPIRE_PER_RUN {
        match expire_one(state).await {
            Ok(true) => expired += 1,
            Ok(false) => break,
            Err(e) => {
                warn!("could not remove a guest whose access ended: {e}");
                break;
            }
        }
    }
    expired
}

async fn expire_one(state: &AppState) -> AppResult<bool> {
    let mut tx = state.pool.begin().await?;
    let Some(guest) = GuestRepo::next_expired_in(&mut tx).await? else {
        return Ok(false);
    };
    let detached = membership::detach_in(
        state,
        &mut tx,
        guest.workspace_id,
        guest.user_id,
        Departure::GuestExpired,
    )
    .await?;
    tx.commit().await?;
    detached
        .settle(state, guest.workspace_id, guest.user_id)
        .await;

    audit::record(
        state,
        AuditEntry::machine(AuditAction::GuestExpired)
            .workspace(guest.workspace_id)
            .resource(guest.user_id)
            .details(serde_json::json!({ "guest_until": guest.guest_until })),
    )
    .await;
    Ok(true)
}

async fn warn_ending(state: &AppState) {
    let ending = match state.guest_repo.claim_ending(WARN_DAYS).await {
        Ok(ending) => ending,
        Err(e) => {
            warn!("could not find guests whose access is ending: {e}");
            return;
        }
    };
    for guest in &ending {
        if let Err(e) = notify_ending(state, guest).await {
            warn!("could not tell admins a guest's access is ending: {e}");
        }
    }
}

async fn notify_ending(state: &AppState, guest: &EndingGuest) -> AppResult<()> {
    let mut recipients = state.app_directory_repo.admins(guest.workspace_id).await?;
    if let Some(inviter) = guest.invited_by {
        if !recipients.contains(&inviter)
            && state
                .workspace_service
                .is_workspace_member(guest.workspace_id, inviter)
                .await?
        {
            recipients.push(inviter);
        }
    }
    let body = format!(
        "{}'s guest access ends on {}",
        guest.name,
        guest.guest_until.format("%Y-%m-%d")
    );
    let data = NotificationData {
        notice: Some(SystemNotice::GuestEnding),
        guest_user_id: Some(guest.user_id),
        guest_name: Some(guest.name.clone()),
        guest_until: Some(guest.guest_until),
        ..Default::default()
    };
    for user_id in recipients {
        state
            .notification_repo
            .create(
                user_id,
                guest.workspace_id,
                &NotificationType::System,
                Some(&body),
                &data,
            )
            .await?;
        let event = crate::notifications::push_event(
            user_id,
            guest.workspace_id,
            "system",
            Some(&body),
            &data,
        );
        let _ = state.publisher.publish("notification.push", event).await;
    }
    Ok(())
}
