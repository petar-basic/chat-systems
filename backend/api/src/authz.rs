use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use crate::conversations::models::Conversation;
use crate::state::AppState;
use crate::workspace::models::{
    Channel, ChannelMember, ChannelRole, ChannelType, WorkspaceMember, WorkspaceRole,
};

pub struct ChannelAccess {
    pub channel: Channel,
    pub member: WorkspaceMember,
    pub channel_member: Option<ChannelMember>,
}

impl ChannelAccess {
    pub fn is_guest(&self) -> bool {
        self.member.role == WorkspaceRole::Guest
    }

    pub fn requires_explicit_membership(&self) -> bool {
        self.is_guest() || !matches!(self.channel.channel_type, ChannelType::Public)
    }

    pub fn is_channel_admin(&self) -> bool {
        matches!(&self.channel_member, Some(cm) if cm.role == ChannelRole::Admin)
    }

    pub fn can_moderate(&self) -> bool {
        if self.member.role.has_at_least(&WorkspaceRole::Admin) {
            return true;
        }
        self.member.role.has_at_least(&WorkspaceRole::Member) && self.is_channel_admin()
    }
}

pub async fn require_workspace_member(
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

pub async fn require_workspace_role(
    state: &AppState,
    workspace_id: Uuid,
    user_id: Uuid,
    minimum: &WorkspaceRole,
) -> AppResult<WorkspaceMember> {
    let member = require_workspace_member(state, workspace_id, user_id).await?;
    if !member.role.has_at_least(minimum) {
        return Err(AppError::Forbidden(format!(
            "Requires at least {minimum:?} role"
        )));
    }
    Ok(member)
}

pub fn outranks(actor: &WorkspaceRole, target: &WorkspaceRole) -> bool {
    actor.level() > target.level()
}

pub async fn find_channel(state: &AppState, channel_id: Uuid) -> AppResult<Channel> {
    state
        .workspace_service
        .repo
        .find_channel_by_id(channel_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Channel not found".into()))
}

pub async fn require_channel_access(
    state: &AppState,
    channel_id: Uuid,
    user_id: Uuid,
) -> AppResult<ChannelAccess> {
    let channel = find_channel(state, channel_id).await?;
    let member = require_workspace_member(state, channel.workspace_id, user_id).await?;
    let channel_member = state
        .workspace_service
        .repo
        .get_channel_member(channel_id, user_id)
        .await?;

    let access = ChannelAccess {
        channel,
        member,
        channel_member,
    };

    if access.requires_explicit_membership() && access.channel_member.is_none() {
        return Err(AppError::Forbidden("Not a member of this channel".into()));
    }

    Ok(access)
}

pub async fn can_moderate_channel(state: &AppState, channel: &Channel, user_id: Uuid) -> bool {
    let member = match state
        .workspace_service
        .repo
        .get_member(channel.workspace_id, user_id)
        .await
    {
        Ok(Some(member)) => member,
        _ => return false,
    };

    let channel_member = match state
        .workspace_service
        .repo
        .get_channel_member(channel.id, user_id)
        .await
    {
        Ok(channel_member) => channel_member,
        Err(_) => return false,
    };

    ChannelAccess {
        channel: channel.clone(),
        member,
        channel_member,
    }
    .can_moderate()
}

pub async fn require_channel_moderator(
    state: &AppState,
    channel: &Channel,
    user_id: Uuid,
) -> AppResult<()> {
    if can_moderate_channel(state, channel, user_id).await {
        return Ok(());
    }
    Err(AppError::Forbidden(
        "Requires channel admin or workspace admin".into(),
    ))
}

pub async fn require_conversation_participant(
    state: &AppState,
    conversation_id: Uuid,
    user_id: Uuid,
) -> AppResult<Conversation> {
    let conversation = state
        .conversation_repo
        .find_by_id(conversation_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Conversation not found".into()))?;

    if !state
        .conversation_repo
        .is_participant(conversation_id, user_id)
        .await?
    {
        return Err(AppError::Forbidden(
            "Not a participant in this conversation".into(),
        ));
    }

    Ok(conversation)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn access(
        role: WorkspaceRole,
        channel_type: ChannelType,
        channel_admin: bool,
    ) -> ChannelAccess {
        let workspace_id = Uuid::new_v4();
        let channel_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        ChannelAccess {
            channel: Channel {
                id: channel_id,
                workspace_id,
                name: Some("general".into()),
                channel_type,
                topic: None,
                description: None,
                created_by: Some(user_id),
                is_default: false,
                is_archived: false,
                settings: serde_json::json!({}),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            },
            member: WorkspaceMember {
                workspace_id,
                user_id,
                role,
                joined_at: chrono::Utc::now(),
            },
            channel_member: channel_admin.then(|| ChannelMember {
                channel_id,
                user_id,
                role: ChannelRole::Admin,
                last_read_at: None,
                last_read_msg: None,
                notifications: "default".into(),
                is_muted: false,
                is_starred: false,
                joined_at: chrono::Utc::now(),
            }),
        }
    }

    #[test]
    fn public_channels_need_no_explicit_membership_for_members() {
        let a = access(WorkspaceRole::Member, ChannelType::Public, false);
        assert!(!a.requires_explicit_membership());
    }

    #[test]
    fn guests_always_need_explicit_membership() {
        let a = access(WorkspaceRole::Guest, ChannelType::Public, false);
        assert!(a.requires_explicit_membership());
    }

    #[test]
    fn non_public_channels_always_need_explicit_membership() {
        for channel_type in [ChannelType::Private, ChannelType::Dm, ChannelType::GroupDm] {
            let a = access(WorkspaceRole::Admin, channel_type, false);
            assert!(a.requires_explicit_membership());
        }
    }

    #[test]
    fn workspace_admins_moderate_without_channel_membership() {
        let a = access(WorkspaceRole::Admin, ChannelType::Public, false);
        assert!(a.can_moderate());
    }

    #[test]
    fn channel_admins_moderate_their_own_channel() {
        let a = access(WorkspaceRole::Member, ChannelType::Public, true);
        assert!(a.can_moderate());
    }

    #[test]
    fn plain_members_and_guests_do_not_moderate() {
        assert!(!access(WorkspaceRole::Member, ChannelType::Public, false).can_moderate());
        assert!(!access(WorkspaceRole::Guest, ChannelType::Public, true).can_moderate());
    }
}
