use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::routing::{delete, get, patch, post};
use axum::{middleware, Json, Router};
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};
use shared_common::validation;

use super::models::*;
use crate::middleware::{auth_middleware, AuthUser};
use crate::state::AppState;

pub fn router(state: Arc<AppState>) -> Router {
    let routes = Router::new()
        .route("/workspaces", get(list_workspaces))
        .route("/workspaces", post(create_workspace))
        .route("/workspaces/deleted", get(list_deleted_workspaces))
        .route("/workspaces/:ws_id", get(get_workspace))
        .route("/workspaces/:ws_id", patch(update_workspace))
        .route("/workspaces/:ws_id", delete(delete_workspace))
        .route("/workspaces/:ws_id/restore", post(restore_workspace))
        .route("/workspaces/:ws_id/members", get(list_members))
        .route(
            "/workspaces/:ws_id/members/:user_id/role",
            patch(update_member_role),
        )
        .route("/workspaces/:ws_id/members/:user_id", delete(remove_member))
        .route("/workspaces/:ws_id/invites", get(list_invites))
        .route("/workspaces/:ws_id/invites", post(create_invite))
        .route(
            "/workspaces/:ws_id/invites/:invite_id",
            delete(revoke_invite),
        )
        .route("/invites/:token/accept", post(accept_invite))
        .route("/workspaces/:ws_id/channels", get(list_channels))
        .route("/workspaces/:ws_id/channels/unread", get(unread_channels))
        .route("/workspaces/:ws_id/channels", post(create_channel))
        .route("/channels/:ch_id", get(get_channel))
        .route("/channels/:ch_id", patch(update_channel))
        .route("/channels/:ch_id", delete(archive_channel))
        .route(
            "/channels/:ch_id/notifications",
            patch(set_channel_notifications),
        )
        .route("/channels/:ch_id/members", get(list_channel_members))
        .route("/channels/:ch_id/members", post(add_channel_member))
        .route(
            "/channels/:ch_id/members/:user_id",
            delete(remove_channel_member),
        )
        .layer(middleware::from_fn(auth_middleware));

    Router::new().merge(routes).with_state(state)
}

