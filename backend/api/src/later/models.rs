use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::files::models::Attachment;

pub type AttachmentList = sqlx::types::Json<Vec<Attachment>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum LaterStatus {
    InProgress,
    Completed,
    Archived,
}

impl LaterStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            LaterStatus::InProgress => "in_progress",
            LaterStatus::Completed => "completed",
            LaterStatus::Archived => "archived",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "completed" => LaterStatus::Completed,
            "archived" => LaterStatus::Archived,
            _ => LaterStatus::InProgress,
        }
    }
}

pub struct LaterRow {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub created_by: Uuid,
    pub channel_id: Option<Uuid>,
    pub message_id: Option<Uuid>,
    pub text: Option<String>,
    pub note: Option<String>,
    pub status: String,
    pub remind_at: Option<DateTime<Utc>>,
    pub reminded_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub message_author_id: Option<Uuid>,
    pub message_content: Option<String>,
    pub message_sent_at: Option<DateTime<Utc>>,
    pub message_attachments: AttachmentList,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LaterMessage {
    pub id: Uuid,
    pub channel_id: Uuid,
    pub author_id: Uuid,
    pub content: String,
    pub attachments: Vec<Attachment>,
    pub sent_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LaterItem {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub created_by: Uuid,
    pub channel_id: Option<Uuid>,
    pub status: LaterStatus,
    pub text: Option<String>,
    pub note: Option<String>,
    pub remind_at: Option<DateTime<Utc>>,
    pub reminded_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub message: Option<LaterMessage>,
}

impl From<LaterRow> for LaterItem {
    fn from(row: LaterRow) -> Self {
        let message = match (
            row.message_id,
            row.channel_id,
            row.message_author_id,
            row.message_content,
            row.message_sent_at,
        ) {
            (Some(id), Some(channel_id), Some(author_id), Some(content), Some(sent_at)) => {
                Some(LaterMessage {
                    id,
                    channel_id,
                    author_id,
                    content,
                    attachments: row.message_attachments.0,
                    sent_at,
                })
            }
            _ => None,
        };
        LaterItem {
            id: row.id,
            workspace_id: row.workspace_id,
            created_by: row.created_by,
            channel_id: row.channel_id,
            status: LaterStatus::parse(&row.status),
            text: row.text,
            note: row.note,
            remind_at: row.remind_at,
            reminded_at: row.reminded_at,
            completed_at: row.completed_at,
            created_at: row.created_at,
            message,
        }
    }
}

pub struct LaterOwner {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub user_id: Uuid,
}

pub struct DueReminder {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub channel_id: Option<Uuid>,
    pub message_id: Option<Uuid>,
    pub text: Option<String>,
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct LaterQuery {
    #[param(inline)]
    pub status: Option<LaterStatus>,
}

#[derive(Debug, Deserialize, garde::Validate, ToSchema)]
#[garde(allow_unvalidated)]
pub struct CreateLaterRequest {
    pub message_id: Option<Uuid>,
    #[garde(inner(custom(shared_common::validation::rules::reminder_content)))]
    pub text: Option<String>,
    pub remind_at: Option<DateTime<Utc>>,
    pub channel_id: Option<Uuid>,
    pub for_user_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, garde::Validate, ToSchema)]
#[garde(allow_unvalidated)]
pub struct UpdateLaterRequest {
    pub status: Option<LaterStatus>,
    pub remind_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub clear_reminder: bool,
    #[garde(inner(length(max = 1000)))]
    pub note: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LaterCreated {
    pub id: Uuid,
}
