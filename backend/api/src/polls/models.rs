use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

pub const MIN_OPTIONS: usize = 2;
pub const MAX_OPTIONS: usize = 10;
pub const QUESTION_MAX_CHARS: usize = 300;
pub const OPTION_MAX_CHARS: usize = 100;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreatePollRequest {
    pub question: String,
    pub options: Vec<String>,
    #[serde(default)]
    pub multiple_choice: bool,
    #[serde(default)]
    pub anonymous: bool,
    pub closes_at: Option<DateTime<Utc>>,
    pub thread_parent_id: Option<Uuid>,
}

pub struct PollDraft {
    pub question: String,
    pub options: Vec<String>,
    pub multiple_choice: bool,
    pub anonymous: bool,
    pub closes_at: Option<DateTime<Utc>>,
}

impl CreatePollRequest {
    pub fn draft(&self, now: DateTime<Utc>) -> AppResult<PollDraft> {
        let question = self.question.trim();
        if question.is_empty() {
            return Err(AppError::Validation("Ask a question".into()));
        }
        if question.chars().count() > QUESTION_MAX_CHARS {
            return Err(AppError::Validation(format!(
                "A question can be at most {QUESTION_MAX_CHARS} characters"
            )));
        }
        let options: Vec<String> = self
            .options
            .iter()
            .map(|option| option.trim().to_string())
            .filter(|option| !option.is_empty())
            .collect();
        if !(MIN_OPTIONS..=MAX_OPTIONS).contains(&options.len()) {
            return Err(AppError::Validation(format!(
                "A poll needs {MIN_OPTIONS} to {MAX_OPTIONS} options"
            )));
        }
        if options.iter().any(|o| o.chars().count() > OPTION_MAX_CHARS) {
            return Err(AppError::Validation(format!(
                "An option can be at most {OPTION_MAX_CHARS} characters"
            )));
        }
        let mut seen: Vec<String> = Vec::with_capacity(options.len());
        for option in &options {
            let folded = option.to_lowercase();
            if seen.contains(&folded) {
                return Err(AppError::Validation(format!(
                    "\"{option}\" is listed twice"
                )));
            }
            seen.push(folded);
        }
        if self.closes_at.is_some_and(|at| at <= now) {
            return Err(AppError::Validation(
                "The closing time has to be in the future".into(),
            ));
        }
        Ok(PollDraft {
            question: question.to_string(),
            options,
            multiple_choice: self.multiple_choice,
            anonymous: self.anonymous,
            closes_at: self.closes_at,
        })
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct VoteRequest {
    pub option_ids: Vec<Uuid>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MyVote {
    pub option_ids: Vec<Uuid>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct VoteResponse {
    pub option_ids: Vec<Uuid>,
    pub poll: PollSummary,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct OptionVoters {
    pub option_id: Uuid,
    pub user_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct PollSummary {
    pub id: Uuid,
    pub question: String,
    pub multiple_choice: bool,
    pub anonymous: bool,
    pub closes_at: Option<DateTime<Utc>>,
    pub closed_at: Option<DateTime<Utc>>,
    pub created_by: Uuid,
    pub options: Vec<PollOptionCount>,
    pub voters: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct PollOptionCount {
    pub id: Uuid,
    pub label: String,
    pub votes: i64,
}

#[derive(Debug, Clone)]
pub struct Poll {
    pub id: Uuid,
    pub message_id: Uuid,
    pub question: String,
    pub multiple_choice: bool,
    pub anonymous: bool,
    pub closes_at: Option<DateTime<Utc>>,
    pub closed_at: Option<DateTime<Utc>>,
    pub created_by: Uuid,
}

impl Poll {
    pub fn is_closed(&self, now: DateTime<Utc>) -> bool {
        self.closed_at.is_some() || self.closes_at.is_some_and(|at| at <= now)
    }
}

#[derive(Debug, Clone)]
pub struct PollPlace {
    pub poll: Poll,
    pub channel_id: Uuid,
    pub workspace_id: Uuid,
    pub message_deleted: bool,
}

#[derive(Debug, Clone)]
pub struct OptionTally {
    pub id: Uuid,
    pub label: String,
    pub votes: i64,
}
