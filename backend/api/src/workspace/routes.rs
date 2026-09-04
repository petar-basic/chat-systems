use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::Json;
use garde::Validate;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::*;
use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::authz;
use crate::dto::{DataList, StatusResponse};
use crate::middleware::AuthUser;
use crate::state::AppState;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(list_workspaces, create_workspace))
        .routes(routes!(list_deleted_workspaces))
        .routes(routes!(get_workspace, update_workspace, delete_workspace))
        .routes(routes!(restore_workspace))
        .routes(routes!(list_audit_log))
        .routes(routes!(list_members))
        .routes(routes!(update_member_role))
        .routes(routes!(remove_member))
        .routes(routes!(list_invites, create_invite))
        .routes(routes!(revoke_invite))
        .routes(routes!(accept_invite))
        .routes(routes!(list_channels, create_channel))
        .routes(routes!(unread_channels))
        .routes(routes!(browse_channels))
        .routes(routes!(get_channel, update_channel, archive_channel))
        .routes(routes!(set_channel_notifications))
        .routes(routes!(list_channel_members, add_channel_member))
        .routes(routes!(join_channel))
        .routes(routes!(update_channel_member_role))
        .routes(routes!(remove_channel_member))
        .routes(routes!(list_channel_bookmarks, create_channel_bookmark))
        .routes(routes!(delete_channel_bookmark))
}

#[utoipa::path(
    operation_id = "workspace_list_workspaces",
    get, path = "/workspaces", tag = "workspaces", responses((status = 200, body = DataList<Workspace>)))]
async fn list_workspaces(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> AppResult<Json<DataList<Workspace>>> {
    let workspaces = state
        .workspace_service
        .repo
        .list_user_workspaces(auth.user_id)
        .await?;
    Ok(Json(workspaces.into()))
}

#[utoipa::path(post, path = "/workspaces", tag = "workspaces", request_body = CreateWorkspaceRequest, responses((status = 200, body = Workspace)))]
async fn create_workspace(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Json(req): Json<CreateWorkspaceRequest>,
) -> AppResult<Json<Workspace>> {
    req.validate()?;
    let workspace = state
        .workspace_service
        .create_workspace(&req.name, req.description.as_deref(), auth.user_id)
        .await?;

    audit::record(
        &state,
        AuditEntry::new(AuditAction::WorkspaceCreated, auth.user_id)
            .workspace(workspace.id)
            .resource(workspace.id)
            .ip(&ip)
            .details(serde_json::json!({ "name": workspace.name })),
    )
    .await;

    Ok(Json(workspace))
}

#[utoipa::path(get, path = "/workspaces/{ws_id}", tag = "workspaces", responses((status = 200, body = Workspace)))]
async fn get_workspace(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<Workspace>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    let workspace = state
        .workspace_service
        .repo
        .find_workspace_by_id(ws_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Workspace not found".into()))?;
    Ok(Json(workspace))
}

#[utoipa::path(patch, path = "/workspaces/{ws_id}", tag = "workspaces", request_body = UpdateWorkspaceRequest, responses((status = 200, body = Workspace)))]
async fn update_workspace(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<UpdateWorkspaceRequest>,
) -> AppResult<Json<Workspace>> {
    req.validate()?;
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
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

#[utoipa::path(
    operation_id = "workspace_delete_workspace",
    delete, path = "/workspaces/{ws_id}", tag = "workspaces", params(DeleteWorkspaceRequest), responses((status = 200, body = StatusResponse)))]
async fn delete_workspace(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(ws_id): Path<Uuid>,
    Query(params): Query<DeleteWorkspaceRequest>,
) -> AppResult<Json<StatusResponse>> {
    if !auth.is_instance_admin {
        authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Owner).await?;
    }
    let hard = params.hard.unwrap_or(false);
    let entry = AuditEntry::new(AuditAction::WorkspaceDeleted, auth.user_id)
        .workspace(ws_id)
        .resource(ws_id)
        .ip(&ip)
        .details(serde_json::json!({ "hard": hard }));

    let mut tx = state.pool.begin().await?;
    if hard {
        state
            .workspace_service
            .repo
            .hard_delete_workspace_in(&mut tx, ws_id)
            .await?;
    } else {
        state
            .workspace_service
            .repo
            .soft_delete_workspace_in(&mut tx, ws_id)
            .await?;
    }
    let delete_type = if hard { "hard" } else { "soft" };
    let staged = state
        .publisher
        .stage_workspace_deleted(&mut tx, ws_id, delete_type)
        .await?;
    tx.commit().await?;
    state.publisher.dispatch(staged).await;
    audit::record(&state, entry).await;
    Ok(Json(StatusResponse::new(if hard {
        "hard_deleted"
    } else {
        "soft_deleted"
    })))
}

