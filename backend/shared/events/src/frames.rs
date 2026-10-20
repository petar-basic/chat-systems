use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct PresenceEntry {
    pub user_id: Uuid,
    pub status: String,
}

/// Every frame the gateway sends to a client. The `type` tag is the wire
/// name; the TypeScript union the frontend compiles against is generated from
/// this enum, so a frame that is not here cannot be sent.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "type")]
pub enum ServerFrame {
    #[serde(rename = "hello")]
    Hello { user_id: Uuid, connection_id: Uuid },
    #[serde(rename = "pong")]
    Pong,
    #[serde(rename = "sync.complete")]
    SyncComplete {
        workspace_id: Uuid,
        replayed: usize,
        last_event_id: Option<String>,
    },
    #[serde(rename = "sync.refetch_required")]
    SyncRefetchRequired { workspace_id: Uuid },
    #[serde(rename = "presence.batch")]
    PresenceBatch {
        #[ts(inline)]
        users: Vec<PresenceEntry>,
    },
    #[serde(rename = "presence.changed")]
    PresenceChanged { user_id: Uuid, status: String },
    #[serde(rename = "typing.indicator")]
    TypingIndicator {
        channel_id: Uuid,
        user_id: Uuid,
        is_typing: bool,
    },
    #[serde(rename = "message.new")]
    MessageNew {
        #[ts(type = "Message")]
        message: serde_json::Value,
        mentioned_user_ids: Vec<Uuid>,
    },
    #[serde(rename = "message.updated")]
    MessageUpdated {
        #[ts(type = "Message")]
        message: serde_json::Value,
    },
    #[serde(rename = "message.deleted")]
    MessageDeleted { message_id: Uuid, channel_id: Uuid },
    #[serde(rename = "message.pinned")]
    MessagePinned {
        message_id: Uuid,
        channel_id: Uuid,
        pinned: bool,
    },
    #[serde(rename = "reaction.added")]
    ReactionAdded {
        message_id: Uuid,
        #[ts(type = "Reaction & { channel_id: string }")]
        reaction: serde_json::Value,
    },
    #[serde(rename = "reaction.removed")]
    ReactionRemoved {
        message_id: Uuid,
        channel_id: Uuid,
        user_id: Uuid,
        emoji: String,
    },
    #[serde(rename = "workspace.deleted")]
    WorkspaceDeleted {
        workspace_id: Uuid,
        delete_type: String,
    },
    #[serde(rename = "workspace.restored")]
    WorkspaceRestored { workspace_id: Uuid },
    #[serde(rename = "notification")]
    Notification {
        workspace_id: Option<Uuid>,
        channel_id: Option<Uuid>,
        message_id: Option<Uuid>,
        title: String,
        body: Option<String>,
        priority: Option<String>,
    },
    #[serde(rename = "conversation.created")]
    ConversationCreated {
        id: Uuid,
        workspace_id: Uuid,
        kind: String,
        created_by: Option<Uuid>,
        last_message_at: DateTime<Utc>,
        created_at: DateTime<Utc>,
        conversation_id: Uuid,
        participant_ids: Vec<Uuid>,
    },
    #[serde(rename = "huddle.member_joined")]
    HuddleMemberJoined { huddle_id: Uuid, user_id: Uuid },
    #[serde(rename = "huddle.member_left")]
    HuddleMemberLeft { huddle_id: Uuid, user_id: Uuid },
    #[serde(rename = "huddle.members")]
    HuddleMembers {
        huddle_id: Uuid,
        user_ids: Vec<Uuid>,
    },
    #[serde(rename = "huddle.mute")]
    HuddleMute {
        huddle_id: Uuid,
        user_id: Uuid,
        audio_muted: bool,
    },
    #[serde(rename = "huddle.camera")]
    HuddleCamera {
        huddle_id: Uuid,
        user_id: Uuid,
        camera_on: bool,
    },
    #[serde(rename = "huddle.screenshare")]
    HuddleScreenshare {
        huddle_id: Uuid,
        user_id: Uuid,
        sharing: bool,
    },
    #[serde(rename = "huddle.reaction")]
    HuddleReaction {
        huddle_id: Uuid,
        user_id: Uuid,
        emoji: String,
    },
    #[serde(rename = "huddle.hand")]
    HuddleHand {
        huddle_id: Uuid,
        user_id: Uuid,
        raised: bool,
    },
    #[serde(rename = "huddle.offer")]
    HuddleOffer {
        huddle_id: Uuid,
        from_user_id: Uuid,
        to_user_id: Uuid,
        #[ts(type = "RTCSessionDescriptionInit")]
        sdp: serde_json::Value,
    },
    #[serde(rename = "huddle.answer")]
    HuddleAnswer {
        huddle_id: Uuid,
        from_user_id: Uuid,
        to_user_id: Uuid,
        #[ts(type = "RTCSessionDescriptionInit")]
        sdp: serde_json::Value,
    },
    #[serde(rename = "huddle.ice")]
    HuddleIce {
        huddle_id: Uuid,
        from_user_id: Uuid,
        to_user_id: Uuid,
        #[ts(type = "RTCIceCandidateInit")]
        candidate: serde_json::Value,
    },
    #[serde(rename = "huddle.ring")]
    HuddleRing {
        huddle_id: Uuid,
        workspace_id: Uuid,
        from_user_id: Uuid,
        to_user_id: Uuid,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        channel_id: Option<Uuid>,
    },
    #[serde(rename = "huddle.started")]
    HuddleStarted {
        huddle_id: Uuid,
        workspace_id: Uuid,
        channel_id: Option<Uuid>,
        #[serde(default)]
        dm_partner_id: Option<Uuid>,
        initiator_id: Uuid,
    },
    #[serde(rename = "huddle.ended")]
    HuddleEnded {
        huddle_id: Uuid,
        workspace_id: Uuid,
        channel_id: Option<Uuid>,
        #[serde(default)]
        dm_partner_id: Option<Uuid>,
        #[serde(default)]
        initiator_id: Option<Uuid>,
    },
    #[serde(rename = "channel.access_revoked")]
    ChannelAccessRevoked {
        channel_id: Uuid,
        workspace_id: Option<Uuid>,
    },
    #[serde(rename = "workspace.access_revoked")]
    WorkspaceAccessRevoked { workspace_id: Uuid },
}

