use std::sync::{Arc, LazyLock};

use axum::http::header::CONTENT_TYPE;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi};
use utoipa_axum::router::OpenApiRouter;

use crate::state::AppState;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Chat Systems API",
        description = "Every error response carries `{\"error\": \"<message>\"}` with the HTTP status saying what kind of error it is."
    ),
    servers((url = "/api")),
    components(schemas(crate::messaging::models::SearchScope)),
    security(("bearer" = [])),
    modifiers(&BearerAuth),
    tags(
        (name = "messages", description = "Channel messages, threads, pins, reactions and search"),
        (name = "workspaces", description = "Workspaces, membership, invites and the audit log"),
        (name = "channels", description = "Channels, their members, notification settings and bookmarks"),
        (name = "conversations", description = "Direct and group conversations and their messages"),
        (name = "notifications", description = "In-app notifications, do-not-disturb and email preferences"),
        (name = "auth", description = "Sessions, invites, registration and password recovery"),
        (name = "users", description = "The signed-in user's own profile and status"),
        (name = "instance", description = "What this instance says about itself"),
        (name = "hooks", description = "Incoming and outgoing webhooks, bots and slash commands"),
        (name = "reminders", description = "Reminders"),
        (name = "files", description = "Uploads and their metadata"),
        (name = "huddles", description = "Voice and video huddles"),
        (name = "groups", description = "User groups that can be @-mentioned"),
        (name = "emoji", description = "Custom emoji"),
        (name = "saved", description = "Saved messages"),
        (name = "scheduled", description = "Messages scheduled for later"),
        (name = "admin", description = "Instance administration"),
        (name = "commands", description = "Slash commands"),
        (name = "exports", description = "Data exports"),
        (name = "push", description = "Web Push subscriptions"),
        (name = "retention", description = "Retention policies"),
        (name = "totp", description = "Two-factor enrolment"),
        (name = "slack-import", description = "Importing a Slack export")
    )
)]
struct ApiDoc;

struct BearerAuth;

impl Modify for BearerAuth {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        openapi
            .components
            .get_or_insert_default()
            .add_security_scheme(
                "bearer",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .description(Some(
                            "Same-origin clients are authenticated by the httpOnly cookie instead.",
                        ))
                        .build(),
                ),
            );
    }
}

/// Every feature router that carries its own OpenAPI paths. `build_app` mounts
/// these behind auth; `spec` reads their documentation. One list, two uses.
pub fn typed_routes() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .merge(crate::auth::routes::router())
        .merge(crate::messaging::routes::router())
        .merge(crate::workspace::routes::router())
        .merge(crate::conversations::routes::router())
        .merge(crate::notifications::routes::router())
        .merge(crate::hooks::routes::router())
        .merge(crate::files::routes::router())
        .merge(crate::huddle::routes::router())
        .merge(crate::groups::routes::router())
        .merge(crate::emoji::routes::router())
        .merge(crate::saved::routes::router())
        .merge(crate::scheduled::routes::router())
        .merge(crate::admin::routes::router())
        .merge(crate::commands::routes::router())
        .merge(crate::export::routes::router())
        .merge(crate::push::routes::router())
        .merge(crate::retention::routes::router())
        .merge(crate::auth::totp_routes::router())
        .merge(crate::slack_import::routes::router())
}

/// Reachable without a session: signing in, accepting an invite, and what an
/// instance says about itself before anyone has logged in.
pub fn public_routes() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .merge(crate::auth::routes::public_router())
        .merge(crate::hooks::routes::public_router())
}

pub fn spec() -> utoipa::openapi::OpenApi {
    OpenApiRouter::<Arc<AppState>>::with_openapi(ApiDoc::openapi())
        .merge(public_routes())
        .merge(typed_routes())
        .into_openapi()
}

static SPEC_JSON: LazyLock<String> =
    LazyLock::new(|| serde_json::to_string(&spec()).expect("the OpenAPI document serialises"));

pub fn router() -> Router {
    Router::new().route("/api/openapi.json", get(serve))
}

async fn serve() -> impl IntoResponse {
    ([(CONTENT_TYPE, "application/json")], SPEC_JSON.as_str())
}

#[cfg(test)]
mod tests {
    #[test]
    fn spec_covers_the_messaging_routes() {
        let json = serde_json::to_value(super::spec()).unwrap();
        let paths = json["paths"].as_object().unwrap();
        assert!(paths["/channels/{ch_id}/messages"]["get"].is_object());
        assert!(paths["/channels/{ch_id}/messages"]["post"].is_object());
        assert!(paths["/messages/{msg_id}/reactions/{emoji}"]["delete"].is_object());
        assert!(paths["/search"]["get"].is_object());
        assert_eq!(json["servers"][0]["url"], "/api");
        assert!(json["components"]["securitySchemes"]["bearer"].is_object());

        let schemas = json["components"]["schemas"].as_object().unwrap();
        for name in [
            "Message",
            "Reaction",
            "MessageEdit",
            "MessageWithReactions",
            "SendMessageRequest",
            "SearchResponse",
            "StatusResponse",
        ] {
            assert!(schemas.contains_key(name), "schema {name} is missing");
        }
    }

    #[test]
    fn every_path_parameter_in_a_template_is_declared() {
        let json = serde_json::to_value(super::spec()).unwrap();
        for (path, item) in json["paths"].as_object().unwrap() {
            let names: Vec<&str> = path
                .split('/')
                .filter_map(|seg| seg.strip_prefix('{').and_then(|s| s.strip_suffix('}')))
                .collect();
            for (method, op) in item.as_object().unwrap() {
                let declared: Vec<&str> = op["parameters"]
                    .as_array()
                    .map(|ps| {
                        ps.iter()
                            .filter(|p| p["in"] == "path")
                            .filter_map(|p| p["name"].as_str())
                            .collect()
                    })
                    .unwrap_or_default();
                for name in &names {
                    assert!(
                        declared.contains(name),
                        "{method} {path}: path parameter {name} is not declared"
                    );
                }
            }
        }
    }

    #[test]
    fn operation_ids_are_unique() {
        let json = serde_json::to_value(super::spec()).unwrap();
        let mut seen = std::collections::HashMap::new();
        for (path, item) in json["paths"].as_object().unwrap() {
            for (method, op) in item.as_object().unwrap() {
                let id = op["operationId"].as_str().unwrap().to_string();
                if let Some(first) = seen.insert(id.clone(), format!("{method} {path}")) {
                    panic!("operationId {id} is used by both {first} and {method} {path}");
                }
            }
        }
    }
}
