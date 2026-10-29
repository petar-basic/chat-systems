pub mod consumer;
pub mod repo;
pub mod routes;
pub mod sender;

use sha2::{Digest, Sha256};
use uuid::Uuid;

use shared_events::Event;

use shared_common::errors::{AppError, AppResult};

use crate::bots::models::{AppEvent, Scope};
use crate::commands::routes::CommandResponse;
use crate::state::AppState;

pub const EVENT_TTL_SECS: f64 = 60.0 * 60.0;
pub const COMMAND_TTL_SECS: f64 = 60.0;
pub const KIND_EVENT: &str = "event";
pub const KIND_SLASH_COMMAND: &str = "slash_command";
pub const APP_TOKEN_PREFIX: &str = "capp-";

pub struct Translated {
    pub workspace_id: Uuid,
    pub channel_id: Uuid,
    pub actor: Option<Uuid>,
    pub text: Option<String>,
    pub events: Vec<(AppEvent, serde_json::Value)>,
}

fn uuid_at(payload: &serde_json::Value, field: &str) -> Option<Uuid> {
    payload.get(field)?.as_str()?.parse().ok()
}

pub fn workspace_of_stream(key: &str) -> Option<Uuid> {
    key.strip_prefix("stream:ws:")?.parse().ok()
}

pub fn translate(workspace_id: Uuid, event: &Event) -> Option<Translated> {
    let p = &event.payload;
    let channel_id = uuid_at(p, "channel_id")?;
    let (actor, text, events) = match event.event_type.as_str() {
        "message.created" => {
            if p.pointer("/metadata/kind").is_some() {
                return None;
            }
            let body = serde_json::json!({
                "channel_id": channel_id,
                "message_id": p.get("id"),
                "user_id": p.get("user_id"),
                "text": p.get("content"),
                "thread_parent_id": p.get("thread_parent_id"),
                "created_at": p.get("created_at"),
            });
            (
                uuid_at(p, "user_id"),
                p.get("content")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                vec![
                    (AppEvent::Message, body.clone()),
                    (AppEvent::AppMention, body),
                ],
            )
        }
        kind @ ("reaction.added" | "reaction.removed") => {
            let app_event = if kind == "reaction.added" {
                AppEvent::ReactionAdded
            } else {
                AppEvent::ReactionRemoved
            };
            let body = serde_json::json!({
                "channel_id": channel_id,
                "message_id": p.get("message_id"),
                "user_id": p.get("user_id"),
                "emoji": p.get("emoji"),
            });
            (uuid_at(p, "user_id"), None, vec![(app_event, body)])
        }
        "channel.member_joined" => {
            let body = serde_json::json!({
                "channel_id": channel_id,
                "user_id": p.get("user_id"),
            });
            (None, None, vec![(AppEvent::MemberJoinedChannel, body)])
        }
        "channel.created" => {
            if p.get("channel_type").and_then(|v| v.as_str()) != Some("public") {
                return None;
            }
            let body = serde_json::json!({
                "channel_id": channel_id,
                "name": p.get("name"),
                "created_by": p.get("created_by"),
            });
            (None, None, vec![(AppEvent::ChannelCreated, body)])
        }
        "channel.updated" => {
            if p.get("is_archived").and_then(|v| v.as_bool()) != Some(true) {
                return None;
            }
            let body = serde_json::json!({ "channel_id": channel_id });
            (None, None, vec![(AppEvent::ChannelArchived, body)])
        }
        _ => return None,
    };
    Some(Translated {
        workspace_id,
        channel_id,
        actor,
        text,
        events,
    })
}

pub fn required_scope(event: AppEvent, channel_type: &str) -> Scope {
    match event {
        AppEvent::Message
        | AppEvent::AppMention
        | AppEvent::ReactionAdded
        | AppEvent::ReactionRemoved => {
            if channel_type == "public" {
                Scope::ChannelsHistory
            } else {
                Scope::GroupsHistory
            }
        }
        AppEvent::MemberJoinedChannel | AppEvent::ChannelCreated | AppEvent::ChannelArchived => {
            Scope::ChannelsRead
        }
    }
}

