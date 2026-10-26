use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use crate::hooks::models::HookType;
use crate::state::AppState;
use crate::workspace::models::{
    Channel, ChannelMember, ChannelRole, ChannelType, PostPolicy, WhoMay, WorkspaceMember,
    WorkspaceRole,
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

    /// Who may post here. `everyone` is the default and what every channel that
    /// has never been configured returns; `moderators` is an announcement
    /// channel, where the people who can moderate are the people who can speak.
    ///
    /// Read off `channels.settings`, which the schema has carried since the
    /// first migration and nothing had ever used.
    pub fn post_policy(&self) -> PostPolicy {
        match self
            .channel
            .settings
            .get("post_policy")
            .and_then(|v| v.as_str())
        {
            Some("moderators") => PostPolicy::Moderators,
            _ => PostPolicy::Everyone,
        }
    }

    pub fn can_post(&self) -> bool {
        match self.post_policy() {
            PostPolicy::Everyone => true,
            PostPolicy::Moderators => self.can_moderate(),
        }
    }

    pub fn can_moderate(&self) -> bool {
        if self.member.role.has_at_least(&WorkspaceRole::Admin) {
            return true;
        }
        self.member.role.has_at_least(&WorkspaceRole::Member) && self.is_channel_admin()
    }

    pub fn allows_broadcast_mentions(&self) -> bool {
        self.channel
            .settings
            .get("allow_broadcast_mentions")
            .and_then(|v| v.as_bool())
            .unwrap_or(true)
    }

    pub fn broadcast_refusal(&self, rule: WhoMay) -> Option<&'static str> {
        if !self.allows_broadcast_mentions() {
            return Some("@channel, @here and @everyone are turned off in this channel");
        }
        match rule {
            WhoMay::Members => None,
            WhoMay::Admins
                if self.member.role.has_at_least(&WorkspaceRole::Admin)
                    || self.is_channel_admin() =>
            {
                None
            }
            WhoMay::Admins => Some(
                "Only workspace admins and this channel's admins can use @channel, @here and @everyone",
            ),
        }
    }

    pub fn can_manage_hook(&self, hook_type: &HookType) -> bool {
        if self.requires_explicit_membership() && self.channel_member.is_none() {
            return false;
        }
        match hook_type {
            HookType::IncomingWebhook => self.can_moderate(),
            HookType::OutgoingWebhook | HookType::SlashCommand => {
                self.member.role.has_at_least(&WorkspaceRole::Admin)
            }
        }
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

/// Access plus the right to speak. Every path that writes a message goes
/// through this rather than through `require_channel_access`, so an
/// announcement channel is enforced in one place instead of in each writer --
/// and the writers are the part that gets missed: the composer, thread replies,
/// scheduled sends and slash commands are four different files.
pub async fn require_channel_post(
    state: &AppState,
    channel_id: Uuid,
    user_id: Uuid,
) -> AppResult<ChannelAccess> {
    let access = require_channel_access(state, channel_id, user_id).await?;
    if !access.can_post() {
        return Err(AppError::Forbidden(
            "Only moderators can post in this channel".into(),
        ));
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

async fn access_to(
    state: &AppState,
    channel: &Channel,
    user_id: Uuid,
) -> AppResult<Option<ChannelAccess>> {
    let repo = &state.workspace_service.repo;
    let Some(member) = repo.get_member(channel.workspace_id, user_id).await? else {
        return Ok(None);
    };
    let channel_member = repo.get_channel_member(channel.id, user_id).await?;
    Ok(Some(ChannelAccess {
        channel: channel.clone(),
        member,
        channel_member,
    }))
}

async fn workspace_rule(state: &AppState, workspace_id: Uuid, key: &str) -> AppResult<WhoMay> {
    let workspace = state
        .workspace_service
        .repo
        .find_workspace_by_id(workspace_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Workspace not found".into()))?;
    Ok(WhoMay::of(&workspace.settings, key))
}

pub async fn can_manage_channel_hook(
    state: &AppState,
    hook_type: &HookType,
    channel: &Channel,
    user_id: Uuid,
) -> AppResult<bool> {
    Ok(access_to(state, channel, user_id)
        .await?
        .is_some_and(|access| access.can_manage_hook(hook_type)))
}

async fn broadcast_refusal(
    state: &AppState,
    channel: &Channel,
    user_id: Uuid,
) -> AppResult<Option<&'static str>> {
    let Some(access) = access_to(state, channel, user_id).await? else {
        return Ok(Some("Not a member of this workspace"));
    };
    let rule = workspace_rule(state, channel.workspace_id, WhoMay::BROADCAST_MENTIONS).await?;
    Ok(access.broadcast_refusal(rule))
}

pub async fn can_broadcast(state: &AppState, channel: &Channel, user_id: Uuid) -> AppResult<bool> {
    Ok(broadcast_refusal(state, channel, user_id).await?.is_none())
}

pub async fn require_broadcast(
    state: &AppState,
    channel: &Channel,
    user_id: Uuid,
) -> AppResult<()> {
    match broadcast_refusal(state, channel, user_id).await? {
        Some(reason) => Err(AppError::Forbidden(reason.into())),
        None => Ok(()),
    }
}

pub async fn require_channel_creator(
    state: &AppState,
    workspace_id: Uuid,
    user_id: Uuid,
    channel_type: &ChannelType,
) -> AppResult<()> {
    let member = require_workspace_member(state, workspace_id, user_id).await?;
    let (key, kind) = match channel_type {
        ChannelType::Private => (WhoMay::PRIVATE_CHANNEL_CREATORS, "private"),
        _ => (WhoMay::PUBLIC_CHANNEL_CREATORS, "public"),
    };
    let rule = workspace_rule(state, workspace_id, key).await?;
    if rule.allows(&member.role) {
        return Ok(());
    }
    Err(AppError::Forbidden(match rule {
        WhoMay::Admins if member.role != WorkspaceRole::Guest => {
            format!("Only admins can create {kind} channels in this workspace")
        }
        _ => format!("Requires at least {:?} role", WorkspaceRole::Member),
    }))
}

pub async fn require_channel_hook_manager(
    state: &AppState,
    hook_type: &HookType,
    channel: &Channel,
    user_id: Uuid,
) -> AppResult<()> {
    if can_manage_channel_hook(state, hook_type, channel, user_id).await? {
        return Ok(());
    }
    Err(AppError::Forbidden(match hook_type {
        HookType::IncomingWebhook => {
            "Requires channel admin or workspace admin, in the channel if it is private".into()
        }
        HookType::OutgoingWebhook | HookType::SlashCommand => {
            "Requires workspace admin, in the channel if it is private".into()
        }
    }))
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

    #[test]
    fn channel_admins_manage_only_the_incoming_webhooks_of_their_channel() {
        let a = access(WorkspaceRole::Member, ChannelType::Private, true);
        assert!(a.can_manage_hook(&HookType::IncomingWebhook));
        assert!(!a.can_manage_hook(&HookType::OutgoingWebhook));
        assert!(!a.can_manage_hook(&HookType::SlashCommand));
    }

    #[test]
    fn workspace_admins_manage_every_hook_of_a_channel_they_can_see() {
        let a = access(WorkspaceRole::Admin, ChannelType::Public, false);
        for hook_type in [
            HookType::IncomingWebhook,
            HookType::OutgoingWebhook,
            HookType::SlashCommand,
        ] {
            assert!(a.can_manage_hook(&hook_type));
        }
        let outside = access(WorkspaceRole::Admin, ChannelType::Private, false);
        assert!(!outside.can_manage_hook(&HookType::IncomingWebhook));
        assert!(!outside.can_manage_hook(&HookType::OutgoingWebhook));
    }

    fn with_settings(mut a: ChannelAccess, settings: serde_json::Value) -> ChannelAccess {
        a.channel.settings = settings;
        a
    }

    #[test]
    fn by_default_everyone_who_may_post_may_broadcast() {
        for role in [
            WorkspaceRole::Guest,
            WorkspaceRole::Member,
            WorkspaceRole::Admin,
        ] {
            let a = access(role, ChannelType::Public, false);
            assert_eq!(a.broadcast_refusal(WhoMay::Members), None);
        }
    }

    #[test]
    fn an_admins_rule_leaves_workspace_and_channel_admins() {
        assert_eq!(
            access(WorkspaceRole::Admin, ChannelType::Public, false)
                .broadcast_refusal(WhoMay::Admins),
            None
        );
        assert_eq!(
            access(WorkspaceRole::Member, ChannelType::Public, true)
                .broadcast_refusal(WhoMay::Admins),
            None
        );
        for role in [WorkspaceRole::Guest, WorkspaceRole::Member] {
            assert!(access(role, ChannelType::Public, false)
                .broadcast_refusal(WhoMay::Admins)
                .is_some());
        }
    }

    #[test]
    fn a_channel_that_turned_broadcasts_off_refuses_everyone() {
        let off = serde_json::json!({ "allow_broadcast_mentions": false });
        for (role, channel_admin) in [(WorkspaceRole::Owner, true), (WorkspaceRole::Member, false)]
        {
            let a = with_settings(
                access(role, ChannelType::Public, channel_admin),
                off.clone(),
            );
            assert!(a.broadcast_refusal(WhoMay::Members).is_some());
        }
    }

    #[test]
    fn plain_members_manage_no_hook() {
        let a = access(WorkspaceRole::Member, ChannelType::Public, false);
        assert!(!a.can_manage_hook(&HookType::IncomingWebhook));
        assert!(!a.can_manage_hook(&HookType::OutgoingWebhook));
    }
}
