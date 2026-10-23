use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::messaging::models::Message;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ThreadState {
    pub root_message_id: Uuid,
    pub following: bool,
    pub unread_count: i64,
}

pub struct FollowerState {
    pub user_id: Uuid,
    pub following: bool,
    pub unread_count: i64,
}

pub struct FollowedThread {
    pub root_message_id: Uuid,
    pub unread_count: i64,
    pub last_read_at: DateTime<Utc>,
    pub last_reply_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ThreadSummary {
    pub root: Message,
    pub latest_replies: Vec<Message>,
    pub unread_count: i64,
    pub last_read_at: DateTime<Utc>,
    pub last_reply_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ThreadPage {
    pub data: Vec<ThreadSummary>,
    pub next_cursor: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UnreadThreads {
    pub count: i64,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ThreadsQuery {
    pub cursor: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
}