pub fn hears(subscriber: &repo::Subscriber, translated: &Translated, event: AppEvent) -> bool {
    if !subscriber.events.iter().any(|e| e == event.as_str()) {
        return false;
    }
    let scope = required_scope(event, &subscriber.channel_type);
    if !subscriber.scopes.iter().any(|s| s == scope.as_str()) {
        return false;
    }
    let in_channel = subscriber.is_member
        || (event == AppEvent::ChannelCreated && subscriber.channel_type == "public");
    if !in_channel {
        return false;
    }
    match event {
        AppEvent::Message | AppEvent::ReactionAdded | AppEvent::ReactionRemoved => {
            translated.actor != Some(subscriber.bot_user_id)
        }
        AppEvent::AppMention => {
            translated.actor != Some(subscriber.bot_user_id)
                && translated
                    .text
                    .as_deref()
                    .is_some_and(|text| text.contains(&format!("]({})", subscriber.bot_user_id)))
        }
        _ => true,
    }
}

pub fn event_id(source: Uuid, event: AppEvent) -> Uuid {
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    hasher.update(event.as_str().as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    uuid::Builder::from_custom_bytes(bytes).into_uuid()
}

pub fn envelope(
    source: &Event,
    workspace_id: Uuid,
    event: AppEvent,
    body: &serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "event_id": event_id(source.id, event),
        "event_time": source.timestamp,
        "type": event.as_str(),
        "workspace_id": workspace_id,
        "event": body,
    })
}

pub fn generate_app_token() -> String {
    format!(
        "{APP_TOKEN_PREFIX}{}",
        crate::bots::generate_token().trim_start_matches(crate::bots::TOKEN_PREFIX)
    )
}

pub fn generate_secret() -> String {
    use rand::RngExt;
    let mut bytes = [0u8; 32];
    rand::rng().fill(&mut bytes[..]);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub async fn deliver_command(
    state: &AppState,
    app_id: Uuid,
    request_id: Uuid,
    channel_id: Uuid,
    command: &str,
    payload: &serde_json::Value,
) -> AppResult<Option<CommandResponse>> {
    let Some(app) = state.bot_repo.find(app_id).await? else {
        return Ok(None);
    };
    if !app.socket_mode {
        return Ok(None);
    }
    if app.connected_sockets == 0 {
        return Err(AppError::BadRequest(format!(
            "/{command} could not be reached"
        )));
    }
    state
        .app_event_repo
        .enqueue(repo::NewDelivery {
            app_id,
            event_id: request_id,
            channel_id,
            kind: KIND_SLASH_COMMAND,
            body: payload,
            ttl_secs: COMMAND_TTL_SECS,
        })
        .await?;
    Ok(Some(CommandResponse::pending(format!(
        "/{command} is working on it…"
    ))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stream_key_names_its_workspace() {
        let workspace_id = Uuid::new_v4();
        assert_eq!(
            workspace_of_stream(&shared_events::workspace_stream(workspace_id)),
            Some(workspace_id)
        );
        assert_eq!(workspace_of_stream("stream:index"), None);
    }

    #[test]
    fn a_reaction_is_translated_without_a_workspace_in_its_payload() {
        let workspace_id = Uuid::new_v4();
        let event = Event::new(
            "reaction.added",
            serde_json::json!({
                "message_id": Uuid::new_v4(),
                "channel_id": Uuid::new_v4(),
                "user_id": Uuid::new_v4(),
                "emoji": "🚀",
            }),
        );
        let translated = translate(workspace_id, &event).expect("translated");
        assert_eq!(translated.workspace_id, workspace_id);
        assert_eq!(translated.events[0].0, AppEvent::ReactionAdded);
    }
}
