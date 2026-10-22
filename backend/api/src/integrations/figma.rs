use chrono::{DateTime, Utc};
use serde_json::Value;

use shared_common::errors::{AppError, AppResult};

use super::http::{self, TokenSet};
use crate::messaging::models::{Unfurl, UnfurlField};

pub const SCOPES: [&str; 2] = ["file_metadata:read", "current_user:read"];
const FILE_KINDS: [&str; 9] = [
    "file", "design", "proto", "board", "slides", "deck", "site", "make", "buzz",
];

pub struct Figma {
    pub web_base: String,
    pub api_base: String,
}

fn provider_error(what: &str, e: impl std::fmt::Display) -> AppError {
    http::provider_error("Figma", what, e)
}

impl Figma {
    pub fn authorize_url(
        &self,
        client_id: &str,
        redirect_uri: &str,
        state: &str,
        challenge: &str,
    ) -> String {
        let mut url = reqwest::Url::parse(&format!("{}/oauth", self.web_base))
            .expect("the Figma web base is a valid url");
        url.query_pairs_mut()
            .append_pair("client_id", client_id)
            .append_pair("redirect_uri", redirect_uri)
            .append_pair("scope", &SCOPES.join(","))
            .append_pair("state", state)
            .append_pair("response_type", "code")
            .append_pair("code_challenge", challenge)
            .append_pair("code_challenge_method", "S256");
        url.to_string()
    }

    async fn token(
        &self,
        path: &str,
        client_id: &str,
        client_secret: &str,
        form: &[(&str, &str)],
    ) -> AppResult<TokenSet> {
        let response = http::client()?
            .post(format!("{}{path}", self.api_base))
            .basic_auth(client_id, Some(client_secret))
            .form(form)
            .send()
            .await
            .map_err(|e| provider_error("token request", e))?;
        if !response.status().is_success() {
            return Err(provider_error("token request", response.status()));
        }
        response
            .json::<TokenSet>()
            .await
            .map_err(|e| provider_error("token response", e))
    }

    pub async fn exchange(
        &self,
        client_id: &str,
        client_secret: &str,
        code: &str,
        redirect_uri: &str,
        verifier: &str,
    ) -> AppResult<TokenSet> {
        self.token(
            "/v1/oauth/token",
            client_id,
            client_secret,
            &[
                ("redirect_uri", redirect_uri),
                ("code", code),
                ("grant_type", "authorization_code"),
                ("code_verifier", verifier),
            ],
        )
        .await
    }

    pub async fn refresh(
        &self,
        client_id: &str,
        client_secret: &str,
        refresh_token: &str,
    ) -> AppResult<TokenSet> {
        self.token(
            "/v1/oauth/refresh",
            client_id,
            client_secret,
            &[("refresh_token", refresh_token)],
        )
        .await
    }

    async fn get(&self, path: &str, token: &str) -> AppResult<Option<Value>> {
        let response = http::client()?
            .get(format!("{}{path}", self.api_base))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| provider_error("request", e))?;
        match response.status() {
            status if status.is_success() => response
                .json::<Value>()
                .await
                .map(Some)
                .map_err(|e| provider_error("response", e)),
            reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::FORBIDDEN => Ok(None),
            status => Err(provider_error("request", status)),
        }
    }

    pub async fn account_name(&self, token: &str) -> AppResult<String> {
        let me = self.get("/v1/me", token).await?.unwrap_or(Value::Null);
        let handle = me.get("handle").and_then(Value::as_str);
        let email = me.get("email").and_then(Value::as_str);
        Ok(match (handle, email) {
            (Some(handle), Some(email)) => format!("{handle} ({email})"),
            (Some(handle), None) => handle.to_string(),
            (None, Some(email)) => email.to_string(),
            (None, None) => "Figma account".to_string(),
        })
    }

    pub async fn file_card(&self, token: &str, key: &str, url: &str) -> AppResult<Option<Unfurl>> {
        Ok(self
            .get(&format!("/v1/files/{key}/meta"), token)
            .await?
            .and_then(|body| card_from_meta(&body, url)))
    }
}

pub fn match_file(url: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(url).ok()?;
    if parsed.scheme() != "https" || !matches!(parsed.host_str()?, "www.figma.com" | "figma.com") {
        return None;
    }
    let segments: Vec<&str> = parsed.path_segments()?.filter(|s| !s.is_empty()).collect();
    let key = match segments.as_slice() {
        [kind, _, "branch", branch, ..] if FILE_KINDS.contains(kind) => *branch,
        [kind, key, ..] if FILE_KINDS.contains(kind) => *key,
        _ => return None,
    };
    ((10..=64).contains(&key.len()) && key.chars().all(|c| c.is_ascii_alphanumeric()))
        .then(|| key.to_string())
}

fn kind_of(editor: &str) -> &'static str {
    match editor {
        "figma" => "Figma design",
        "figjam" => "FigJam board",
        "slides" => "Figma Slides",
        "buzz" => "Figma Buzz",
        "sites" => "Figma Sites",
        "make" => "Figma Make",
        _ => "Figma file",
    }
}

