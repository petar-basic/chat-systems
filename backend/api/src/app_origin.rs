use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::header::{COOKIE, ORIGIN, SET_COOKIE};
use axum::middleware::Next;
use axum::response::Response;
use shared_common::errors::AppError;
use shared_common::paths::API_V1;

use crate::state::AppState;

const MACHINE_ROUTES: [&str; 5] = [
    "/scim/",
    "/hooks/incoming/",
    "/workflows/webhook/",
    "/commands/responses/",
    "/interactions/responses/",
];

pub fn app_may_call(path: &str) -> bool {
    path.strip_prefix(API_V1)
        .is_some_and(|route| !MACHINE_ROUTES.iter().any(|m| route.starts_with(m)))
}

pub async fn tokens_not_cookies(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let from_app = request
        .headers()
        .get(ORIGIN)
        .and_then(|origin| origin.to_str().ok())
        .is_some_and(|origin| shared_common::cors::listed(&state.config.app_origins, origin));
    if !from_app {
        return Ok(next.run(request).await);
    }
    if request.headers().contains_key(COOKIE) {
        return Err(AppError::BadRequest(
            "The app signs in with a token, so it never sends a cookie".into(),
        ));
    }
    let mut response = next.run(request).await;
    response.headers_mut().remove(SET_COOKIE);
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_app_reaches_the_api_but_not_the_machine_routes() {
        assert!(app_may_call("/api/v1/workspaces"));
        assert!(app_may_call("/api/v1/auth/login"));
        assert!(app_may_call("/api/v1/files/download/a/b.png"));
        assert!(!app_may_call("/api/v1/scim/v2/Users"));
        assert!(!app_may_call("/api/v1/hooks/incoming/abc"));
        assert!(!app_may_call("/api/v1/workflows/webhook/abc"));
        assert!(!app_may_call("/api/v1/commands/responses/abc"));
        assert!(!app_may_call("/api/v1/interactions/responses/abc"));
        assert!(!app_may_call("/openapi.json"));
        assert!(!app_may_call("/metrics"));
    }
}
