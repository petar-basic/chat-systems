use std::time::Duration;

use serde::Deserialize;

use shared_common::errors::{AppError, AppResult};

const TIMEOUT: Duration = Duration::from_secs(5);
const USER_AGENT: &str = concat!("Chat-Systems/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, Deserialize)]
pub struct TokenSet {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: Option<i64>,
}

pub fn client() -> AppResult<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(TIMEOUT)
        .user_agent(USER_AGENT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| AppError::Internal(format!("http client: {e}")))
}

pub fn provider_error(provider: &str, what: &str, e: impl std::fmt::Display) -> AppError {
    AppError::BadRequest(format!("{provider} {what} failed: {e}"))
}
