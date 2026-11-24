use std::sync::Arc;

use axum::http::{HeaderValue, Method};
use tower_http::cors::{AllowCredentials, AllowOrigin, CorsLayer};

pub const APP_ORIGINS: &str = "tauri://localhost,http://tauri.localhost,https://tauri.localhost";

fn origin_list(raw: &str) -> Vec<HeaderValue> {
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<HeaderValue>().ok())
        .collect()
}

pub fn listed(raw: &str, origin: &str) -> bool {
    raw.split(',')
        .map(str::trim)
        .any(|o| !o.is_empty() && o == origin)
}

pub fn cors_layer(
    web_origins: &str,
    app_origins: &str,
    app_may_call: fn(&str) -> bool,
) -> CorsLayer {
    let web = Arc::new(origin_list(web_origins));
    let app = origin_list(app_origins);
    let credentialed = Arc::clone(&web);

    CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(move |origin, parts| {
            web.contains(origin) || (app.contains(origin) && app_may_call(parts.uri.path()))
        }))
        .allow_credentials(AllowCredentials::predicate(move |origin, _| {
            credentialed.contains(origin)
        }))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            axum::http::header::AUTHORIZATION,
            axum::http::header::CONTENT_TYPE,
            axum::http::header::ACCEPT,
            axum::http::header::ORIGIN,
            axum::http::header::HeaderName::from_static("x-requested-with"),
            axum::http::header::HeaderName::from_static("x-client-version"),
        ])
        .expose_headers([axum::http::header::RETRY_AFTER])
        .max_age(std::time::Duration::from_secs(86400))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_origin_is_listed_only_exactly() {
        assert!(listed(APP_ORIGINS, "tauri://localhost"));
        assert!(listed(
            " https://a.example , tauri://localhost",
            "https://a.example"
        ));
        assert!(!listed(APP_ORIGINS, "tauri://localhost.evil.example"));
        assert!(!listed("", ""));
    }
}
