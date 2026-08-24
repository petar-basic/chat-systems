use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::{get, patch};
use axum::{Json, Router};
use serde::Deserialize;
use uuid::Uuid;

use shared_common::errors::{is_unique_violation, AppError, AppResult};

use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::authz;
use crate::middleware::AuthUser;
use crate::state::AppState;
use crate::workspace::models::WorkspaceRole;

pub fn router(state: Arc<AppState>) -> Router {
    let routes = Router::new()
        .route(
            "/workspaces/:ws_id/groups",
            get(list_groups).post(create_group),
        )
        .route(
            "/workspaces/:ws_id/groups/:group_id",
            patch(update_group).delete(delete_group),
        )
        .route(
            "/workspaces/:ws_id/groups/:group_id/members",
            get(list_members).post(add_member),
        )
        .route(
            "/workspaces/:ws_id/groups/:group_id/members/:user_id",
            axum::routing::delete(remove_member),
        );

    crate::protected(state, routes)
}

/// The same shape as a channel name, for the same reason: it is typed after an
/// `@` and has to be unambiguous at a glance.
pub fn validate_handle(handle: &str) -> AppResult<String> {
    let handle = handle.trim().trim_start_matches('@').to_lowercase();

    if handle.len() < 2 || handle.len() > 64 {
        return Err(AppError::Validation(
            "A group handle is 2 to 64 characters".into(),
        ));
    }
    if !handle
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    {
        return Err(AppError::Validation(
            "A group handle uses lowercase letters, digits, dashes and underscores".into(),
        ));
    }
    // These already mean something in a message, and a group that shadowed one
    // would either never fire or fire twice.
    if matches!(handle.as_str(), "channel" | "here" | "everyone") {
        return Err(AppError::Validation(format!(
            "@{handle} is a broadcast mention on this instance"
        )));
    }

    Ok(handle)
}

/// Anybody in the workspace can see which groups exist -- a handle they cannot
/// discover is a handle they will mention by accident.
async fn list_groups(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    let groups = state.group_repo.list(ws_id, auth.user_id).await?;
    Ok(Json(serde_json::json!({ "data": groups })))
}

#[derive(Debug, Deserialize)]
pub struct CreateGroupRequest {
    pub handle: String,
    pub name: Option<String>,
    pub description: Option<String>,
}

async fn create_group(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<CreateGroupRequest>,
) -> AppResult<Json<serde_json::Value>> {
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;

    let handle = validate_handle(&req.handle)?;
    let name = req.name.unwrap_or_else(|| handle.clone());

    let group = state
        .group_repo
        .create(
            ws_id,
            &handle,
            &name,
            req.description.as_deref(),
            auth.user_id,
        )
        .await
        .map_err(|e| {
            if is_unique_violation(&e) {
                AppError::Conflict(format!("@{handle} already exists in this workspace"))
            } else {
                AppError::Database(e.to_string())
            }
        })?;

    audit::record(
        &state,
        AuditEntry::new(AuditAction::GroupCreated, auth.user_id)
            .workspace(ws_id)
            .resource(group.id)
            .ip(&ip)
            .details(serde_json::json!({ "handle": group.handle })),
    )
    .await;

    Ok(Json(serde_json::to_value(group).unwrap_or_default()))
}

#[derive(Debug, Deserialize)]
pub struct UpdateGroupRequest {
    pub name: String,
    pub description: Option<String>,
}

/// The handle is not editable. Messages already sent carry the group id, but the
/// text people read is the handle, and renaming it would silently rewrite the
/// history of who was asked.
async fn update_group(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path((ws_id, group_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<UpdateGroupRequest>,
) -> AppResult<Json<serde_json::Value>> {
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
    let group = load(&state, ws_id, group_id).await?;

    let updated = state
        .group_repo
        .update(group.id, req.name.trim(), req.description.as_deref())
        .await?;

    audit::record(
        &state,
        AuditEntry::new(AuditAction::GroupUpdated, auth.user_id)
            .workspace(ws_id)
            .resource(group.id)
            .ip(&ip),
    )
    .await;

    Ok(Json(serde_json::to_value(updated).unwrap_or_default()))
}

async fn delete_group(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path((ws_id, group_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
    let group = load(&state, ws_id, group_id).await?;

    state.group_repo.delete(group.id).await?;

    audit::record(
        &state,
        AuditEntry::new(AuditAction::GroupDeleted, auth.user_id)
            .workspace(ws_id)
            .resource(group.id)
            .ip(&ip)
            .details(serde_json::json!({ "handle": group.handle })),
    )
    .await;

    Ok(Json(serde_json::json!({ "status": "deleted" })))
}

async fn list_members(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((ws_id, group_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    let group = load(&state, ws_id, group_id).await?;
    let members = state.group_repo.list_member_ids(group.id).await?;
    Ok(Json(serde_json::json!({ "data": members })))
}

#[derive(Debug, Deserialize)]
pub struct MemberRequest {
    pub user_id: Uuid,
}

/// Membership is workspace membership first: a group is a shorthand for people
/// who are already here, not a way to reach somebody who is not.
async fn add_member(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path((ws_id, group_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<MemberRequest>,
) -> AppResult<Json<serde_json::Value>> {
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
    let group = load(&state, ws_id, group_id).await?;
    authz::require_workspace_member(&state, ws_id, req.user_id).await?;

    state.group_repo.add_member(group.id, req.user_id).await?;

    audit::record(
        &state,
        AuditEntry::new(AuditAction::GroupMemberAdded, auth.user_id)
            .workspace(ws_id)
            .resource(group.id)
            .ip(&ip)
            .details(serde_json::json!({ "handle": group.handle, "user_id": req.user_id })),
    )
    .await;

    Ok(Json(serde_json::json!({ "status": "added" })))
}

async fn remove_member(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path((ws_id, group_id, user_id)): Path<(Uuid, Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
    let group = load(&state, ws_id, group_id).await?;

    state.group_repo.remove_member(group.id, user_id).await?;

    audit::record(
        &state,
        AuditEntry::new(AuditAction::GroupMemberRemoved, auth.user_id)
            .workspace(ws_id)
            .resource(group.id)
            .ip(&ip)
            .details(serde_json::json!({ "handle": group.handle, "user_id": user_id })),
    )
    .await;

    Ok(Json(serde_json::json!({ "status": "removed" })))
}

async fn load(
    state: &AppState,
    ws_id: Uuid,
    group_id: Uuid,
) -> AppResult<crate::groups::repo::UserGroup> {
    state
        .group_repo
        .find(group_id)
        .await?
        .filter(|g| g.workspace_id == ws_id)
        .ok_or_else(|| AppError::NotFound("No such group".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_handle_is_normalized_the_way_people_type_it() {
        assert_eq!(validate_handle("@Backend").expect("valid"), "backend");
        assert_eq!(validate_handle("  on-call  ").expect("valid"), "on-call");
    }

    #[test]
    fn a_group_cannot_shadow_a_broadcast_mention() {
        assert!(validate_handle("channel").is_err());
        assert!(validate_handle("@here").is_err());
        assert!(validate_handle("everyone").is_err());
    }

    #[test]
    fn a_handle_that_would_not_parse_after_an_at_sign_is_refused() {
        assert!(validate_handle("a").is_err());
        assert!(validate_handle("two words").is_err());
        assert!(validate_handle("team!").is_err());
    }
}
