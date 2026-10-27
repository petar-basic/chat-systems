use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use redis::AsyncCommands;
use tracing::{info, warn};

use shared_events::Event;

use super::models::{NotificationType, NotifyLevel, Recipient};
use super::repo::NotificationRepo;
use crate::messaging::stream_group::StreamGroup;
use crate::push::sender::{PushPayload, PushSender};

pub const RING_MAX_AGE: chrono::Duration = chrono::Duration::seconds(60);
const ITEM_TITLE_MAX_CHARS: usize = 120;

pub async fn start_consumer(
    redis_url: &str,
    app_state: Arc<crate::state::AppState>,
    repo: Arc<NotificationRepo>,
    push: Arc<PushSender>,
) {
    let Some(mut group) = StreamGroup::connect(redis_url, "notifications").await else {
        return;
    };
    let mut pub_conn = group.connection();

    info!("Notification consumer started");

    loop {
        for delivery in group.next_batch().await {
            // An async block rather than a bare match so that skipping an event
            // cannot skip its acknowledgement: `continue` will not compile here,
            // which is the point.
            async {
                match delivery.event.event_type.as_str() {
                    "message.created" => {
                        notify_message(
                            &app_state,
                            &repo,
                            &push,
                            &mut pub_conn,
                            &delivery.event.payload,
                        )
                        .await;
                    }
                    "huddle.ring" => notify_ring(&repo, &mut pub_conn, &delivery.event).await,
                    "list.comment_upserted" | "list.assigned" => {
                        notify_list(&app_state, &repo, &push, &mut pub_conn, &delivery.event).await;
                    }
                    _ => {}
                }
            }
            .await;

            group.ack(&delivery.key, &delivery.id).await;
        }
    }
}

pub struct Notice {
    pub activity: Option<NotificationType>,
    pub kind: &'static str,
    pub title: String,
    pub email: bool,
}

#[derive(Clone, Copy)]
pub struct Origin<'a> {
    pub workspace_id: Option<uuid::Uuid>,
    pub channel_id: Option<&'a str>,
    pub channel_uuid: Option<uuid::Uuid>,
    pub message_id: Option<&'a str>,
    pub thread_parent_id: Option<&'a str>,
    pub sender_id: Option<&'a str>,
    pub content: &'a str,
    pub list_item: Option<(uuid::Uuid, uuid::Uuid)>,
}

impl<'a> Origin<'a> {
    fn of(payload: &'a serde_json::Value, content: &'a str) -> Self {
        let text = |name: &str| payload.get(name).and_then(|v| v.as_str());
        Self {
            workspace_id: text("workspace_id").and_then(|v| v.parse().ok()),
            channel_id: text("channel_id"),
            channel_uuid: text("channel_id").and_then(|v| v.parse().ok()),
            message_id: text("id"),
            thread_parent_id: text("thread_parent_id"),
            sender_id: text("user_id"),
            content,
            list_item: None,
        }
    }
}

fn mentioned_ids(payload: &serde_json::Value) -> Vec<uuid::Uuid> {
    ids_in(payload, "mentioned_user_ids")
}

