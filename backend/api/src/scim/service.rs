use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::auth::models::User;
use crate::sessions::{self, SessionScope};
use crate::state::AppState;

/// Deprovisioning is not one write. Suspending the account without ending the
/// sessions leaves the person connected; ending the sessions without dropping
/// the memberships leaves their private-channel access waiting for the next time
/// somebody re-enables them. All of it, or the two systems disagree silently —
/// which is the whole failure this endpoint exists to prevent.
pub async fn deactivate(
    state: &AppState,
    user_id: Uuid,
    token_id: Uuid,
    ip: &ClientIp,
) -> AppResult<()> {
    sqlx::query!(
        "UPDATE users SET status = 'suspended', updated_at = NOW() WHERE id = $1",
        user_id
    )
    .execute(&state.pool)
    .await?;

    sessions::revoke(state, user_id, SessionScope::All, "deprovisioned").await?;

    let workspaces = state
        .workspace_service
        .repo
        .list_user_workspaces(user_id)
        .await?;

    for workspace in &workspaces {
        let dropped = crate::workspace::membership::detach(state, workspace.id, user_id).await?;
        audit::record(
            state,
            AuditEntry::machine(AuditAction::WorkspaceMemberRemoved)
                .workspace(workspace.id)
                .resource(user_id)
                .ip(ip)
                .details(serde_json::json!({
                    "via": "scim",
                    "token_id": token_id,
                    "channels_dropped": dropped.len(),
                })),
        )
        .await;
    }

    let _ = state
        .publisher
        .publish("user.suspended", serde_json::json!({ "user_id": user_id }))
        .await;

    audit::record(
        state,
        AuditEntry::machine(AuditAction::UserSuspended)
            .resource(user_id)
            .ip(ip)
            .details(serde_json::json!({
                "via": "scim",
                "token_id": token_id,
                "workspaces_removed": workspaces.len(),
            })),
    )
    .await;

    Ok(())
}

/// Deliberately not the inverse of `deactivate`. Removal took the memberships
/// away for good, so coming back needs a fresh invite: an identity provider must
/// not be able to hand somebody their old private channels by flipping a flag.
pub async fn reactivate(
    state: &AppState,
    user_id: Uuid,
    token_id: Uuid,
    ip: &ClientIp,
) -> AppResult<()> {
    sqlx::query!(
        "UPDATE users SET status = 'active', updated_at = NOW() WHERE id = $1",
        user_id
    )
    .execute(&state.pool)
    .await?;

    sessions::restore(state, user_id).await;

    audit::record(
        state,
        AuditEntry::machine(AuditAction::UserActivated)
            .resource(user_id)
            .ip(ip)
            .details(serde_json::json!({
                "via": "scim",
                "token_id": token_id,
                "memberships_restored": false,
            })),
    )
    .await;

    Ok(())
}

pub async fn provision(
    state: &AppState,
    email: &str,
    display_name: Option<&str>,
    token_id: Uuid,
    ip: &ClientIp,
) -> AppResult<User> {
    let email = email.trim().to_lowercase();
    if !email.contains('@') {
        return Err(AppError::BadRequest("userName must be an email".into()));
    }

    let repo = state.auth_service.repo();
    if repo.find_by_email(&email).await?.is_some() {
        return Err(AppError::Conflict(
            "A user with that userName exists".into(),
        ));
    }

    let user = repo.create_sso_user(&email).await?;
    if let Some(name) = display_name {
        sqlx::query!(
            "UPDATE users SET display_name = $2, updated_at = NOW() WHERE id = $1",
            user.id,
            name
        )
        .execute(&state.pool)
        .await?;
    }

    audit::record(
        state,
        AuditEntry::machine(AuditAction::UserProvisioned)
            .resource(user.id)
            .ip(ip)
            .details(serde_json::json!({ "via": "scim", "token_id": token_id, "email": email })),
    )
    .await;

    repo.find_by_id(user.id)
        .await?
        .ok_or_else(|| AppError::Internal("Provisioned user vanished".into()))
}

pub async fn rename(state: &AppState, user_id: Uuid, display_name: &str) -> AppResult<()> {
    sqlx::query!(
        "UPDATE users SET display_name = $2, updated_at = NOW() WHERE id = $1",
        user_id,
        display_name
    )
    .execute(&state.pool)
    .await?;
    Ok(())
}