async fn list_workspaces(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> AppResult<Json<serde_json::Value>> {
    let workspaces = state
        .workspace_service
        .repo
        .list_user_workspaces(auth.user_id)
        .await?;
    Ok(Json(serde_json::json!({ "data": workspaces })))
}

async fn create_workspace(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(req): Json<CreateWorkspaceRequest>,
) -> AppResult<Json<Workspace>> {
    validation::validate_workspace_name(&req.name)?;
    let workspace = state
        .workspace_service
        .create_workspace(&req.name, req.description.as_deref(), auth.user_id)
        .await?;
    Ok(Json(workspace))
}

async fn get_workspace(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<Workspace>> {
    require_member(&state, ws_id, auth.user_id).await?;
    let workspace = state
        .workspace_service
        .repo
        .find_workspace_by_id(ws_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Workspace not found".into()))?;
    Ok(Json(workspace))
}

async fn update_workspace(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<UpdateWorkspaceRequest>,
) -> AppResult<Json<Workspace>> {
    if let Some(name) = &req.name {
        validation::validate_workspace_name(name)?;
    }
    require_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
    let workspace = state
        .workspace_service
        .repo
        .update_workspace(
            ws_id,
            req.name.as_deref(),
            req.description.as_deref(),
            req.icon_url.as_deref(),
        )
        .await?;
    Ok(Json(workspace))
}

async fn delete_workspace(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Query(params): Query<DeleteWorkspaceRequest>,
) -> AppResult<Json<serde_json::Value>> {
    if !auth.is_instance_admin {
        require_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
    }
    let hard = params.hard.unwrap_or(false);
    if hard {
        state
            .workspace_service
            .repo
            .hard_delete_workspace(ws_id)
            .await?;
        let _ = state
            .publisher
            .publish_workspace_deleted(ws_id, "hard")
            .await;
        Ok(Json(serde_json::json!({ "status": "hard_deleted" })))
    } else {
        state
            .workspace_service
            .repo
            .soft_delete_workspace(ws_id)
            .await?;
        let _ = state
            .publisher
            .publish_workspace_deleted(ws_id, "soft")
            .await;
        Ok(Json(serde_json::json!({ "status": "soft_deleted" })))
    }
}

async fn restore_workspace(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<Workspace>> {
    if !auth.is_instance_admin {
        require_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
    }
    let workspace = state
        .workspace_service
        .repo
        .restore_workspace(ws_id)
        .await?;
    let _ = state.publisher.publish_workspace_restored(ws_id).await;
    Ok(Json(workspace))
}

async fn list_deleted_workspaces(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> AppResult<Json<serde_json::Value>> {
    let workspaces = state
        .workspace_service
        .repo
        .list_deleted_workspaces_for_user(auth.user_id, auth.is_instance_admin)
        .await?;
    Ok(Json(serde_json::json!({ "data": workspaces })))
}

async fn list_members(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    require_member(&state, ws_id, auth.user_id).await?;
    let members = state
        .workspace_service
        .repo
        .list_members_with_users(ws_id)
        .await?;
    Ok(Json(serde_json::json!({ "data": members })))
}

async fn update_member_role(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((ws_id, user_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<UpdateMemberRoleRequest>,
) -> AppResult<Json<WorkspaceMember>> {
    require_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
    let member = state
        .workspace_service
        .repo
        .update_member_role(ws_id, user_id, &req.role)
        .await?;
    Ok(Json(member))
}

async fn remove_member(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((ws_id, user_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
    require_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
    state
        .workspace_service
        .repo
        .remove_member(ws_id, user_id)
        .await?;
    Ok(Json(serde_json::json!({ "status": "removed" })))
}

async fn list_invites(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    require_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
    let invites = state.workspace_service.repo.list_invites(ws_id).await?;
    Ok(Json(serde_json::json!({ "data": invites })))
}

async fn create_invite(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<CreateInviteRequest>,
) -> AppResult<Json<WorkspaceInvite>> {
    require_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
    let invite = state
        .workspace_service
        .create_invite(
            ws_id,
            auth.user_id,
            req.email.as_deref(),
            req.role,
            &state.auth_service,
        )
        .await?;
    Ok(Json(invite))
}

async fn accept_invite(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(token): Path<String>,
) -> AppResult<Json<WorkspaceMember>> {
    let member = state
        .workspace_service
        .accept_invite(&token, auth.user_id)
        .await?;
    Ok(Json(member))
}

async fn revoke_invite(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((ws_id, invite_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
    require_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
    let invite = state
        .workspace_service
        .repo
        .find_invite_by_id(invite_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Invite not found".into()))?;
    if invite.workspace_id != ws_id {
        return Err(AppError::NotFound("Invite not found".into()));
    }
    state
        .workspace_service
        .repo
        .delete_invite(invite_id)
        .await?;
    Ok(Json(serde_json::json!({ "status": "revoked" })))
}

async fn list_channels(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    require_member(&state, ws_id, auth.user_id).await?;
    let channels = state
        .workspace_service
        .repo
        .list_user_channels(ws_id, auth.user_id)
        .await?;
    let muted: std::collections::HashSet<Uuid> = state
        .workspace_service
        .repo
        .muted_channel_ids(ws_id, auth.user_id)
        .await?
        .into_iter()
        .collect();
    let data: Vec<serde_json::Value> = channels
        .into_iter()
        .map(|c| {
            let is_muted = muted.contains(&c.id);
            let mut json = serde_json::to_value(&c).unwrap_or_default();
            if let Some(obj) = json.as_object_mut() {
                obj.insert("muted".to_string(), serde_json::json!(is_muted));
            }
            json
        })
        .collect();
    Ok(Json(serde_json::json!({ "data": data })))
}

async fn set_channel_notifications(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ch_id): Path<Uuid>,
    Json(req): Json<SetChannelNotificationsRequest>,
) -> AppResult<Json<serde_json::Value>> {
    state
        .workspace_service
        .repo
        .set_channel_muted(ch_id, auth.user_id, req.muted)
        .await?;
    Ok(Json(serde_json::json!({ "muted": req.muted })))
}

async fn unread_channels(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    require_member(&state, ws_id, auth.user_id).await?;
    let channel_ids = state
        .workspace_service
        .repo
        .unread_channel_ids(ws_id, auth.user_id)
        .await?;
    Ok(Json(serde_json::json!({ "channel_ids": channel_ids })))
}

async fn create_channel(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<CreateChannelRequest>,
) -> AppResult<Json<Channel>> {
    validation::validate_channel_name(&req.name)?;
    require_member(&state, ws_id, auth.user_id).await?;
    let channel_type = req.channel_type.unwrap_or(ChannelType::Public);
    let channel = state
        .workspace_service
        .repo
        .create_channel(
            ws_id,
            &req.name,
            &channel_type,
            req.description.as_deref(),
            auth.user_id,
            req.is_default.unwrap_or(false),
        )
        .await?;

    let _ = state
        .workspace_service
        .repo
        .add_channel_member(channel.id, auth.user_id, &ChannelRole::Admin)
        .await;

    Ok(Json(channel))
}

async fn get_channel(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ch_id): Path<Uuid>,
) -> AppResult<Json<Channel>> {
    let channel = state
        .workspace_service
        .repo
        .find_channel_by_id(ch_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Channel not found".into()))?;
    require_member(&state, channel.workspace_id, auth.user_id).await?;
    Ok(Json(channel))
}

async fn update_channel(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ch_id): Path<Uuid>,
    Json(req): Json<UpdateChannelRequest>,
) -> AppResult<Json<Channel>> {
    if let Some(name) = &req.name {
        validation::validate_channel_name(name)?;
    }
    let channel = state
        .workspace_service
        .repo
        .find_channel_by_id(ch_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Channel not found".into()))?;
    require_role(
        &state,
        channel.workspace_id,
        auth.user_id,
        &WorkspaceRole::Admin,
    )
    .await?;
    let updated = state
        .workspace_service
        .repo
        .update_channel(
            ch_id,
            req.name.as_deref(),
            req.topic.as_deref(),
            req.description.as_deref(),
        )
        .await?;
    Ok(Json(updated))
}

async fn archive_channel(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ch_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let channel = state
        .workspace_service
        .repo
        .find_channel_by_id(ch_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Channel not found".into()))?;
    require_role(
        &state,
        channel.workspace_id,
        auth.user_id,
        &WorkspaceRole::Admin,
    )
    .await?;
    state.workspace_service.repo.archive_channel(ch_id).await?;
    Ok(Json(serde_json::json!({ "status": "archived" })))
}

async fn list_channel_members(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ch_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let channel = state
        .workspace_service
        .repo
        .find_channel_by_id(ch_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Channel not found".into()))?;
    require_member(&state, channel.workspace_id, auth.user_id).await?;
    let members = state
        .workspace_service
        .repo
        .list_channel_members(ch_id)
        .await?;
    Ok(Json(serde_json::json!({ "data": members })))
}

async fn add_channel_member(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ch_id): Path<Uuid>,
    Json(req): Json<AddChannelMemberRequest>,
) -> AppResult<Json<ChannelMember>> {
    let channel = state
        .workspace_service
        .repo
        .find_channel_by_id(ch_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Channel not found".into()))?;
    require_role(
        &state,
        channel.workspace_id,
        auth.user_id,
        &WorkspaceRole::Admin,
    )
    .await?;
    let member = state
        .workspace_service
        .repo
        .add_channel_member(ch_id, req.user_id, &ChannelRole::Member)
        .await?;
    Ok(Json(member))
}

async fn remove_channel_member(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((ch_id, user_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
    if auth.user_id != user_id {
        let channel = state
            .workspace_service
            .repo
            .find_channel_by_id(ch_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Channel not found".into()))?;
        require_role(
            &state,
            channel.workspace_id,
            auth.user_id,
            &WorkspaceRole::Admin,
        )
        .await?;
    }
    state
        .workspace_service
        .repo
        .remove_channel_member(ch_id, user_id)
        .await?;
    Ok(Json(serde_json::json!({ "status": "removed" })))
}

async fn require_member(
    state: &AppState,
    workspace_id: Uuid,
    user_id: Uuid,
) -> AppResult<WorkspaceMember> {
    state
        .workspace_service
        .repo
        .get_member(workspace_id, user_id)
        .await?
        .ok_or_else(|| AppError::Forbidden("Not a member of this workspace".into()))
}

async fn require_role(
    state: &AppState,
    workspace_id: Uuid,
    user_id: Uuid,
    minimum: &WorkspaceRole,
) -> AppResult<WorkspaceMember> {
    let member = require_member(state, workspace_id, user_id).await?;
    if !member.role.has_at_least(minimum) {
        return Err(AppError::Forbidden(format!(
            "Requires at least {minimum:?} role"
        )));
    }
    Ok(member)
}
