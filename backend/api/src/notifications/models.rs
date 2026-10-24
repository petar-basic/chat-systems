use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq, ToSchema)]
#[sqlx(type_name = "notification_type", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum NotificationType {
    Mention,
    Dm,
    Reply,
    Reaction,
    Call,
    Reminder,
    System,
    Keyword,
    List,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq, ToSchema)]
#[sqlx(type_name = "notify_level", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum NotifyLevel {
    All,
    Mentions,
    Nothing,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Notification {
    pub id: Uuid,
    pub user_id: Uuid,
    pub workspace_id: Uuid,
    pub notification_type: NotificationType,
    pub title: String,
    pub body: Option<String>,
    pub data: serde_json::Value,
    pub is_read: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MarkNotificationsReadRequest {
    pub notification_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SetDndRequest {
    pub dnd_until: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct EmailPreferenceRequest {
    pub mention_emails: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UpdatedCount {
    pub updated: u64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UnreadCountResponse {
    pub unread_count: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DndStatus {
    pub dnd_until: Option<DateTime<Utc>>,
    pub schedule_paused_until: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct EmailPreference {
    pub mention_emails: bool,
    pub available: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SetNotifyLevelRequest {
    pub level: Option<NotifyLevel>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ChannelNotifyLevel {
    pub notify_level: Option<NotifyLevel>,
    pub effective_notify_level: NotifyLevel,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SetNotifyDefaultRequest {
    pub level: NotifyLevel,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NotifyDefault {
    pub notify_default: NotifyLevel,
}

#[derive(Debug, Clone)]
pub struct Recipient {
    pub user_id: Uuid,
    pub level: NotifyLevel,
    pub muted: bool,
    pub direct: bool,
}

#[derive(Debug, Clone)]
pub struct KeywordMatch {
    pub user_id: Uuid,
    pub keyword: String,
}

#[derive(Debug, Deserialize, garde::Validate, ToSchema)]
pub struct SetKeywordsRequest {
    #[garde(
        length(max = 50),
        inner(custom(shared_common::validation::rules::notification_keyword))
    )]
    pub keywords: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct KeywordList {
    pub keywords: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NotificationSchedule {
    pub enabled: bool,
    pub days: Vec<i16>,
    pub start_minute: i16,
    pub end_minute: i16,
    pub timezone: String,
}

#[derive(Debug, Deserialize, garde::Validate, ToSchema)]
pub struct SetScheduleRequest {
    #[garde(skip)]
    pub enabled: bool,
    #[garde(length(min = 1, max = 7), inner(range(min = 1, max = 7)))]
    pub days: Vec<i16>,
    #[garde(range(min = 0, max = 1439))]
    pub start_minute: i16,
    #[garde(range(min = 0, max = 1439), custom(differs_from(&self.start_minute)))]
    pub end_minute: i16,
}

fn differs_from(start: &i16) -> impl FnOnce(&i16, &()) -> garde::Result + '_ {
    move |end, _| {
        if end == start {
            return Err(garde::Error::new(
                "The schedule must end at a different time than it starts",
            ));
        }
        Ok(())
    }
}

impl SetScheduleRequest {
    pub fn days(&self) -> Vec<i16> {
        let mut days = self.days.clone();
        days.sort_unstable();
        days.dedup();
        days
    }
}