fn ids_in(payload: &serde_json::Value, key: &str) -> Vec<uuid::Uuid> {
    payload
        .get(key)
        .and_then(|v| v.as_array())
        .map(|ids| {
            ids.iter()
                .filter_map(|id| id.as_str().and_then(|v| v.parse().ok()))
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    Mention,
    Reply,
    Direct,
    Keyword,
    Message,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Signals {
    pub mentioned: bool,
    pub following: bool,
    pub keyword: bool,
    pub in_thread: bool,
}

pub fn reason_for(recipient: &Recipient, signals: Signals) -> Option<Reason> {
    if recipient.muted || recipient.level == NotifyLevel::Nothing {
        return None;
    }
    let everything = recipient.level == NotifyLevel::All && !signals.in_thread;
    if signals.mentioned {
        Some(Reason::Mention)
    } else if signals.following {
        Some(Reason::Reply)
    } else if everything && recipient.direct {
        Some(Reason::Direct)
    } else if signals.keyword {
        Some(Reason::Keyword)
    } else if everything {
        Some(Reason::Message)
    } else {
        None
    }
}

struct Names {
    sender: String,
    channel: String,
}

impl Names {
    async fn of(state: &crate::state::AppState, origin: &Origin<'_>) -> Self {
        let sender = match (
            origin.workspace_id,
            origin.sender_id.and_then(|v| v.parse::<uuid::Uuid>().ok()),
        ) {
            (Some(workspace_id), Some(id)) => state
                .profile_repo
                .name(workspace_id, id)
                .await
                .ok()
                .flatten(),
            (None, Some(id)) => state
                .auth_service
                .repo()
                .find_by_id(id)
                .await
                .ok()
                .flatten()
                .and_then(|u| u.display_name),
            _ => None,
        };
        let channel = match origin.channel_uuid {
            Some(id) => state
                .workspace_service
                .repo
                .find_channel_by_id(id)
                .await
                .ok()
                .flatten()
                .and_then(|c| c.name),
            None => None,
        };
        Self {
            sender: sender.unwrap_or_else(|| "Somebody".to_string()),
            channel: channel.unwrap_or_default(),
        }
    }

    fn notice(&self, reason: Reason, keyword: Option<&str>) -> Notice {
        match reason {
            Reason::Mention => Notice {
                activity: Some(NotificationType::Mention),
                kind: "mention",
                title: "You were mentioned".into(),
                email: true,
            },
            Reason::Reply => Notice {
                activity: Some(NotificationType::Reply),
                kind: "reply",
                title: "New reply in a thread".into(),
                email: false,
            },
            Reason::Direct => Notice {
                activity: Some(NotificationType::Dm),
                kind: "dm",
                title: self.sender.clone(),
                email: true,
            },
            Reason::Keyword => Notice {
                activity: Some(NotificationType::Keyword),
                kind: "keyword",
                title: match (keyword.unwrap_or_default(), self.channel.as_str()) {
                    (word, "") => format!("Matched \"{word}\""),
                    (word, channel) => format!("Matched \"{word}\" in #{channel}"),
                },
                email: false,
            },
            Reason::Message => Notice {
                activity: None,
                kind: "message",
                title: format!("{} in #{}", self.sender, self.channel),
                email: false,
            },
        }
    }
}

pub async fn notify_message(
    state: &crate::state::AppState,
    repo: &NotificationRepo,
    push: &PushSender,
    pub_conn: &mut redis::aio::ConnectionManager,
    event_payload: &serde_json::Value,
) {
    let content = preview_text(event_payload);
    let origin = Origin::of(event_payload, &content);
    let (Some(channel_id), Some(sender)) = (
        origin.channel_uuid,
        origin.sender_id.and_then(|v| v.parse::<uuid::Uuid>().ok()),
    ) else {
        return;
    };

    let mut recipients = match repo.recipients(channel_id, sender).await {
        Ok(recipients) => recipients,
        Err(e) => {
            warn!(channel_id = %channel_id, "Notification consumer: failed to read recipients: {e}");
            return;
        }
    };

    let root = origin
        .thread_parent_id
        .and_then(|v| v.parse::<uuid::Uuid>().ok());
    let followers: HashSet<uuid::Uuid> = match root {
        Some(root) => match state.thread_repo.reply_recipients(root, sender).await {
            Ok(followers) => followers.into_iter().collect(),
            Err(e) => {
                warn!(root_message_id = %root, "Notification consumer: failed to read thread followers: {e}");
                HashSet::new()
            }
        },
        None => HashSet::new(),
    };
    let members: HashSet<uuid::Uuid> = recipients.iter().map(|r| r.user_id).collect();
    recipients.extend(
        followers
            .iter()
            .filter(|uid| !members.contains(uid))
            .map(|&user_id| Recipient {
                user_id,
                level: NotifyLevel::Mentions,
                muted: false,
                direct: false,
            }),
    );

    let mentioned: HashSet<uuid::Uuid> = mentioned_ids(event_payload).into_iter().collect();
    let keywords = keyword_matches(repo, channel_id, sender, event_payload).await;
    let reasons: Vec<(uuid::Uuid, Reason)> = recipients
        .iter()
        .filter_map(|r| {
            let signals = Signals {
                mentioned: mentioned.contains(&r.user_id),
                following: followers.contains(&r.user_id),
                keyword: keywords.contains_key(&r.user_id),
                in_thread: root.is_some(),
            };
            reason_for(r, signals).map(|reason| (r.user_id, reason))
        })
        .collect();
    if reasons.is_empty() {
        return;
    }

    let names = Names::of(state, &origin).await;
    let reply_origin = Origin {
        message_id: origin.thread_parent_id,
        ..origin
    };
    let via = Via { state, repo, push };
    for (uid, reason) in reasons {
        let at = if reason == Reason::Reply {
            &reply_origin
        } else {
            &origin
        };
        let notice = names.notice(reason, keywords.get(&uid).map(String::as_str));
        deliver(&via, pub_conn, uid, at, &names, &notice).await;
    }
}

async fn keyword_matches(
    repo: &NotificationRepo,
    channel_id: uuid::Uuid,
    sender: uuid::Uuid,
    payload: &serde_json::Value,
) -> HashMap<uuid::Uuid, String> {
    let content = payload
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if content.trim().is_empty() {
        return HashMap::new();
    }
    match repo.keyword_matches(channel_id, sender, content).await {
        Ok(matches) => matches
            .into_iter()
            .map(|m| (m.user_id, m.keyword))
            .collect(),
        Err(e) => {
            warn!(channel_id = %channel_id, "Notification consumer: failed to match keywords: {e}");
            HashMap::new()
        }
    }
}

struct Via<'a> {
    state: &'a crate::state::AppState,
    repo: &'a NotificationRepo,
    push: &'a PushSender,
}

async fn deliver(
    via: &Via<'_>,
    pub_conn: &mut redis::aio::ConnectionManager,
    uid: uuid::Uuid,
    origin: &Origin<'_>,
    names: &Names,
    notice: &Notice,
) {
    let Via { state, repo, push } = *via;
    let channel_id = origin.channel_id;
    let message_id = origin.message_id;
    let content = origin.content;
    let Some(ws_id) = origin.workspace_id else {
        return;
    };

    let mut data_json = serde_json::json!({
        "channel_id": channel_id,
        "message_id": message_id,
        "thread_parent_id": origin.thread_parent_id,
    });
    let mut payload = serde_json::json!({
        "user_id": uid.to_string(),
        "workspace_id": ws_id.to_string(),
        "channel_id": channel_id,
        "message_id": message_id,
        "title": notice.title,
        "body": content,
        "kind": notice.kind,
    });
    if let Some((list_id, item_id)) = origin.list_item {
        for fields in [&mut data_json, &mut payload] {
            fields["list_id"] = serde_json::json!(list_id);
            fields["item_id"] = serde_json::json!(item_id);
        }
    }

    if let Some(activity) = &notice.activity {
        if let Err(e) = repo
            .create(
                uid,
                ws_id,
                activity,
                &notice.title,
                Some(content),
                &data_json,
            )
            .await
        {
            warn!(
                user_id = %uid,
                workspace_id = %ws_id,
                "Notification consumer: failed to persist a notification: {}",
                e
            );
        }
    }

    if repo.is_dnd_active(ws_id, uid).await.unwrap_or(false) {
        return;
    }

    let notif_event = serde_json::json!({
        "event_type": "notification.push",
        "payload": payload,
    });

    let json = serde_json::to_string(&notif_event).unwrap_or_default();
    let _: Result<(), _> = pub_conn.publish("events:notification", &json).await;

    if crate::presence::is_online(pub_conn, ws_id, uid).await {
        return;
    }

    let badge = repo.unread_count(uid, ws_id).await.unwrap_or(0);
    let reached = push
        .send_to_user(
            uid,
            &PushPayload {
                title: notice.title.clone(),
                body: PushPayload::preview(content),
                workspace_id: Some(ws_id.to_string()),
                channel_id: channel_id.map(str::to_string),
                message_id: message_id.map(str::to_string),
                badge_count: badge,
            },
        )
        .await;

    if reached || !notice.email {
        return;
    }

    super::email::enqueue(
        state,
        super::email::PendingMention {
            user_id: uid,
            workspace_id: ws_id,
            channel_id: origin.channel_uuid,
            message_id: message_id.and_then(|v| v.parse().ok()),
            sender_name: &names.sender,
            channel_name: &names.channel,
            direct: notice.activity == Some(NotificationType::Dm),
        },
    )
    .await;
}

/// A ring that reaches the worker late — redelivered after a crash, or read
/// from a backlog — is dropped rather than rung: a call announced a minute
/// after it started is worse than one not announced. A ring delivered twice
/// creates one notification, because the row is unique per person and call.
pub async fn notify_ring(
    repo: &NotificationRepo,
    pub_conn: &mut redis::aio::ConnectionManager,
    event: &Event,
) {
    if chrono::Utc::now() - event.timestamp > RING_MAX_AGE {
        return;
    }

    let field = |name: &str| {
        event
            .payload
            .get(name)
            .and_then(|v| v.as_str())
            .and_then(|v| v.parse::<uuid::Uuid>().ok())
    };
    let (Some(to_user_id), Some(workspace_id), Some(huddle_id)) = (
        field("to_user_id"),
        field("workspace_id"),
        field("huddle_id"),
    ) else {
        return;
    };

    let data = serde_json::json!({
        "huddle_id": huddle_id,
        "from_user_id": event.payload.get("from_user_id"),
    });

    let created = match repo
        .create(
            to_user_id,
            workspace_id,
            &NotificationType::Call,
            "Incoming huddle",
            Some("Someone is starting a huddle"),
            &data,
        )
        .await
    {
        Ok(created) => created,
        Err(e) => {
            warn!(user_id = %to_user_id, "failed to persist call notification: {}", e);
            return;
        }
    };
    if created.is_none() {
        return;
    }

    if repo
        .is_dnd_active(workspace_id, to_user_id)
        .await
        .unwrap_or(false)
    {
        return;
    }

    let notif_event = serde_json::json!({
        "event_type": "notification.push",
        "payload": {
            "user_id": to_user_id.to_string(),
            "workspace_id": workspace_id.to_string(),
            "title": "Incoming huddle",
            "body": "Someone is starting a huddle",
            "kind": "call",
        }
    });
    let json = serde_json::to_string(&notif_event).unwrap_or_default();
    let _: Result<(), _> = pub_conn.publish("events:notification", &json).await;
}

pub async fn notify_list(
    state: &crate::state::AppState,
    repo: &NotificationRepo,
    push: &PushSender,
    pub_conn: &mut redis::aio::ConnectionManager,
    event: &Event,
) {
    let payload = &event.payload;
    let text = |name: &str| payload.get(name).and_then(|v| v.as_str());
    let uuid = |name: &str| text(name).and_then(|v| v.parse::<uuid::Uuid>().ok());
    let (Some(list_id), Some(item_id)) = (uuid("list_id"), uuid("item_id")) else {
        return;
    };
    let item_title = match text("item_title").unwrap_or("Untitled") {
        title if title.chars().count() > ITEM_TITLE_MAX_CHARS => {
            let clipped: String = title.chars().take(ITEM_TITLE_MAX_CHARS - 1).collect();
            format!("{clipped}…")
        }
        title => title.to_string(),
    };
    let list_title = text("list_title").unwrap_or_default();
    let assigned = event.event_type == "list.assigned";
    let (actor, candidates, body) = if assigned {
        (
            text("actor_id"),
            ids_in(payload, "assignee_ids"),
            format!("in {list_title}"),
        )
    } else {
        (
            text("author_id"),
            mentioned_ids(payload),
            payload
                .pointer("/comment/content")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
        )
    };
    let Some(actor_id) = actor.and_then(|v| v.parse::<uuid::Uuid>().ok()) else {
        return;
    };
    let mut candidates: Vec<uuid::Uuid> = candidates
        .into_iter()
        .filter(|id| *id != actor_id)
        .collect();
    candidates.sort();
    candidates.dedup();
    if candidates.is_empty() {
        return;
    }

    let readers = match uuid("channel_id") {
        Some(channel_id) => {
            match state
                .list_repo
                .channel_members_among(channel_id, &candidates)
                .await
            {
                Ok(readers) => readers,
                Err(e) => {
                    warn!(list_id = %list_id, "Notification consumer: failed to read list readers: {e}");
                    return;
                }
            }
        }
        None => uuid("owner_id")
            .filter(|owner| candidates.contains(owner))
            .into_iter()
            .collect(),
    };
    if readers.is_empty() {
        return;
    }

    let origin = Origin {
        workspace_id: uuid("workspace_id"),
        channel_id: text("channel_id"),
        channel_uuid: uuid("channel_id"),
        message_id: None,
        thread_parent_id: None,
        sender_id: actor,
        content: &body,
        list_item: Some((list_id, item_id)),
    };
    let names = Names::of(state, &origin).await;
    let notice = Notice {
        activity: Some(NotificationType::List),
        kind: "list",
        title: if assigned {
            format!("{} assigned you to {item_title}", names.sender)
        } else {
            format!("{} mentioned you on {item_title}", names.sender)
        },
        email: false,
    };
    let via = Via { state, repo, push };
    for uid in readers {
        deliver(&via, pub_conn, uid, &origin, &names, &notice).await;
    }
}

fn preview_text(payload: &serde_json::Value) -> String {
    let content = payload
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if !content.is_empty() {
        return content.to_string();
    }
    let files = crate::files::service::attachments_in(
        payload.get("metadata").unwrap_or(&serde_json::Value::Null),
    );
    match files.as_slice() {
        [] => String::new(),
        [only] => format!("📎 {}", only.filename),
        many => format!("📎 {} files", many.len()),
    }
}
