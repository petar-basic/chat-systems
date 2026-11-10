use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::membership;
use super::models::{WorkspaceMember, WorkspaceRole};
use crate::auth::models::UserStatus;
use crate::state::AppState;

fn last_owner() -> AppError {
    AppError::Forbidden("Make someone else an owner first".into())
}

pub async fn step_down(
    state: &AppState,
    workspace_id: Uuid,
    owner: Uuid,
    to: &WorkspaceRole,
) -> AppResult<WorkspaceMember> {
    let repo = &state.workspace_service.repo;
    let mut tx = state.pool.begin().await?;
    if repo.lock_owners(&mut tx, workspace_id, owner).await? == 0 {
        return Err(last_owner());
    }
    let member = repo
        .update_member_role(&mut tx, workspace_id, owner, to)
        .await?;
    repo.hand_over_owner_id(&mut tx, workspace_id, owner)
        .await?;
    tx.commit().await?;
    Ok(member)
}

pub async fn leave(state: &AppState, workspace_id: Uuid, owner: Uuid) -> AppResult<Vec<Uuid>> {
    let repo = &state.workspace_service.repo;
    let mut tx = state.pool.begin().await?;
    if repo.lock_owners(&mut tx, workspace_id, owner).await? == 0 {
        return Err(last_owner());
    }
    repo.hand_over_owner_id(&mut tx, workspace_id, owner)
        .await?;
    let detached = membership::detach_in(state, &mut tx, workspace_id, owner).await?;
    tx.commit().await?;
    Ok(detached.settle(state, workspace_id, owner).await)
}

pub struct Appointed {
    pub member: WorkspaceMember,
    pub was: WorkspaceRole,
}

pub async fn appoint(state: &AppState, workspace_id: Uuid, user_id: Uuid) -> AppResult<Appointed> {
    let repo = &state.workspace_service.repo;
    let person = repo
        .get_member(workspace_id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Not a member of this workspace".into()))?;
    if person.is_bot {
        return Err(AppError::BadRequest("An app can't own a workspace".into()));
    }
    let account = state
        .auth_service
        .repo()
        .find_by_id(user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".into()))?;
    if account.status != UserStatus::Active {
        return Err(AppError::BadRequest(
            "Only an active account can own a workspace".into(),
        ));
    }

    let mut tx = state.pool.begin().await?;
    let member = repo
        .update_member_role(&mut tx, workspace_id, user_id, &WorkspaceRole::Owner)
        .await?;
    repo.claim_owner_id(&mut tx, workspace_id, user_id).await?;
    tx.commit().await?;
    Ok(Appointed {
        member,
        was: person.role,
    })
}
