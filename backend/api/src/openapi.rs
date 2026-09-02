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
    tags((name = "messages", description = "Channel messages, threads, pins, reactions and search"))
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

pub fn spec() -> utoipa::openapi::OpenApi {
    OpenApiRouter::<Arc<AppState>>::with_openapi(ApiDoc::openapi())
        .merge(crate::messaging::routes::router())
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
}
