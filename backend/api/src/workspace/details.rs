use serde_json::json;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::{Channel, PostPolicy};
use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::messaging::publisher::EventPublisher;
use crate::state::AppState;

#[derive(Default)]
pub struct Details<'a> {
    pub name: Option<&'a str>,
    pub topic: Option<&'a str>,
    pub description: Option<&'a str>,
    pub post_policy: Option<PostPolicy>,
    pub allow_broadcast_mentions: Option<bool>,
}

impl Details<'_> {
    fn touches_about(&self) -> bool {
        self.name.is_some() || self.topic.is_some() || self.description.is_some()
    }
}

pub async fn update(
    state: &AppState,
    channel: &Channel,
    actor: Uuid,
    ip: &ClientIp,
    details: Details<'_>,
) -> AppResult<Channel> {
    let repo = &state.workspace_service.repo;
    let mut tx = state.pool.begin().await?;
    let mut updated = channel.clone();
    if details.touches_about() {
        updated = repo
            .update_channel(
                &mut tx,
                channel.id,
                details.name,
                details.topic,
                details.description,
            )
            .await
            .map_err(duplicate_channel_name)?;
    }
    if let Some(policy) = details.post_policy {
        updated = repo
            .merge_channel_settings(
                &mut tx,
                channel.id,
                &json!({ "post_policy": policy.as_str() }),
            )
            .await?;
    }
    if let Some(allowed) = details.allow_broadcast_mentions {
        updated = repo
            .merge_channel_settings(
                &mut tx,
                channel.id,
                &json!({ "allow_broadcast_mentions": allowed }),
            )
            .await?;
    }

    let mut staged = Vec::new();
    for (text, metadata) in announcements(channel, &updated) {
        let msg = state
            .message_repo
            .create_system_message_in(&mut tx, channel.id, actor, &text, metadata)
            .await?;
        staged.push(
            state
                .publisher
                .stage_message_created(&mut tx, &msg, channel.workspace_id, &[])
                .await?,
        );
    }
    staged.push(EventPublisher::stage_channel_updated(&mut tx, &updated).await?);
    tx.commit().await?;
    for event in staged {
        state.publisher.dispatch(event).await;
    }

    let entry = |action| {
        AuditEntry::new(action, actor)
            .workspace(channel.workspace_id)
            .resource(channel.id)
            .ip(ip)
    };
    if details.touches_about() {
        audit::record(
            state,
            entry(AuditAction::ChannelUpdated)
                .details(json!({ "from": channel.name, "to": updated.name })),
        )
        .await;
    }
    if let Some(policy) = details.post_policy {
        audit::record(
            state,
            entry(AuditAction::ChannelPostPolicyChanged).details(json!({
                "from": post_policy(channel),
                "to": policy.as_str(),
            })),
        )
        .await;
    }
    if let Some(allowed) = details.allow_broadcast_mentions {
        audit::record(
            state,
            entry(AuditAction::ChannelBroadcastMentionsChanged)
                .details(json!({ "allowed": allowed })),
        )
        .await;
    }

    Ok(updated)
}

fn post_policy(channel: &Channel) -> &str {
    channel
        .settings
        .get("post_policy")
        .and_then(|v| v.as_str())
        .unwrap_or("everyone")
}

fn announcements(before: &Channel, after: &Channel) -> Vec<(String, serde_json::Value)> {
    let mut said = Vec::new();
    if let (Some(from), Some(to)) = (&before.name, &after.name) {
        if from != to {
            said.push((
                format!("renamed the channel from “{from}” to “{to}”"),
                json!({ "kind": "channel_renamed", "from": from, "to": to }),
            ));
        }
    }
    if let Some(topic) = changed(before.topic.as_deref(), after.topic.as_deref()) {
        said.push((
            match topic {
                Some(topic) => format!("set the channel topic: {topic}"),
                None => "cleared the channel topic".into(),
            },
            json!({ "kind": "channel_topic_changed", "topic": topic }),
        ));
    }
    if let Some(description) = changed(before.description.as_deref(), after.description.as_deref())
    {
        said.push((
            match description {
                Some(description) => format!("set the channel description: {description}"),
                None => "cleared the channel description".into(),
            },
            json!({ "kind": "channel_description_changed", "description": description }),
        ));
    }
    said
}

fn changed<'a>(before: Option<&str>, after: Option<&'a str>) -> Option<Option<&'a str>> {
    let filled = |text: &str| !text.trim().is_empty();
    let after = after.filter(|text| filled(text));
    (before.filter(|text| filled(text)) != after).then_some(after)
}

pub fn duplicate_channel_name(err: sqlx::Error) -> AppError {
    if shared_common::errors::is_unique_violation(&err) {
        AppError::Conflict("A channel with that name already exists".into())
    } else {
        err.into()
    }
}
