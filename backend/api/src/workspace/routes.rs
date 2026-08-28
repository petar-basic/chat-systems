use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};
use shared_common::validation;

use super::models::*;
use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::authz;
use crate::middleware::AuthUser;
use crate::state::AppState;

pub fn router(state: Arc<AppState>) -> Router {
    let routes = Router::new()
        .route("/workspaces", get(list_workspaces))
        .route("/workspaces", post(create_workspace))
        .route("/workspaces/deleted", get(list_deleted_workspaces))
        .route("/workspaces/{ws_id}", get(get_workspace))
        .route("/workspaces/{ws_id}", patch(update_workspace))
        .route("/workspaces/{ws_id}", delete(delete_workspace))
        .route("/workspaces/{ws_id}/restore", post(restore_workspace))
        .route("/workspaces/{ws_id}/audit-log", get(list_audit_log))
        .route("/workspaces/{ws_id}/members", get(list_members))
        .route(
            "/workspaces/{ws_id}/members/{user_id}/role",
            patch(update_member_role),
        )
        .route(
            "/workspaces/{ws_id}/members/{user_id}",
            delete(remove_member),
        )
        .route("/workspaces/{ws_id}/invites", get(list_invites))
        .route("/workspaces/{ws_id}/invites", post(create_invite))
        .route(
            "/workspaces/{ws_id}/invites/{invite_id}",
            delete(revoke_invite),
        )
        .route("/invites/{token}/accept", post(accept_invite))
        .route("/workspaces/{ws_id}/channels", get(list_channels))
        .route("/workspaces/{ws_id}/channels/unread", get(unread_channels))
        .route("/workspaces/{ws_id}/channels/browse", get(browse_channels))
        .route("/workspaces/{ws_id}/channels", post(create_channel))
        .route("/channels/{ch_id}", get(get_channel))
        .route("/channels/{ch_id}", patch(update_channel))
        .route("/channels/{ch_id}", delete(archive_channel))
        .route(
            "/channels/{ch_id}/notifications",
            patch(set_channel_notifications),
        )
        .route("/channels/{ch_id}/members", get(list_channel_members))
        .route("/channels/{ch_id}/members", post(add_channel_member))
        .route("/channels/{ch_id}/join", post(join_channel))
        .route(
            "/channels/{ch_id}/members/{user_id}/role",
            patch(update_channel_member_role),
        )
        .route(
            "/channels/{ch_id}/members/{user_id}",
            delete(remove_channel_member),
        )
        .route("/channels/{ch_id}/bookmarks", get(list_channel_bookmarks))
        .route("/channels/{ch_id}/bookmarks", post(create_channel_bookmark))
        .route(
            "/channels/{ch_id}/bookmarks/{bookmark_id}",
            delete(delete_channel_bookmark),
        );

    crate::protected(state, routes)
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
    ip: ClientIp,
    Json(req): Json<CreateWorkspaceRequest>,
) -> AppResult<Json<Workspace>> {
    validation::validate_workspace_name(&req.name)?;
    if let Some(description) = &req.description {
        validation::validate_description(description)?;
    }
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

async fn update_workspace(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<UpdateWorkspaceRequest>,
) -> AppResult<Json<Workspace>> {
    if let Some(name) = &req.name {
        validation::validate_workspace_name(name)?;
    }
    if let Some(description) = &req.description {
        validation::validate_description(description)?;
    }
    if let Some(icon_url) = &req.icon_url {
        // An empty string is how the client says "remove it"; anything else has
        // to be a URL.
        if !icon_url.is_empty() {
            validation::validate_icon_url(icon_url)?;
        }
    }
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

async fn delete_workspace(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(ws_id): Path<Uuid>,
    Query(params): Query<DeleteWorkspaceRequest>,
) -> AppResult<Json<serde_json::Value>> {
    if !auth.is_instance_admin {
        authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Owner).await?;
    }
    let hard = params.hard.unwrap_or(false);
    let entry = AuditEntry::new(AuditAction::WorkspaceDeleted, auth.user_id)
        .workspace(ws_id)
        .resource(ws_id)
        .ip(&ip)
        .details(serde_json::json!({ "hard": hard }));

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
        audit::record(&state, entry).await;
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
        audit::record(&state, entry).await;
        Ok(Json(serde_json::json!({ "status": "soft_deleted" })))
    }
}