fn https(value: Option<String>) -> Option<String> {
    value.filter(|u| u.starts_with("https://"))
}

fn card_from_meta(body: &Value, url: &str) -> Option<Unfurl> {
    let file = body.get("file")?;
    let text = |pointer: &str| {
        file.pointer(pointer)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    let title = text("/name")?;

    let mut fields = Vec::new();
    if let Some(project) = text("/folder_name") {
        fields.push(UnfurlField {
            label: "Project".into(),
            value: project,
            ..UnfurlField::default()
        });
    }
    if let Some(touched) = text("/last_touched_at").and_then(|t| t.parse::<DateTime<Utc>>().ok()) {
        let by = text("/last_touched_by/handle");
        fields.push(UnfurlField {
            label: "Last edited".into(),
            value: match &by {
                Some(name) => format!("{} by {name}", touched.format("%b %-d, %Y")),
                None => touched.format("%b %-d, %Y").to_string(),
            },
            avatar_url: by.and(https(text("/last_touched_by/img_url"))),
            ..UnfurlField::default()
        });
    }

    Some(Unfurl {
        url: url.to_string(),
        title,
        site_name: Some("Figma".into()),
        image_url: https(text("/thumbnail_url")),
        kind: Some("app".into()),
        provider: Some("figma".into()),
        subtitle: Some(kind_of(text("/editorType").as_deref().unwrap_or_default()).into()),
        fields,
        fetched_at: Some(Utc::now()),
        ..Unfurl::default()
    })
}

#[cfg(test)]
mod tests {
    use super::{card_from_meta, match_file};
    use serde_json::json;

    const KEY: &str = "AbCdEfGhIjKlMnOpQrStUv";

    #[test]
    fn matches_every_figma_file_kind() {
        for url in [
            format!("https://www.figma.com/design/{KEY}/Checkout?node-id=1-2"),
            format!("https://www.figma.com/file/{KEY}/Checkout"),
            format!("https://www.figma.com/proto/{KEY}/Checkout?page-id=0%3A1"),
            format!("https://www.figma.com/board/{KEY}/Retro"),
            format!("https://www.figma.com/slides/{KEY}/Kickoff"),
            format!("https://figma.com/design/{KEY}"),
        ] {
            assert_eq!(match_file(&url).as_deref(), Some(KEY), "{url}");
        }
    }

    #[test]
    fn a_branch_link_previews_the_branch() {
        assert_eq!(
            match_file(&format!(
                "https://www.figma.com/design/{KEY}/branch/BrAnChKeY1234567890xy/Checkout"
            ))
            .as_deref(),
            Some("BrAnChKeY1234567890xy")
        );
    }

    #[test]
    fn leaves_everything_else_alone() {
        for url in [
            format!("http://www.figma.com/design/{KEY}/x"),
            format!("https://www.figma.com.evil.test/design/{KEY}/x"),
            "https://www.figma.com/community/file/123456789012345".into(),
            "https://www.figma.com/files/recents-and-sharing".into(),
            "https://www.figma.com/design/short/x".into(),
            "https://www.figma.com/design/has-dash-in-key-1234/x".into(),
        ] {
            assert!(match_file(&url).is_none(), "{url}");
        }
    }

    #[test]
    fn maps_file_meta_to_a_card() {
        let body = json!({ "file": {
            "name": "Checkout redesign",
            "folder_name": "Payments",
            "editorType": "figma",
            "last_touched_at": "2026-10-21T09:30:00Z",
            "last_touched_by": { "handle": "Lazar", "img_url": "https://s3.test/l.png" },
            "thumbnail_url": "https://s3.test/thumb.png"
        }});
        let card = card_from_meta(&body, "u").expect("card");
        assert_eq!(card.title, "Checkout redesign");
        assert_eq!(card.subtitle.as_deref(), Some("Figma design"));
        assert_eq!(card.image_url.as_deref(), Some("https://s3.test/thumb.png"));
        let fields: Vec<(&str, &str, Option<&str>)> = card
            .fields
            .iter()
            .map(|f| (f.label.as_str(), f.value.as_str(), f.avatar_url.as_deref()))
            .collect();
        assert_eq!(
            fields,
            vec![
                ("Project", "Payments", None),
                (
                    "Last edited",
                    "Oct 21, 2026 by Lazar",
                    Some("https://s3.test/l.png")
                )
            ]
        );
    }

    #[test]
    fn a_board_without_extras_still_gets_a_card() {
        let body = json!({ "file": {
            "name": "Retro",
            "editorType": "figjam",
            "thumbnail_url": "http://insecure.test/t.png"
        }});
        let card = card_from_meta(&body, "u").expect("card");
        assert_eq!(card.subtitle.as_deref(), Some("FigJam board"));
        assert_eq!(card.image_url, None);
        assert!(card.fields.is_empty());
        assert!(card_from_meta(&json!({ "name": "flat" }), "u").is_none());
    }
}
