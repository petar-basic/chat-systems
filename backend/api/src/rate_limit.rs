use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::Method;
use axum::middleware::Next;
use axum::response::Response;
use redis::aio::ConnectionManager;

use shared_common::errors::{AppError, AppResult};

use crate::middleware::AuthUser;
use crate::state::AppState;

const WRITE_LIMIT: u64 = 120;
const WRITE_WINDOW_SECS: u64 = 60;

pub async fn enforce(
    conn: &mut ConnectionManager,
    key: &str,
    max: u64,
    window_secs: u64,
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
            tracing::warn!("rate limit check failed (failing open): {}", e);
            return Ok(());
        }
    };

    if count > max {
        return Err(AppError::TooManyRequests(
            "Too many requests. Please try again later.".into(),
        ));
    }

    Ok(())
}

pub async fn write_rate_limit(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let is_write = matches!(
        *request.method(),
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    );

    if is_write {
        if let Some(auth) = request.extensions().get::<AuthUser>() {
            let key = format!("rate_limit:write:{}", auth.user_id);
            let mut conn = state.redis.clone();
            enforce(&mut conn, &key, WRITE_LIMIT, WRITE_WINDOW_SECS).await?;
        }
    }

    Ok(next.run(request).await)
}
