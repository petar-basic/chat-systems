use chrono::{DateTime, Utc};
use serde_json::Value;

use shared_common::errors::{AppError, AppResult};

use super::http::{self, TokenSet};
use crate::messaging::models::{Unfurl, UnfurlField};

pub const SCOPE: &str = "https://www.googleapis.com/auth/drive.metadata.readonly";
const FILE_FIELDS: &str =
    "name,mimeType,owners(displayName,photoLink),modifiedTime,lastModifyingUser(displayName)";

pub struct Google {
    pub accounts_base: String,
    pub oauth_base: String,
    pub api_base: String,
}

fn provider_error(what: &str, e: impl std::fmt::Display) -> AppError {
    http::provider_error("Google", what, e)
}

impl Google {
    pub fn authorize_url(
        &self,
        client_id: &str,
        redirect_uri: &str,
        state: &str,
        challenge: &str,
    ) -> String {
        let mut url = reqwest::Url::parse(&format!("{}/o/oauth2/v2/auth", self.accounts_base))
            .expect("the Google accounts base is a valid url");
        url.query_pairs_mut()
            .append_pair("client_id", client_id)
            .append_pair("redirect_uri", redirect_uri)
            .append_pair("response_type", "code")
            .append_pair("scope", SCOPE)
            .append_pair("access_type", "offline")
            .append_pair("prompt", "consent")
            .append_pair("include_granted_scopes", "true")
            .append_pair("state", state)
            .append_pair("code_challenge", challenge)
            .append_pair("code_challenge_method", "S256");
        url.to_string()
    }

    async fn token(&self, form: &[(&str, &str)]) -> AppResult<TokenSet> {
        let response = http::client()?
            .post(format!("{}/token", self.oauth_base))
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
        self.token(&[
            ("grant_type", "authorization_code"),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("code_verifier", verifier),
        ])
        .await
    }

    pub async fn refresh(
        &self,
        client_id: &str,
        client_secret: &str,
        refresh_token: &str,
    ) -> AppResult<TokenSet> {
        self.token(&[
            ("grant_type", "refresh_token"),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("refresh_token", refresh_token),
        ])
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
        let about = self
            .get(
                "/drive/v3/about?fields=user(displayName,emailAddress)",
                token,
            )
            .await?
            .unwrap_or(Value::Null);
        let name = about.pointer("/user/displayName").and_then(Value::as_str);
        let email = about.pointer("/user/emailAddress").and_then(Value::as_str);
        Ok(match (name, email) {
            (Some(name), Some(email)) => format!("{name} ({email})"),
            (Some(name), None) => name.to_string(),
            (None, Some(email)) => email.to_string(),
            (None, None) => "Google account".to_string(),
        })
    }

    pub async fn file_card(
        &self,
        token: &str,
        file_id: &str,
        url: &str,
    ) -> AppResult<Option<Unfurl>> {
        let path = format!("/drive/v3/files/{file_id}?fields={FILE_FIELDS}&supportsAllDrives=true");
        Ok(self
            .get(&path, token)
            .await?
            .and_then(|body| card_from_file(&body, url)))
    }
}

pub fn match_file(url: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(url).ok()?;
    if parsed.scheme() != "https" {
        return None;
    }
    let segments: Vec<&str> = parsed.path_segments()?.filter(|s| !s.is_empty()).collect();
    let id = match (parsed.host_str()?, segments.as_slice()) {
        ("docs.google.com", [kind, "d", id, ..])
            if matches!(
                *kind,
                "document" | "spreadsheets" | "presentation" | "forms" | "drawings"
            ) =>
        {
            id.to_string()
        }
        ("drive.google.com", ["file", "d", id, ..])
        | ("drive.google.com", ["drive", "folders", id, ..]) => id.to_string(),
        ("drive.google.com", ["drive", "u", _, "folders", id, ..]) => id.to_string(),
        ("drive.google.com", ["open"]) => parsed
            .query_pairs()
            .find(|(k, _)| k == "id")
            .map(|(_, v)| v.into_owned())?,
        _ => return None,
    };
    (id.len() >= 10
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'))
    .then_some(id)
}

fn kind_of(mime: &str) -> &'static str {
    match mime {
        "application/vnd.google-apps.document" => "Google Doc",
        "application/vnd.google-apps.spreadsheet" => "Google Sheet",
        "application/vnd.google-apps.presentation" => "Google Slides",
        "application/vnd.google-apps.form" => "Google Form",
        "application/vnd.google-apps.drawing" => "Google Drawing",
        "application/vnd.google-apps.folder" => "Folder",
        "application/pdf" => "PDF",
        m if m.starts_with("image/") => "Image",
        m if m.starts_with("video/") => "Video",
        _ => "File",
    }
}