impl ServerFrame {
    /// Reads a published event back as the frame it becomes on the wire: the
    /// event type is the tag and the payload carries the fields.
    pub fn from_event(
        event_type: &str,
        payload: &serde_json::Value,
    ) -> Result<Self, serde_json::Error> {
        let mut value = payload.clone();
        if let Some(obj) = value.as_object_mut() {
            obj.insert(
                "type".into(),
                serde_json::Value::String(event_type.to_string()),
            );
        }
        serde_json::from_value(value)
    }

    /// The TypeScript union, with the message shapes it refers to taken from
    /// the OpenAPI-generated schema rather than declared twice.
    pub fn typescript() -> String {
        let decl = <Self as TS>::decl(&ts_rs::Config::default());
        format!(
            "// Generated by `cargo run --bin chat-ts-events`. Do not edit.\n\
             import type {{ components }} from './schema';\n\n\
             type Message = components['schemas']['Message'];\n\
             type Reaction = components['schemas']['Reaction'];\n\n\
             export {decl}\n"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wire_tag_is_the_event_name() {
        let frame = ServerFrame::MessageDeleted {
            message_id: Uuid::nil(),
            channel_id: Uuid::nil(),
        };
        let json = serde_json::to_value(&frame).unwrap();
        assert_eq!(json["type"], "message.deleted");
        assert_eq!(
            serde_json::to_value(ServerFrame::Pong).unwrap(),
            serde_json::json!({ "type": "pong" })
        );
    }

    #[test]
    fn a_published_payload_reads_back_as_its_frame() {
        let payload = serde_json::json!({
            "huddle_id": Uuid::nil(),
            "user_id": Uuid::nil(),
            "audio_muted": true,
            "extra_field_the_gateway_does_not_care_about": 1,
        });
        match ServerFrame::from_event("huddle.mute", &payload).unwrap() {
            ServerFrame::HuddleMute { audio_muted, .. } => assert!(audio_muted),
            other => panic!("wrong frame: {other:?}"),
        }
        assert!(ServerFrame::from_event("huddle.mute", &serde_json::json!({})).is_err());
    }

    #[test]
    fn the_typescript_union_names_every_frame_by_its_wire_tag() {
        let ts = ServerFrame::typescript();
        assert!(ts.contains("export type ServerFrame ="));
        assert!(ts.contains(r#""type": "message.new""#) || ts.contains(r#"type: "message.new""#));
        assert!(ts.contains("message: Message"));
        assert!(ts.contains("candidate: RTCIceCandidateInit"));
        assert!(!ts.contains("JsonValue"));
    }
}