#[utoipa::path(post, path = "/workspaces/{ws_id}/restore", tag = "workspaces", responses((status = 200, body = Workspace)))]
async fn restore_workspace(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<Workspace>> {
    if !auth.is_instance_admin {
        authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Owner).await?;
    }
    let mut tx = state.pool.begin().await?;
    let workspace = state
        .workspace_service
        .repo
        .restore_workspace_in(&mut tx, ws_id)
        .await?;
    let staged = state
        .publisher
        .stage_workspace_restored(&mut tx, ws_id)
        .await?;
    tx.commit().await?;
    state.publisher.dispatch(staged).await;

    audit::record(
        &state,
        AuditEntry::new(AuditAction::WorkspaceRestored, auth.user_id)
            .workspace(ws_id)
            .resource(ws_id)
            .ip(&ip),
    )
    .await;

    Ok(Json(workspace))
}

#[utoipa::path(
    operation_id = "workspace_list_audit_log",
    get, path = "/workspaces/{ws_id}/audit-log", tag = "workspaces", params(audit::AuditQuery), responses((status = 200, body = DataList<audit::AuditRow>)))]
async fn list_audit_log(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Query(query): Query<audit::AuditQuery>,
) -> AppResult<Json<DataList<audit::AuditRow>>> {
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
    let entries = audit::list(&state, Some(ws_id), &query).await?;
    Ok(Json(entries.into()))
}

#[utoipa::path(get, path = "/workspaces/deleted", tag = "workspaces", responses((status = 200, body = DataList<Workspace>)))]
async fn list_deleted_workspaces(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> AppResult<Json<DataList<Workspace>>> {
    let workspaces = state
        .workspace_service
        .repo
        .list_deleted_workspaces_for_user(auth.user_id, auth.is_instance_admin)
        .await?;
    Ok(Json(workspaces.into()))
}

#[utoipa::path(
    operation_id = "workspace_list_members",
    get, path = "/workspaces/{ws_id}/members", tag = "workspaces", responses((status = 200, body = DataList<MemberWithUser>)))]
