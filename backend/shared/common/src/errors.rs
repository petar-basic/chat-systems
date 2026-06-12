use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Validation: {0}")]
    Validation(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Internal: {0}")]
    Internal(String),

    #[error("Database: {0}")]
    Database(String),

    #[error("Too many requests: {0}")]
    TooManyRequests(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg.clone()),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg.clone()),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Validation(msg) => (StatusCode::UNPROCESSABLE_ENTITY, msg.clone()),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, msg.clone()),
            AppError::Internal(msg) => {
                tracing::error!(error = %msg, "internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal server error".to_string(),
                )
            }
            AppError::Database(msg) => {
                tracing::error!(error = %msg, "database error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal server error".to_string(),
                )
            }
            AppError::TooManyRequests(msg) => (StatusCode::TOO_MANY_REQUESTS, msg.clone()),
        };

        let body = json!({ "error": message });
        (status, Json(body)).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        AppError::Database(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::response::IntoResponse;
    use std::future::Future;
    use std::pin::Pin;
    use std::task::{Context, Poll, Waker};

    fn block_on<F: Future>(future: F) -> F::Output {
        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);
        let mut future = Box::pin(future);
        loop {
            match Pin::new(&mut future).poll(&mut cx) {
                Poll::Ready(out) => return out,
                Poll::Pending => std::hint::spin_loop(),
            }
        }
    }

    fn render(error: AppError) -> (StatusCode, String) {
        let response = error.into_response();
        let status = response.status();
        let bytes =
            block_on(to_bytes(response.into_body(), usize::MAX)).expect("body should collect");
        let body = String::from_utf8(bytes.to_vec()).expect("body should be valid utf-8");
        (status, body)
    }

    #[test]
    fn internal_error_returns_500_without_leaking_secret() {
        let secret = "connection string postgres://u:p@host/db";
        let (status, body) = render(AppError::Internal(secret.to_string()));

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(
            !body.contains(secret),
            "Internal error body leaked the secret: {body}"
        );
        assert!(
            body.contains("internal server error"),
            "expected opaque message, got: {body}"
        );
    }

    #[test]
    fn database_error_returns_500_without_leaking_secret() {
        let secret = "ERROR: relation \"secret_table\" does not exist";
        let (status, body) = render(AppError::Database(secret.to_string()));

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(
            !body.contains(secret),
            "Database error body leaked the secret: {body}"
        );
        assert!(
            body.contains("internal server error"),
            "expected opaque message, got: {body}"
        );
    }

    #[test]
    fn not_found_surfaces_message_with_404() {
        let msg = "workspace 1234 not found";
        let (status, body) = render(AppError::NotFound(msg.to_string()));

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(
            body.contains(msg),
            "NotFound message should be surfaced, got: {body}"
        );
    }

    #[test]
    fn bad_request_surfaces_message_with_400() {
        let msg = "missing field: channel_id";
        let (status, body) = render(AppError::BadRequest(msg.to_string()));

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            body.contains(msg),
            "BadRequest message should be surfaced, got: {body}"
        );
    }
}