async fn restore_workspace(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<Workspace>> {
    if !auth.is_instance_admin {
        authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Owner).await?;
    }
    let workspace = state
        .workspace_service
        .repo
        .restore_workspace(ws_id)
        .await?;
    let _ = state.publisher.publish_workspace_restored(ws_id).await;

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

async fn list_audit_log(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Query(query): Query<audit::AuditQuery>,
) -> AppResult<Json<serde_json::Value>> {
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
    let entries = audit::list(&state, Some(ws_id), &query).await?;
    Ok(Json(serde_json::json!({ "data": entries })))
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

    Ok(Json(serde_json::json!({ "data": members })))
}

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

async fn remove_member(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path((ws_id, user_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
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

    Ok(Json(serde_json::json!({ "status": "removed" })))
}

async fn list_invites(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
    let invites = state.workspace_service.repo.list_invites(ws_id).await?;
    Ok(Json(serde_json::json!({ "data": invites })))
}

async fn create_invite(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<CreateInviteRequest>,
) -> AppResult<Json<WorkspaceInvite>> {
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
    if let Some(email) = &req.email {
        validation::validate_email(email)?;
    }
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

async fn revoke_invite(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path((ws_id, invite_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
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

    Ok(Json(serde_json::json!({ "status": "revoked" })))
}

async fn list_channels(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
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
    let counts: Vec<serde_json::Value> = counts
        .into_iter()
        .map(|(channel_id, unread, mentions, last_read_msg)| {
            serde_json::json!({
                "channel_id": channel_id,
                "unread_count": unread,
                "mention_count": mentions,
                "last_read_msg": last_read_msg,
            })
        })
        .collect();

    Ok(Json(
        serde_json::json!({ "channel_ids": channel_ids, "counts": counts }),
    ))
}

fn duplicate_channel_name(err: sqlx::Error) -> AppError {
    if shared_common::errors::is_unique_violation(&err) {
        AppError::Conflict("A channel with that name already exists".into())
    } else {
        err.into()
    }
}

async fn create_channel(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<CreateChannelRequest>,
) -> AppResult<Json<Channel>> {
    validation::validate_channel_name(&req.name)?;
    if let Some(description) = &req.description {
        validation::validate_description(description)?;
    }
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
    authz::require_workspace_member(&state, channel.workspace_id, auth.user_id).await?;
    Ok(Json(channel))
}

async fn update_channel(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(ch_id): Path<Uuid>,
    Json(req): Json<UpdateChannelRequest>,
) -> AppResult<Json<Channel>> {
    if let Some(name) = &req.name {
        validation::validate_channel_name(name)?;
    }
    if let Some(topic) = &req.topic {
        validation::validate_channel_topic(topic)?;
    }
    if let Some(description) = &req.description {
        validation::validate_description(description)?;
    }
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

    if let Some(requested) = &req.post_policy {
        let policy = authz::PostPolicy::parse(requested)?;
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

async fn browse_channels(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Member).await?;
    let channels = state
        .workspace_service
        .repo
        .list_browsable_channels(ws_id, auth.user_id)
        .await?;
    Ok(Json(serde_json::json!({ "data": channels })))
}

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

async fn archive_channel(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(ch_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
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
    authz::require_workspace_member(&state, channel.workspace_id, auth.user_id).await?;
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

async fn remove_channel_member(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path((ch_id, user_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
    let channel = authz::find_channel(&state, ch_id).await?;
    if auth.user_id != user_id {
        authz::require_channel_moderator(&state, &channel, auth.user_id).await?;
    }
    state
        .workspace_service
        .repo
        .remove_channel_member(ch_id, user_id)
        .await?;

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

    let _ = state
        .publisher
        .publish_channel_member_removed(ch_id, channel.workspace_id, user_id)
        .await;

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

    Ok(Json(serde_json::json!({ "status": "removed" })))
}

async fn list_channel_bookmarks(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ch_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    authz::require_channel_access(&state, ch_id, auth.user_id).await?;
    let bookmarks = state
        .workspace_service
        .repo
        .list_channel_bookmarks(ch_id)
        .await?;
    Ok(Json(serde_json::json!({ "data": bookmarks })))
}

async fn create_channel_bookmark(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ch_id): Path<Uuid>,
    Json(req): Json<CreateChannelBookmarkRequest>,
) -> AppResult<Json<ChannelBookmark>> {
    let access = authz::require_channel_access(&state, ch_id, auth.user_id).await?;
    authz::require_channel_moderator(&state, &access.channel, auth.user_id).await?;

    validation::validate_bookmark_label(&req.label)?;
    validation::validate_bookmark_url(&req.url)?;
    let emoji = req
        .emoji
        .as_deref()
        .map(str::trim)
        .filter(|e| !e.is_empty());
    if let Some(emoji) = emoji {
        validation::validate_status_emoji(emoji)?;
    }

    let bookmark = state
        .workspace_service
        .repo
        .create_channel_bookmark(ch_id, auth.user_id, req.label.trim(), req.url.trim(), emoji)
        .await?;

    Ok(Json(bookmark))
}

async fn delete_channel_bookmark(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((ch_id, bookmark_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
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

    Ok(Json(serde_json::json!({ "status": "deleted" })))
}