async fn list_members(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<DataList<MemberWithUser>>> {
    let member = authz::require_workspace_member(&state, ws_id, auth.user_id).await?;

    // A guest is somebody from outside who was let into a room. Handing them the
    // company's directory, with addresses, is the one door the rest of the guest
    // rules leave open.
    let members = if member.role == WorkspaceRole::Guest {
        state
            .workspace_service
            .repo
            .list_members_visible_to_guest(ws_id, auth.user_id)
            .await?
    } else {
        state
            .workspace_service
            .repo
            .list_members_with_users(ws_id)
            .await?
    };

    Ok(Json(members.into()))
}

#[utoipa::path(patch, path = "/workspaces/{ws_id}/members/{user_id}/role", tag = "workspaces", request_body = UpdateMemberRoleRequest, responses((status = 200, body = WorkspaceMember)))]
async fn update_member_role(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path((ws_id, user_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<UpdateMemberRoleRequest>,
) -> AppResult<Json<WorkspaceMember>> {
    let actor =
        authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
    let target = authz::require_workspace_member(&state, ws_id, user_id).await?;

    if !authz::outranks(&actor.role, &target.role) {
        return Err(AppError::Forbidden(
            "Cannot change the role of a member at or above your own level".into(),
        ));
    }
    if !actor.role.has_at_least(&req.role) {
        return Err(AppError::Forbidden(
            "Cannot grant a role above your own level".into(),
        ));
    }
    if target.role == WorkspaceRole::Owner && req.role != WorkspaceRole::Owner {
        return Err(AppError::Forbidden(
            "The workspace owner cannot be demoted".into(),
        ));
    }

    let member = state
        .workspace_service
        .repo
        .update_member_role(ws_id, user_id, &req.role)
        .await?;

    audit::record(
        &state,
        AuditEntry::new(AuditAction::WorkspaceRoleChanged, auth.user_id)
            .workspace(ws_id)
            .resource(user_id)
            .ip(&ip)
            .details(serde_json::json!({ "from": target.role, "to": req.role })),
    )
    .await;

    Ok(Json(member))
}

#[utoipa::path(
    operation_id = "workspace_remove_member",
    delete, path = "/workspaces/{ws_id}/members/{user_id}", tag = "workspaces", responses((status = 200, body = StatusResponse)))]
async fn remove_member(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path((ws_id, user_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<StatusResponse>> {
    let target = authz::require_workspace_member(&state, ws_id, user_id).await?;
    if target.role == WorkspaceRole::Owner {
        return Err(AppError::Forbidden(
            "The workspace owner cannot be removed".into(),
        ));
    }
    if auth.user_id != user_id {
        let actor =
            authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin)
                .await?;
        if !authz::outranks(&actor.role, &target.role) {
            return Err(AppError::Forbidden(
                "Cannot remove a member at or above your own level".into(),
            ));
        }
    }
    crate::workspace::membership::detach(&state, ws_id, user_id).await?;

    audit::record(
        &state,
        AuditEntry::new(AuditAction::WorkspaceMemberRemoved, auth.user_id)
            .workspace(ws_id)
            .resource(user_id)
            .ip(&ip)
            .details(serde_json::json!({ "self_service": auth.user_id == user_id })),
    )
    .await;

    Ok(Json(StatusResponse::new("removed")))
}

#[utoipa::path(get, path = "/workspaces/{ws_id}/invites", tag = "workspaces", responses((status = 200, body = DataList<WorkspaceInvite>)))]
async fn list_invites(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<DataList<WorkspaceInvite>>> {
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
    let invites = state.workspace_service.repo.list_invites(ws_id).await?;
    Ok(Json(invites.into()))
}

#[utoipa::path(post, path = "/workspaces/{ws_id}/invites", tag = "workspaces", request_body = CreateInviteRequest, responses((status = 200, body = WorkspaceInvite)))]
async fn create_invite(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<CreateInviteRequest>,
) -> AppResult<Json<WorkspaceInvite>> {
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
    req.validate()?;
    let lifetime = InviteLifetime::resolve(&req)?;
    let invite = state
        .workspace_service
        .create_invite(
            ws_id,
            auth.user_id,
            req.email.as_deref(),
            req.role,
            lifetime,
            &state.auth_service,
        )
        .await?;

    audit::record(
        &state,
        AuditEntry::new(AuditAction::InviteCreated, auth.user_id)
            .workspace(ws_id)
            .resource(invite.id)
            .ip(&ip)
            .details(serde_json::json!({
                "email": invite.email,
                "role": invite.role,
                "expires_at": invite.expires_at,
            })),
    )
    .await;

    Ok(Json(invite))
}

#[utoipa::path(
    operation_id = "workspace_accept_invite",
    post, path = "/invites/{token}/accept", tag = "workspaces", responses((status = 200, body = WorkspaceMember)))]
async fn accept_invite(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(token): Path<String>,
) -> AppResult<Json<WorkspaceMember>> {
    let user = state
        .auth_service
        .repo()
        .find_by_id(auth.user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".into()))?;
    let member = state
        .workspace_service
        .accept_invite(&token, auth.user_id, &user.email)
        .await?;
    Ok(Json(member))
}

#[utoipa::path(delete, path = "/workspaces/{ws_id}/invites/{invite_id}", tag = "workspaces", responses((status = 200, body = StatusResponse)))]
async fn revoke_invite(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path((ws_id, invite_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<StatusResponse>> {
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
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

    audit::record(
        &state,
        AuditEntry::new(AuditAction::InviteRevoked, auth.user_id)
            .workspace(ws_id)
            .resource(invite_id)
            .ip(&ip)
            .details(serde_json::json!({ "email": invite.email })),
    )
    .await;

    Ok(Json(StatusResponse::new("revoked")))
}

#[utoipa::path(get, path = "/workspaces/{ws_id}/channels", tag = "channels", responses((status = 200, body = DataList<ChannelListing>)))]
async fn list_channels(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<DataList<ChannelListing>>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
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
    let data: Vec<ChannelListing> = channels
        .into_iter()
        .map(|channel| ChannelListing {
            muted: muted.contains(&channel.id),
            channel,
        })
        .collect();
    Ok(Json(DataList { data }))
}

#[utoipa::path(patch, path = "/channels/{ch_id}/notifications", tag = "channels", request_body = SetChannelNotificationsRequest, responses((status = 200, body = MutedResponse)))]
async fn set_channel_notifications(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ch_id): Path<Uuid>,
    Json(req): Json<SetChannelNotificationsRequest>,
) -> AppResult<Json<MutedResponse>> {
    state
        .workspace_service
        .repo
        .set_channel_muted(ch_id, auth.user_id, req.muted)
        .await?;
    Ok(Json(MutedResponse { muted: req.muted }))
}

#[utoipa::path(get, path = "/workspaces/{ws_id}/channels/unread", tag = "channels", responses((status = 200, body = UnreadChannels)))]
async fn unread_channels(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<UnreadChannels>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    let counts = state
        .message_repo
        .unread_counts(ws_id, auth.user_id)
        .await?;

    let channel_ids: Vec<Uuid> = counts
        .iter()
        .filter(|(_, unread, _, _)| *unread > 0)
        .map(|(id, _, _, _)| *id)
        .collect();
    let counts: Vec<UnreadCount> = counts
        .into_iter()
        .map(
            |(channel_id, unread_count, mention_count, last_read_msg)| UnreadCount {
                channel_id,
                unread_count,
                mention_count,
                last_read_msg,
            },
        )
        .collect();

    Ok(Json(UnreadChannels {
        channel_ids,
        counts,
    }))
}

fn duplicate_channel_name(err: sqlx::Error) -> AppError {
    if shared_common::errors::is_unique_violation(&err) {
        AppError::Conflict("A channel with that name already exists".into())
    } else {
        err.into()
    }
}

#[utoipa::path(post, path = "/workspaces/{ws_id}/channels", tag = "channels", request_body = CreateChannelRequest, responses((status = 200, body = Channel)))]
async fn create_channel(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<CreateChannelRequest>,
) -> AppResult<Json<Channel>> {
    req.validate()?;
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Member).await?;
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
        .await
        .map_err(duplicate_channel_name)?;

    let _ = state
        .workspace_service
        .repo
        .add_channel_member(channel.id, auth.user_id, &ChannelRole::Admin)
        .await;

    let channel = match req.post_policy {
        Some(policy) => {
            state
                .workspace_service
                .repo
                .set_channel_post_policy(channel.id, policy.as_str())
                .await?
        }
        None => channel,
    };

    audit::record(
        &state,
        AuditEntry::new(AuditAction::ChannelCreated, auth.user_id)
            .workspace(ws_id)
            .resource(channel.id)
            .ip(&ip)
            .details(serde_json::json!({
                "name": channel.name,
                "channel_type": channel_type,
            })),
    )
    .await;

    Ok(Json(channel))
}

#[utoipa::path(get, path = "/channels/{ch_id}", tag = "channels", responses((status = 200, body = Channel)))]
async fn get_channel(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ch_id): Path<Uuid>,
) -> AppResult<Json<Channel>> {
    let channel = authz::find_channel(&state, ch_id).await?;
    authz::require_workspace_member(&state, channel.workspace_id, auth.user_id).await?;
    Ok(Json(channel))
}

#[utoipa::path(patch, path = "/channels/{ch_id}", tag = "channels", request_body = UpdateChannelRequest, responses((status = 200, body = Channel)))]
async fn update_channel(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(ch_id): Path<Uuid>,
    Json(req): Json<UpdateChannelRequest>,
) -> AppResult<Json<Channel>> {
    req.validate()?;
    let channel = authz::find_channel(&state, ch_id).await?;
    authz::require_channel_moderator(&state, &channel, auth.user_id).await?;
    let mut updated = state
        .workspace_service
        .repo
        .update_channel(
            ch_id,
            req.name.as_deref(),
            req.topic.as_deref(),
            req.description.as_deref(),
        )
        .await
        .map_err(duplicate_channel_name)?;

    if let Some(policy) = req.post_policy {
        let before = channel
            .settings
            .get("post_policy")
            .and_then(|v| v.as_str())
            .unwrap_or("everyone")
            .to_string();

        updated = state
            .workspace_service
            .repo
            .set_channel_post_policy(ch_id, policy.as_str())
            .await?;

        // "Who silenced this channel" is exactly the question an audit log is
        // for, so it is recorded separately from the rename.
        audit::record(
            &state,
            AuditEntry::new(AuditAction::ChannelPostPolicyChanged, auth.user_id)
                .workspace(channel.workspace_id)
                .resource(ch_id)
                .ip(&ip)
                .details(serde_json::json!({ "from": before, "to": policy.as_str() })),
        )
        .await;
    }

    audit::record(
        &state,
        AuditEntry::new(AuditAction::ChannelUpdated, auth.user_id)
            .workspace(channel.workspace_id)
            .resource(ch_id)
            .ip(&ip)
            .details(serde_json::json!({ "from": channel.name, "to": updated.name })),
    )
    .await;

    Ok(Json(updated))
}

#[utoipa::path(get, path = "/workspaces/{ws_id}/channels/browse", tag = "channels", responses((status = 200, body = DataList<BrowsableChannel>)))]
async fn browse_channels(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<DataList<BrowsableChannel>>> {
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Member).await?;
    let channels = state
        .workspace_service
        .repo
        .list_browsable_channels(ws_id, auth.user_id)
        .await?;
    Ok(Json(channels.into()))
}

#[utoipa::path(post, path = "/channels/{ch_id}/join", tag = "channels", responses((status = 200, body = ChannelMember)))]
async fn join_channel(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ch_id): Path<Uuid>,
) -> AppResult<Json<ChannelMember>> {
    let channel = authz::find_channel(&state, ch_id).await?;
    authz::require_workspace_role(
        &state,
        channel.workspace_id,
        auth.user_id,
        &WorkspaceRole::Member,
    )
    .await?;
    if channel.channel_type != ChannelType::Public {
        return Err(AppError::Forbidden(
            "Only public channels can be joined directly".into(),
        ));
    }
    if channel.is_archived {
        return Err(AppError::Forbidden(
            "This channel is archived and cannot be joined".into(),
        ));
    }
    let member = state
        .workspace_service
        .repo
        .add_channel_member(ch_id, auth.user_id, &ChannelRole::Member)
        .await?;
    Ok(Json(member))
}

#[utoipa::path(delete, path = "/channels/{ch_id}", tag = "channels", responses((status = 200, body = StatusResponse)))]
async fn archive_channel(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(ch_id): Path<Uuid>,
) -> AppResult<Json<StatusResponse>> {
    let channel = authz::find_channel(&state, ch_id).await?;
    authz::require_channel_moderator(&state, &channel, auth.user_id).await?;
    state.workspace_service.repo.archive_channel(ch_id).await?;

    audit::record(
        &state,
        AuditEntry::new(AuditAction::ChannelArchived, auth.user_id)
            .workspace(channel.workspace_id)
            .resource(ch_id)
            .ip(&ip)
            .details(serde_json::json!({ "name": channel.name })),
    )
    .await;

    Ok(Json(StatusResponse::new("archived")))
}

#[utoipa::path(get, path = "/channels/{ch_id}/members", tag = "channels", responses((status = 200, body = DataList<ChannelMember>)))]
async fn list_channel_members(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ch_id): Path<Uuid>,
) -> AppResult<Json<DataList<ChannelMember>>> {
    let channel = authz::find_channel(&state, ch_id).await?;
    authz::require_workspace_member(&state, channel.workspace_id, auth.user_id).await?;
    let members = state
        .workspace_service
        .repo
        .list_channel_members(ch_id)
        .await?;
    Ok(Json(members.into()))
}

#[utoipa::path(post, path = "/channels/{ch_id}/members", tag = "channels", request_body = AddChannelMemberRequest, responses((status = 200, body = ChannelMember)))]
async fn add_channel_member(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(ch_id): Path<Uuid>,
    Json(req): Json<AddChannelMemberRequest>,
) -> AppResult<Json<ChannelMember>> {
    let channel = authz::find_channel(&state, ch_id).await?;
    authz::require_workspace_role(
        &state,
        channel.workspace_id,
        auth.user_id,
        &WorkspaceRole::Member,
    )
    .await?;
    if !authz::can_moderate_channel(&state, &channel, auth.user_id).await
        && channel.channel_type != ChannelType::Public
    {
        state
            .workspace_service
            .repo
            .get_channel_member(ch_id, auth.user_id)
            .await?
            .ok_or_else(|| {
                AppError::Forbidden("Only members of this channel can add people to it".into())
            })?;
    }
    authz::require_workspace_member(&state, channel.workspace_id, req.user_id).await?;
    let member = state
        .workspace_service
        .repo
        .add_channel_member(ch_id, req.user_id, &ChannelRole::Member)
        .await?;

    audit::record(
        &state,
        AuditEntry::new(AuditAction::ChannelMemberAdded, auth.user_id)
            .workspace(channel.workspace_id)
            .resource(ch_id)
            .ip(&ip)
            .details(serde_json::json!({ "user_id": req.user_id })),
    )
    .await;

    Ok(Json(member))
}

#[utoipa::path(patch, path = "/channels/{ch_id}/members/{user_id}/role", tag = "channels", request_body = UpdateChannelMemberRoleRequest, responses((status = 200, body = ChannelMember)))]
async fn update_channel_member_role(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path((ch_id, user_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<UpdateChannelMemberRoleRequest>,
) -> AppResult<Json<ChannelMember>> {
    let channel = authz::find_channel(&state, ch_id).await?;
    authz::require_channel_moderator(&state, &channel, auth.user_id).await?;
    let target = state
        .workspace_service
        .repo
        .get_channel_member(ch_id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Not a member of this channel".into()))?;
    let member = state
        .workspace_service
        .repo
        .update_channel_member_role(ch_id, user_id, &req.role)
        .await?;

    audit::record(
        &state,
        AuditEntry::new(AuditAction::ChannelRoleChanged, auth.user_id)
            .workspace(channel.workspace_id)
            .resource(ch_id)
            .ip(&ip)
            .details(serde_json::json!({
                "user_id": user_id,
                "from": target.role,
                "to": req.role,
            })),
    )
    .await;

    Ok(Json(member))
}

#[utoipa::path(delete, path = "/channels/{ch_id}/members/{user_id}", tag = "channels", responses((status = 200, body = StatusResponse)))]
async fn remove_channel_member(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path((ch_id, user_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<StatusResponse>> {
    let channel = authz::find_channel(&state, ch_id).await?;
    if auth.user_id != user_id {
        authz::require_channel_moderator(&state, &channel, auth.user_id).await?;
    }
    let mut tx = state.pool.begin().await?;
    state
        .workspace_service
        .repo
        .remove_channel_member_in(&mut tx, ch_id, user_id)
        .await?;
    let staged = state
        .publisher
        .stage_channel_member_removed(&mut tx, ch_id, channel.workspace_id, user_id)
        .await?;
    tx.commit().await?;

    if let Err(e) = state
        .scheduled_repo
        .cancel_pending_for_channel(ch_id, user_id)
        .await
    {
        tracing::warn!(
            "failed to cancel scheduled messages for a left channel: {}",
            e
        );
    }

    state.publisher.dispatch(staged).await;

    audit::record(
        &state,
        AuditEntry::new(AuditAction::ChannelMemberRemoved, auth.user_id)
            .workspace(channel.workspace_id)
            .resource(ch_id)
            .ip(&ip)
            .details(serde_json::json!({
                "user_id": user_id,
                "self_service": auth.user_id == user_id,
            })),
    )
    .await;

    Ok(Json(StatusResponse::new("removed")))
}

#[utoipa::path(get, path = "/channels/{ch_id}/bookmarks", tag = "channels", responses((status = 200, body = DataList<ChannelBookmark>)))]
async fn list_channel_bookmarks(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ch_id): Path<Uuid>,
) -> AppResult<Json<DataList<ChannelBookmark>>> {
    authz::require_channel_access(&state, ch_id, auth.user_id).await?;
    let bookmarks = state
        .workspace_service
        .repo
        .list_channel_bookmarks(ch_id)
        .await?;
    Ok(Json(bookmarks.into()))
}

#[utoipa::path(post, path = "/channels/{ch_id}/bookmarks", tag = "channels", request_body = CreateChannelBookmarkRequest, responses((status = 200, body = ChannelBookmark)))]
async fn create_channel_bookmark(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ch_id): Path<Uuid>,
    Json(req): Json<CreateChannelBookmarkRequest>,
) -> AppResult<Json<ChannelBookmark>> {
    let access = authz::require_channel_access(&state, ch_id, auth.user_id).await?;
    authz::require_channel_moderator(&state, &access.channel, auth.user_id).await?;

    req.validate()?;
    let emoji = req
        .emoji
        .as_deref()
        .map(str::trim)
        .filter(|e| !e.is_empty());

    let bookmark = state
        .workspace_service
        .repo
        .create_channel_bookmark(ch_id, auth.user_id, req.label.trim(), req.url.trim(), emoji)
        .await?;

    Ok(Json(bookmark))
}

#[utoipa::path(delete, path = "/channels/{ch_id}/bookmarks/{bookmark_id}", tag = "channels", responses((status = 200, body = StatusResponse)))]
async fn delete_channel_bookmark(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((ch_id, bookmark_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<StatusResponse>> {
    let access = authz::require_channel_access(&state, ch_id, auth.user_id).await?;
    authz::require_channel_moderator(&state, &access.channel, auth.user_id).await?;

    let bookmark = state
        .workspace_service
        .repo
        .find_channel_bookmark(bookmark_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Bookmark not found".into()))?;
    if bookmark.channel_id != ch_id {
        return Err(AppError::NotFound("Bookmark not found".into()));
    }

    state
        .workspace_service
        .repo
        .delete_channel_bookmark(bookmark_id)
        .await?;

    Ok(Json(StatusResponse::new("deleted")))
}
