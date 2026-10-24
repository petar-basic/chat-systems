use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

pub const COMMENT_MAX_CHARS: usize = 4000;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ListComment {
    pub id: Uuid,
    pub item_id: Uuid,
    pub list_id: Uuid,
    pub user_id: Uuid,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub edited_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CommentRequest {
    pub content: String,
}

pub fn clean_comment(content: &str) -> AppResult<String> {
    let content = content.trim();
    if content.is_empty() {
        return Err(AppError::Validation("A comment needs some text".into()));
    }
    if content.chars().count() > COMMENT_MAX_CHARS {
        return Err(AppError::Validation(format!(
            "A comment can be at most {COMMENT_MAX_CHARS} characters"
        )));
    }
    Ok(content.to_string())
}
