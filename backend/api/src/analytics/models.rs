use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::workspace::models::{ChannelType, WorkspaceRole};

pub const WINDOWS: [i32; 3] = [7, 30, 90];

#[derive(Debug, Deserialize, IntoParams)]
pub struct AnalyticsQuery {
    pub days: i32,
    #[serde(default = "include")]
    pub guests: bool,
}

fn include() -> bool {
    true
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DailyActivity {
    pub day: NaiveDate,
    pub members: i64,
    pub active_members: i64,
    pub joined: i64,
    pub public_messages: i64,
    pub private_messages: i64,
    pub direct_messages: i64,
    pub files: i64,
    pub reactions: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct WeeklyActivity {
    pub week: NaiveDate,
    pub active_members: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct WorkspaceAnalytics {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub members: i64,
    pub active_members: i64,
    pub daily: Vec<DailyActivity>,
    pub weekly: Vec<WeeklyActivity>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MemberActivity {
    pub user_id: Uuid,
    pub role: WorkspaceRole,
    pub days_active: i64,
    pub messages: i64,
    pub last_active: Option<NaiveDate>,
}

pub const QUIET_THRESHOLDS: [i32; 4] = [30, 60, 90, 180];
pub const MAX_ARCHIVED_AT_ONCE: usize = 200;

#[derive(Debug, Serialize, ToSchema)]
pub struct ChannelActivity {
    pub channel_id: Uuid,
    pub name: Option<String>,
    pub channel_type: ChannelType,
    pub is_archived: bool,
    pub members: i64,
    pub messages: i64,
    pub posters: i64,
    pub readers: i64,
    pub last_message_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ChannelAnalytics {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub channel: ChannelActivity,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct QuietQuery {
    pub days: i32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct QuietChannel {
    pub channel_id: Uuid,
    pub name: Option<String>,
    pub channel_type: ChannelType,
    pub members: i64,
    pub last_message_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ArchiveChannelsRequest {
    pub channel_ids: Vec<Uuid>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ArchivedChannels {
    pub archived: Vec<Uuid>,
}
