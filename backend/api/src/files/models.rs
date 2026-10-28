use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct FileRecord {
    pub id: Uuid,
    pub message_id: Option<Uuid>,
    pub user_id: Uuid,
    pub workspace_id: Uuid,
    pub filename: String,
    pub storage_key: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub thumbnail_key: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Attachment {
    pub id: Uuid,
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<i32>,
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct UploadQuery {
    #[serde(default)]
    pub draft: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FileUploadResponse {
    pub id: Uuid,
    pub url: String,
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub thumbnail_url: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FileMeta {
    pub file: FileRecord,
    pub url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum FileKindFilter {
    Image,
    Video,
    Audio,
    Document,
}

impl FileKindFilter {
    pub fn as_str(self) -> &'static str {
        match self {
            FileKindFilter::Image => "image",
            FileKindFilter::Video => "video",
            FileKindFilter::Audio => "audio",
            FileKindFilter::Document => "document",
        }
    }
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct ChannelFilesQuery {
    #[param(inline)]
    pub kind: Option<FileKindFilter>,
    pub cursor: Option<Uuid>,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ChannelFile {
    #[serde(flatten)]
    pub attachment: Attachment,
    pub message_id: Uuid,
    pub user_id: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "file_kind", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
    Pdf,
    Image,
    Video,
    Audio,
    Spreadsheet,
    Presentation,
    Document,
    Archive,
    Text,
    Other,
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct FileSearchQuery {
    pub q: Option<String>,
    pub workspace_id: Uuid,
    pub channel_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    #[param(inline)]
    pub kind: Option<FileKind>,
    pub after: Option<DateTime<Utc>>,
    pub before: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

impl FileSearchQuery {
    pub fn has_filter(&self) -> bool {
        self.channel_id.is_some()
            || self.user_id.is_some()
            || self.kind.is_some()
            || self.after.is_some()
            || self.before.is_some()
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FileSearchHit {
    #[serde(flatten)]
    pub attachment: Attachment,
    pub kind: FileKind,
    pub message_id: Uuid,
    pub channel_id: Uuid,
    pub channel_name: Option<String>,
    pub user_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub snippet: Option<Vec<super::text::SnippetPart>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "file_text_status", rename_all = "snake_case")]
pub enum FileTextStatus {
    Pending,
    Done,
    Empty,
    Unsupported,
    TooLarge,
    Failed,
}

impl FileTextStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            FileTextStatus::Pending => "pending",
            FileTextStatus::Done => "done",
            FileTextStatus::Empty => "empty",
            FileTextStatus::Unsupported => "unsupported",
            FileTextStatus::TooLarge => "too_large",
            FileTextStatus::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone)]
pub struct TextJob {
    pub file_id: Uuid,
    pub storage_key: String,
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub kind: FileKind,
}
