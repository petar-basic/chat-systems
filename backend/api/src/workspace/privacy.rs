use serde_json::json;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::{Channel, ChannelPrivacy, ChannelType, WorkspaceRole};
use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::authz;
use crate::messaging::publisher::EventPublisher;
use crate::state::AppState;

pub async fn require_changeable(
    state: &AppState,
    channel_id: Uuid,
    user_id: Uuid,
) -> AppResult<Channel> {
    let access = authz::require_channel_access(state, channel_id, user_id).await?;
    if !access.member.role.has_at_least(&WorkspaceRole::Admin) {
        return Err(AppError::Forbidden(
            "Only workspace admins can make a channel private or public".into(),
        ));
    }
    let channel = access.channel;
    authz::require_not_conversation(&channel)?;
    authz::require_not_archived(&channel)?;
    if channel.is_default {
        return Err(AppError::BadRequest(
            "Everyone joins this channel when they arrive, so it stays public".into(),
        ));
    }
    Ok(channel)
}

pub async fn preview(state: &AppState, channel: &Channel) -> AppResult<ChannelPrivacy> {
    let (messages, files) = state
        .workspace_service
        .repo
        .channel_contents(channel.id)
        .await?;
    Ok(ChannelPrivacy {
        private: channel.channel_type == ChannelType::Private,
        messages,
        files,
    })
}

pub async fn set(
    state: &AppState,
    channel: &Channel,
    actor: Uuid,
    ip: &ClientIp,
    private: bool,
) -> AppResult<Channel> {
    let (from, to, text) = if private {
        (
            ChannelType::Public,
            ChannelType::Private,
            "made this channel private",
        )
    } else {
        (
            ChannelType::Private,
            ChannelType::Public,
            "made this channel public",
        )
    };

    let mut tx = state.pool.begin().await?;
    let Some(updated) = state
        .workspace_service
        .repo
        .set_channel_type(&mut tx, channel.id, from.clone(), to.clone())
        .await?
    else {
        return Ok(channel.clone());
    };
    let msg = state
        .message_repo
        .create_system_message_in(
            &mut tx,
            channel.id,
            actor,
            text,
            json!({ "kind": "channel_privacy_changed", "private": private }),
        )
        .await?;
    let staged = vec![
        state
            .publisher
            .stage_message_created(&mut tx, &msg, channel.workspace_id, &[])
            .await?,
        EventPublisher::stage_channel_updated(&mut tx, &updated).await?,
        EventPublisher::stage_channel_privacy_changed(&mut tx, &updated).await?,
    ];
    tx.commit().await?;
    state.publisher.dispatch_all(staged).await;

    audit::record(
        state,
        AuditEntry::new(AuditAction::ChannelPrivacyChanged, actor)
            .workspace(channel.workspace_id)
            .resource(channel.id)
            .ip(ip)
            .details(json!({
                "name": channel.name,
                "from": from,
                "to": to,
            })),
    )
    .await;
    Ok(updated)
}
