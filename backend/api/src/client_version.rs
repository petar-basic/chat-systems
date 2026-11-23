use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use semver::Version;
use serde::Serialize;

pub const API_VERSION: u32 = 1;
pub const MIN_CLIENT_VERSION: Version = Version::new(5, 0, 0);
pub const HEADER: &str = "x-client-version";

#[derive(Serialize)]
struct NeedsNewerClient {
    error: &'static str,
    min_client_version: String,
}

pub fn is_served(header: Option<&str>) -> bool {
    header
        .and_then(|value| Version::parse(value.trim()).ok())
        .is_none_or(|version| version >= MIN_CLIENT_VERSION)
}

pub async fn refuse_old_clients(request: Request, next: Next) -> Response {
    let header = request
        .headers()
        .get(HEADER)
        .and_then(|value| value.to_str().ok());
    if is_served(header) {
        return next.run(request).await;
    }
    (
        StatusCode::UPGRADE_REQUIRED,
        Json(NeedsNewerClient {
            error: "This server needs a newer app",
            min_client_version: MIN_CLIENT_VERSION.to_string(),
        }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_client_without_a_version_is_served() {
        assert!(is_served(None));
        assert!(is_served(Some("dev")));
        assert!(is_served(Some("lw3k9x")));
    }

    #[test]
    fn a_client_is_served_from_the_oldest_version_on() {
        assert!(is_served(Some("5.0.0")));
        assert!(is_served(Some(" 5.2.1 ")));
        assert!(is_served(Some("6.0.0")));
    }

    #[test]
    fn an_older_client_is_refused() {
        assert!(!is_served(Some("4.8.1")));
        assert!(!is_served(Some("5.0.0-beta.1")));
    }
}
