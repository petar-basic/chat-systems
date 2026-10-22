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
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Attachment {
    pub id: Uuid,
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub url: String,
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
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FileMeta {
    pub file: FileRecord,
    pub url: String,
}
