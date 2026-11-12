use std::sync::Arc;

use askama::Template;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Html;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use crate::notifications::text::{self, Words};
use crate::profile::models::Language;
use crate::sealed::{self, Keyring};
use crate::state::AppState;

type HmacSha256 = Hmac<Sha256>;

const MAC_BYTES: usize = 16;

fn mac(keyring: &Keyring, user_id: Uuid, workspace_id: Uuid) -> HmacSha256 {
    let mut mac = HmacSha256::new_from_slice(&keyring.derive(sealed::EMAIL_UNSUBSCRIBE))
        .expect("HMAC accepts any key length");
    mac.update(b"mention-emails:");
    mac.update(user_id.as_bytes());
    mac.update(workspace_id.as_bytes());
    mac
}

pub fn token_for(keyring: &Keyring, user_id: Uuid, workspace_id: Uuid) -> String {
    let mut bytes = Vec::with_capacity(32 + MAC_BYTES);
    bytes.extend_from_slice(user_id.as_bytes());
    bytes.extend_from_slice(workspace_id.as_bytes());
    bytes.extend_from_slice(
        &mac(keyring, user_id, workspace_id).finalize().into_bytes()[..MAC_BYTES],
    );
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn url_for(public_url: &str, keyring: &Keyring, user_id: Uuid, workspace_id: Uuid) -> String {
    format!(
        "{}/api/email/unsubscribe/{}",
        public_url.trim_end_matches('/'),
        token_for(keyring, user_id, workspace_id)
    )
}

fn parse(keyring: &Keyring, token: &str) -> Option<(Uuid, Uuid)> {
    let bytes = URL_SAFE_NO_PAD.decode(token.trim()).ok()?;
    if bytes.len() != 32 + MAC_BYTES {
        return None;
    }
    let user_id = Uuid::from_slice(&bytes[..16]).ok()?;
    let workspace_id = Uuid::from_slice(&bytes[16..32]).ok()?;
    mac(keyring, user_id, workspace_id)
        .verify_truncated_left(&bytes[32..])
        .ok()?;
    Some((user_id, workspace_id))
}

pub fn public_router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new().routes(routes!(email_unsubscribe_page, email_unsubscribe))
}

#[derive(Template)]
#[template(path = "email/unsubscribe.html")]
struct Page<'a> {
    lang: &'a str,
    title: String,
    heading: String,
    lines: Vec<String>,
    button: Option<String>,
    link: Option<(String, String)>,
}

enum Outcome {
    Confirm,
    Done,
}

#[utoipa::path(get, path = "/email/unsubscribe/{token}", tag = "notifications", responses((status = 200, description = "A page that asks whether to stop mention emails", content_type = "text/html", body = String), (status = 400, description = "The link is not one this server made", content_type = "text/html", body = String)), security(()))]
async fn email_unsubscribe_page(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
) -> AppResult<(StatusCode, Html<String>)> {
    respond(&state, &token, Outcome::Confirm).await
}

#[utoipa::path(post, path = "/email/unsubscribe/{token}", tag = "notifications", responses((status = 200, description = "Mention emails are off", content_type = "text/html", body = String), (status = 400, description = "The link is not one this server made", content_type = "text/html", body = String)), security(()))]
async fn email_unsubscribe(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
) -> AppResult<(StatusCode, Html<String>)> {
    respond(&state, &token, Outcome::Done).await
}

async fn respond(
    state: &AppState,
    token: &str,
    outcome: Outcome,
) -> AppResult<(StatusCode, Html<String>)> {
    let words = text::words(Language::En);
    let found = match parse(&state.config.secrets_keys, token) {
        Some((user_id, workspace_id)) => state
            .workspace_service
            .repo
            .find_workspace_by_id(workspace_id)
            .await?
            .map(|workspace| (user_id, workspace)),
        None => None,
    };
    let Some((user_id, workspace)) = found else {
        let page = Page {
            lang: words.lang(),
            title: words.get("email.unsubscribePage.title", &[]),
            heading: words.get("email.unsubscribePage.title", &[]),
            lines: vec![words.get("email.unsubscribePage.invalid", &[])],
            button: None,
            link: None,
        };
        return Ok((StatusCode::BAD_REQUEST, Html(render(&page)?)));
    };
    let name = workspace.name.as_str();
    let open = Some((
        words.get("email.unsubscribePage.open", &[("workspace", name)]),
        format!(
            "{}/app/{}?preferences",
            state.config.public_url.trim_end_matches('/'),
            workspace.id
        ),
    ));
    let page = match outcome {
        Outcome::Confirm => confirm_page(&words, name),
        Outcome::Done => {
            state
                .notification_repo
                .set_mention_emails(workspace.id, user_id, false)
                .await?;
            Page {
                lang: words.lang(),
                title: words.get("email.unsubscribePage.title", &[]),
                heading: words.get("email.unsubscribePage.done", &[("workspace", name)]),
                lines: vec![words.get("email.unsubscribePage.again", &[])],
                button: None,
                link: open,
            }
        }
    };
    Ok((StatusCode::OK, Html(render(&page)?)))
}

fn confirm_page<'a>(words: &Words<'a>, workspace: &str) -> Page<'a> {
    Page {
        lang: words.lang(),
        title: words.get("email.unsubscribePage.title", &[]),
        heading: words.get("email.unsubscribePage.confirm", &[("workspace", workspace)]),
        lines: vec![words.get("email.unsubscribePage.explain", &[])],
        button: Some(words.get("email.unsubscribePage.button", &[])),
        link: None,
    }
}

fn render(page: &Page<'_>) -> AppResult<String> {
    page.render()
        .map_err(|e| AppError::Internal(format!("Failed to render the page: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;

    #[test]
    fn a_token_names_its_person_and_workspace_and_nothing_else_passes() {
        let keyring = AppConfig::for_tests().secrets_keys;
        let user = Uuid::new_v4();
        let workspace = Uuid::new_v4();
        let token = token_for(&keyring, user, workspace);
        assert_eq!(parse(&keyring, &token), Some((user, workspace)));

        let mut bytes = URL_SAFE_NO_PAD.decode(&token).unwrap();
        bytes[16..32].copy_from_slice(Uuid::new_v4().as_bytes());
        assert_eq!(parse(&keyring, &URL_SAFE_NO_PAD.encode(&bytes)), None);
        assert_eq!(parse(&keyring, "not-a-token"), None);

        let other = Keyring::parse("other:b3RoZXItc2VjcmV0cy1rZXktMDEyMzQ1Njc4OWFiY2Q=").unwrap();
        assert_eq!(parse(&other, &token), None);
    }
}
