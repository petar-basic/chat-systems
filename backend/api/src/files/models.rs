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
