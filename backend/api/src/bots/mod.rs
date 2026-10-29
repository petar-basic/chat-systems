pub mod models;
pub mod repo;
pub mod routes;

use std::sync::Arc;

use axum::extract::{FromRequestParts, MatchedPath, Request, State};
use axum::http::request::Parts;
use axum::http::Method;
use axum::middleware::Next;
use axum::response::Response;

use shared_common::errors::{AppError, AppResult};

use crate::audit::{self, AuditAction, AuditEntry};
use crate::middleware::AuthUser;
use crate::state::AppState;
use models::{BotToken, Scope};

pub const TOKEN_PREFIX: &str = "cbot-";

pub fn generate_token() -> String {
    use base64::Engine;
    use rand::RngExt;
    let mut bytes = [0u8; 32];
    rand::rng().fill(&mut bytes[..]);
    format!(
        "{TOKEN_PREFIX}{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    )
}

pub fn hash_token(token: &str) -> String {
    crate::scim::repo::hash_token(token)
}

pub struct Caller(AuthUser);

impl Caller {
    pub fn scoped(self, scope: Scope) -> AppResult<AuthUser> {
        match &self.0.bot {
            Some(bot) if !bot.allows(scope) => Err(AppError::Forbidden(format!(
                "This bot token does not have the {} scope",
                scope.as_str()
            ))),
            _ => Ok(self.0),
        }
    }

    pub fn user_id(&self) -> uuid::Uuid {
        self.0.user_id
    }

    pub fn bot(&self) -> Option<&BotToken> {
        self.0.bot.as_ref()
    }
}

impl<S> FromRequestParts<S> for Caller
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthUser>()
            .cloned()
            .map(Caller)
            .ok_or_else(|| AppError::Unauthorized("Not authenticated".into()))
    }
}

pub fn history_scope(channel: &crate::workspace::models::Channel) -> Scope {
    match channel.channel_type {
        crate::workspace::models::ChannelType::Public => Scope::ChannelsHistory,
        _ => Scope::GroupsHistory,
    }
}

pub async fn audit_bot_writes(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    let bot = request
        .extensions()
        .get::<AuthUser>()
        .and_then(|auth| auth.bot.clone().map(|bot| (auth.user_id, bot)));
    let writes = !matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    );
    let (Some((bot_user_id, bot)), true) = (bot, writes) else {
        return next.run(request).await;
    };
    let method = request.method().to_string();
    let route = request.extensions().get::<MatchedPath>().map_or_else(
        || request.uri().path().to_string(),
        |p| p.as_str().to_string(),
    );

    let response = next.run(request).await;
    if response.status().is_success() {
        audit::record(
            &state,
            AuditEntry::new(AuditAction::BotAppCalled, bot_user_id)
                .workspace(bot.workspace_id)
                .resource(bot.app_id)
                .details(serde_json::json!({ "method": method, "route": route })),
        )
        .await;
    }
    response
}