fn card_from_file(body: &Value, url: &str) -> Option<Unfurl> {
    let text = |pointer: &str| {
        body.pointer(pointer)
            .and_then(Value::as_str)
            .map(str::to_string)
    };
    let title = text("/name")?;
    let kind = kind_of(text("/mimeType").as_deref().unwrap_or_default());

    let mut fields = Vec::new();
    if let Some(owner) = text("/owners/0/displayName") {
        fields.push(UnfurlField {
            label: "Owner".into(),
            value: owner,
            avatar_url: text("/owners/0/photoLink").filter(|u| u.starts_with("https://")),
            ..UnfurlField::default()
        });
    }
    if let Some(modified) = text("/modifiedTime").and_then(|t| t.parse::<DateTime<Utc>>().ok()) {
        let by = text("/lastModifyingUser/displayName")
            .map(|name| format!(" by {name}"))
            .unwrap_or_default();
        fields.push(UnfurlField {
            label: "Last modified".into(),
            value: format!("{}{by}", modified.format("%b %-d, %Y")),
            ..UnfurlField::default()
        });
    }

    Some(Unfurl {
        url: url.to_string(),
        title,
        site_name: Some("Google Drive".into()),
        kind: Some("app".into()),
        provider: Some("google_drive".into()),
        subtitle: Some(kind.to_string()),
        fields,
        fetched_at: Some(Utc::now()),
        ..Unfurl::default()
    })
}

#[cfg(test)]
mod tests {
    use super::{card_from_file, match_file};
    use serde_json::json;

    const ID: &str = "1AbCdEfGhIjKlMnOpQrStUvWxYz_-012";

    #[test]
    fn matches_drive_and_docs_links() {
        for url in [
            format!("https://docs.google.com/document/d/{ID}/edit"),
            format!("https://docs.google.com/spreadsheets/d/{ID}/edit#gid=0"),
            format!("https://docs.google.com/presentation/d/{ID}"),
            format!("https://drive.google.com/file/d/{ID}/view?usp=sharing"),
            format!("https://drive.google.com/drive/folders/{ID}"),
            format!("https://drive.google.com/drive/u/0/folders/{ID}"),
            format!("https://drive.google.com/open?id={ID}"),
        ] {
            assert_eq!(match_file(&url).as_deref(), Some(ID), "{url}");
        }
    }

    #[test]
    fn leaves_everything_else_alone() {
        for url in [
            "https://docs.google.com/forms/d/e/1FAIpQLSf/viewform",
            "https://drive.google.com/drive/my-drive",
            "https://docs.google.com/document/d/short",
            "http://docs.google.com/document/d/1AbCdEfGhIjKlMnOpQ/edit",
            "https://evil.test/document/d/1AbCdEfGhIjKlMnOpQ/edit",
        ] {
            assert!(match_file(url).is_none(), "{url}");
        }
    }

    #[test]
    fn maps_a_doc_to_a_card() {
        let body = json!({
            "name": "Q4 plan",
            "mimeType": "application/vnd.google-apps.document",
            "owners": [{ "displayName": "Ana", "photoLink": "https://lh3.test/a" }],
            "modifiedTime": "2026-10-21T09:30:00.000Z",
            "lastModifyingUser": { "displayName": "Lazar" }
        });
        let card = card_from_file(&body, "u").expect("card");
        assert_eq!(card.title, "Q4 plan");
        assert_eq!(card.subtitle.as_deref(), Some("Google Doc"));
        let fields: Vec<(String, String)> = card
            .fields
            .iter()
            .map(|f| (f.label.clone(), f.value.clone()))
            .collect();
        assert_eq!(
            fields,
            vec![
                ("Owner".into(), "Ana".into()),
                ("Last modified".into(), "Oct 21, 2026 by Lazar".into())
            ]
        );
    }
}
