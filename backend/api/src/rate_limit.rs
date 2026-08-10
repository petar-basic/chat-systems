use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::Method;
use axum::middleware::Next;
use axum::response::Response;
use redis::aio::ConnectionManager;

use shared_common::errors::{AppError, AppResult};

use crate::middleware::AuthUser;
use crate::state::AppState;

/// What to do when Redis itself is unreachable. Failing open is right for
/// sending a message — a Redis blip should not stop the company talking. It is
/// wrong for `/auth/login`, where the limiter is the only thing between an
/// attacker and unlimited password guesses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimiterFailure {
    Open,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    pub name: &'static str,
    pub max: u64,
    pub window_secs: u64,
}

/// One global number is simultaneously too loose for invites and too tight for
/// somebody reacting through a busy thread, so the budget follows the action.
const MESSAGE: Budget = Budget {
    name: "message",
    max: 120,
    window_secs: 60,
};
const REACTION: Budget = Budget {
    name: "reaction",
    max: 240,
    window_secs: 60,
};
const INVITE: Budget = Budget {
    name: "invite",
    max: 20,
    window_secs: 3600,
};
const WORKSPACE: Budget = Budget {
    name: "workspace",
    max: 5,
    window_secs: 3600,
};
const CHANNEL: Budget = Budget {
    name: "channel",
    max: 30,
    window_secs: 3600,
};
const DEFAULT: Budget = Budget {
    name: "default",
    max: 120,
    window_secs: 60,
};

pub fn budget_for(method: &Method, path: &str) -> Option<Budget> {
    if !matches!(
        *method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    ) {
        return None;
    }

    let budget = if path.contains("/reactions") {
        REACTION
    } else if path.ends_with("/invites") {
        INVITE
    } else if path == "/api/workspaces" {
        WORKSPACE
    } else if path.ends_with("/channels") {
        CHANNEL
    } else if path.ends_with("/messages") || path.ends_with("/thread") {
        MESSAGE
    } else {
        DEFAULT
    };

    Some(budget)
}

pub async fn enforce(
    conn: &mut ConnectionManager,
    key: &str,
    max: u64,
    window_secs: u64,
    on_failure: LimiterFailure,
) -> AppResult<()> {
    let script = redis::Script::new(
        r"
        local count = redis.call('INCR', KEYS[1])
        if count == 1 then
            redis.call('EXPIRE', KEYS[1], tonumber(ARGV[1]))
        end
        return count
        ",
    );

    let count: u64 = match script.key(key).arg(window_secs).invoke_async(conn).await {
        Ok(count) => count,
        Err(e) => {
            metrics::counter!(
                "rate_limit_backend_failures_total",
                "policy" => match on_failure {
                    LimiterFailure::Open => "open",
                    LimiterFailure::Closed => "closed",
                },
            )
            .increment(1);
            tracing::warn!("rate limit check failed ({:?}): {}", on_failure, e);
            return match on_failure {
                LimiterFailure::Open => Ok(()),
                LimiterFailure::Closed => Err(AppError::ServiceUnavailable(
                    "Authentication is temporarily unavailable. Please try again.".into(),
                )),
            };
        }
    };

    if count > max {
        return Err(AppError::TooManyRequests {
            message: "Too many requests. Please try again later.".into(),
            retry_after_secs: window_secs,
        });
    }

    Ok(())
}

pub async fn write_rate_limit(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let budget = budget_for(request.method(), request.uri().path());

    if let (Some(budget), Some(auth)) = (budget, request.extensions().get::<AuthUser>()) {
        let key = format!("rate_limit:write:{}:{}", budget.name, auth.user_id);
        let mut conn = state.redis.clone();
        enforce(
            &mut conn,
            &key,
            budget.max,
            budget.window_secs,
            LimiterFailure::Open,
        )
        .await?;
    }

    Ok(next.run(request).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn class(method: Method, path: &str) -> Option<&'static str> {
        budget_for(&method, path).map(|b| b.name)
    }

    #[test]
    fn reads_are_never_limited() {
        assert_eq!(class(Method::GET, "/api/workspaces"), None);
        assert_eq!(class(Method::GET, "/api/channels/abc/messages"), None);
    }

    #[test]
    fn every_mutating_route_lands_in_a_class() {
        for path in [
            "/api/workspaces",
            "/api/workspaces/abc/invites",
            "/api/workspaces/abc/channels",
            "/api/channels/abc/messages",
            "/api/messages/abc/thread",
            "/api/messages/abc/reactions",
            "/api/conversations/abc/messages",
            "/api/users/me/password",
            "/api/files/upload/abc",
            "/api/scheduled-messages/abc",
        ] {
            assert!(
                class(Method::POST, path).is_some(),
                "{path} has no rate-limit class"
            );
        }
    }

    #[test]
    fn the_abusable_actions_get_their_own_budget() {
        assert_eq!(class(Method::POST, "/api/workspaces"), Some("workspace"));
        assert_eq!(
            class(Method::POST, "/api/workspaces/abc/invites"),
            Some("invite")
        );
        assert_eq!(
            class(Method::POST, "/api/workspaces/abc/channels"),
            Some("channel")
        );
        assert_eq!(
            class(Method::POST, "/api/messages/abc/reactions"),
            Some("reaction")
        );
        assert_eq!(
            class(Method::POST, "/api/channels/abc/messages"),
            Some("message")
        );
        assert_eq!(
            class(Method::POST, "/api/conversations/abc/messages"),
            Some("message")
        );
        assert_eq!(class(Method::DELETE, "/api/files/abc"), Some("default"));
    }
}
