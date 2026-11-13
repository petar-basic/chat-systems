use serde_json::json;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::authz;
use crate::messaging::publisher::EventPublisher;
use crate::state::AppState;
use crate::workspace::details::duplicate_channel_name;
use crate::workspace::models::{Channel, ChannelType};

const SMALLEST_GROUP: usize = 3;

pub struct Convertible {
    pub conversation: Channel,
    pub participants: Vec<Uuid>,
}

pub async fn require_convertible(
    state: &AppState,
    conversation_id: Uuid,
    user_id: Uuid,
) -> AppResult<Convertible> {
    let conversation = authz::require_channel_access(state, conversation_id, user_id)
        .await?
        .channel;
    let participants = state
        .conversation_repo
        .participant_ids(conversation.id)
        .await?;
    if conversation.channel_type != ChannelType::GroupDm || participants.len() < SMALLEST_GROUP {
        return Err(AppError::BadRequest(
            "Only a group DM of three or more people becomes a channel".into(),
        ));
    }
    authz::require_channel_creator(
        state,
        conversation.workspace_id,
        user_id,
        &ChannelType::Private,
    )
    .await?;
    Ok(Convertible {
        conversation,
        participants,
    })
}

pub async fn convert(
    state: &AppState,
    convertible: &Convertible,
    actor: Uuid,
    ip: &ClientIp,
    name: &str,
) -> AppResult<Channel> {
    let conversation = &convertible.conversation;
    let workspace_id = conversation.workspace_id;

    let mut tx = state.pool.begin().await?;
    crate::guests::service::require_room_in(
        &mut tx,
        workspace_id,
        &convertible.participants,
        conversation.id,
    )
    .await?;
    let channel = state
        .conversation_repo
        .convert_in(&mut tx, conversation.id, name, actor)
        .await
        .map_err(duplicate_channel_name)?
        .ok_or_else(|| AppError::Conflict("This conversation is already a channel".into()))?;
    let msg = state
        .message_repo
        .create_system_message_in(
            &mut tx,
            channel.id,
            actor,
            "converted this conversation into a private channel",
            json!({ "kind": "converted_to_channel", "name": name }),
        )
        .await?;
    let staged = vec![
        state
            .publisher
            .stage_message_created(&mut tx, &msg, workspace_id, &[])
            .await?,
        EventPublisher::stage_channel_updated(&mut tx, &channel).await?,
        EventPublisher::stage(
            &mut tx,
            "conversation.converted",
            workspace_id,
            json!({
                "conversation_id": channel.id,
                "workspace_id": workspace_id,
                "participant_ids": convertible.participants,
                "channel": channel,
            }),
        )
        .await?,
    ];
    tx.commit().await?;
    state.publisher.dispatch_all(staged).await;

    audit::record(
        state,
        AuditEntry::new(AuditAction::ConversationConverted, actor)
            .workspace(workspace_id)
            .resource(channel.id)
            .ip(ip)
            .details(json!({
                "name": name,
                "participants": convertible.participants.len(),
            })),
    )
    .await;
    Ok(channel)
}
