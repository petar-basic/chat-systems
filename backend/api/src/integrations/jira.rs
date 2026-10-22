use chrono::Utc;
use serde_json::Value;

use shared_common::errors::{AppError, AppResult};

use super::http::{self, TokenSet};
use super::models::AtlassianSite;
use crate::messaging::models::{Unfurl, UnfurlField};

pub struct Atlassian {
    pub auth_base: String,
    pub api_base: String,
}

#[derive(Debug, PartialEq)]
pub struct IssueRef {
    pub site_url: String,
    pub key: String,
}

fn provider_error(what: &str, e: impl std::fmt::Display) -> AppError {
    http::provider_error("Jira", what, e)
}

impl Atlassian {
    pub fn authorize_url(
        &self,
        client_id: &str,
        redirect_uri: &str,
        state: &str,
        challenge: &str,
        scopes: &[&str],
    ) -> String {
        let mut url = reqwest::Url::parse(&format!("{}/authorize", self.auth_base))
            .expect("the Atlassian auth base is a valid url");
        url.query_pairs_mut()
            .append_pair("audience", "api.atlassian.com")
            .append_pair("client_id", client_id)
            .append_pair("scope", &scopes.join(" "))
            .append_pair("redirect_uri", redirect_uri)
            .append_pair("state", state)
            .append_pair("response_type", "code")
            .append_pair("prompt", "consent")
            .append_pair("code_challenge", challenge)
            .append_pair("code_challenge_method", "S256");
        url.to_string()
    }

    async fn token(&self, body: Value) -> AppResult<TokenSet> {
        let response = http::client()?
            .post(format!("{}/oauth/token", self.auth_base))
            .json(&body)
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
        self.token(serde_json::json!({
            "grant_type": "authorization_code",
            "client_id": client_id,
            "client_secret": client_secret,
            "code": code,
            "redirect_uri": redirect_uri,
            "code_verifier": verifier,
        }))
        .await
    }

    pub async fn refresh(
        &self,
        client_id: &str,
        client_secret: &str,
        refresh_token: &str,
    ) -> AppResult<TokenSet> {
        self.token(serde_json::json!({
            "grant_type": "refresh_token",
            "client_id": client_id,
            "client_secret": client_secret,
            "refresh_token": refresh_token,
        }))
        .await
    }

    pub(super) async fn get(&self, path: &str, token: &str) -> AppResult<Option<Value>> {
        let response = http::client()?
            .get(format!("{}{path}", self.api_base))
            .bearer_auth(token)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await
            .map_err(|e| provider_error("request", e))?;
        if matches!(
            response.status(),
            reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::FORBIDDEN
        ) {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(provider_error("request", response.status()));
        }
        response
            .json::<Value>()
            .await
            .map(Some)
            .map_err(|e| provider_error("response", e))
    }

    pub async fn sites(&self, token: &str) -> AppResult<Vec<AtlassianSite>> {
        let sites = self
            .get("/oauth/token/accessible-resources", token)
            .await?
            .unwrap_or(Value::Null);
        Ok(sites
            .as_array()
            .map(|all| {
                all.iter()
                    .filter_map(|site| {
                        Some(AtlassianSite {
                            id: site.get("id")?.as_str()?.to_string(),
                            url: site.get("url")?.as_str()?.trim_end_matches('/').to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    pub async fn account_name(&self, token: &str) -> AppResult<String> {
        let me = self.get("/me", token).await?.unwrap_or(Value::Null);
        let name = me.get("name").and_then(Value::as_str);
        let email = me.get("email").and_then(Value::as_str);
        Ok(match (name, email) {
            (Some(name), Some(email)) => format!("{name} ({email})"),
            (Some(name), None) => name.to_string(),
            (None, Some(email)) => email.to_string(),
            (None, None) => "Atlassian account".to_string(),
        })
    }

    pub async fn issue_card(
        &self,
        token: &str,
        cloud_id: &str,
        issue: &IssueRef,
        url: &str,
    ) -> AppResult<Option<Unfurl>> {
        let path = format!(
            "/ex/jira/{cloud_id}/rest/api/3/issue/{}?fields=summary,status,priority,assignee,issuetype",
            issue.key
        );
        Ok(self
            .get(&path, token)
            .await?
            .and_then(|body| card_from_issue(&body, &issue.key, url)))
    }
}

pub fn match_issue(url: &str) -> Option<IssueRef> {
    let parsed = reqwest::Url::parse(url).ok()?;
    if parsed.scheme() != "https" {
        return None;
    }
    let host = parsed.host_str()?;
    if !host.ends_with(".atlassian.net") {
        return None;
    }
    let segments: Vec<&str> = parsed.path_segments()?.filter(|s| !s.is_empty()).collect();
    let key = match segments.as_slice() {
        ["browse", key] => key.to_string(),
        _ => parsed
            .query_pairs()
            .find(|(name, _)| name == "selectedIssue")
            .map(|(_, value)| value.into_owned())?,
    };
    is_issue_key(&key).then(|| IssueRef {
        site_url: format!("https://{host}"),
        key,
    })
}

fn is_issue_key(key: &str) -> bool {
    let Some((project, number)) = key.rsplit_once('-') else {
        return false;
    };
    project
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_uppercase())
        && project
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        && !number.is_empty()
        && number.chars().all(|c| c.is_ascii_digit())
}

fn card_from_issue(body: &Value, key: &str, url: &str) -> Option<Unfurl> {
    let fields = body.get("fields")?;
    let text = |pointer: &str| {
        fields
            .pointer(pointer)
            .and_then(Value::as_str)
            .map(str::to_string)
    };
    let title = text("/summary")?;
    let issue_type = text("/issuetype/name").unwrap_or_else(|| "Issue".into());

    let mut card_fields = Vec::new();
    if let Some(status) = text("/status/name") {
        let tone = match text("/status/statusCategory/key").as_deref() {
            Some("done") => "success",
            Some("indeterminate") => "info",
            _ => "neutral",
        };
        card_fields.push(UnfurlField {
            label: "Status".into(),
            value: status,
            tone: Some(tone.into()),
            avatar_url: None,
        });
    }
    if let Some(priority) = text("/priority/name") {
        card_fields.push(UnfurlField {
            label: "Priority".into(),
            value: priority,
            ..UnfurlField::default()
        });
    }
    card_fields.push(UnfurlField {
        label: "Assignee".into(),
        value: text("/assignee/displayName").unwrap_or_else(|| "Unassigned".into()),
        avatar_url: text("/assignee/avatarUrls/24x24").filter(|u| u.starts_with("https://")),
        ..UnfurlField::default()
    });

    Some(Unfurl {
        url: url.to_string(),
        title,
        site_name: Some("Jira".into()),
        kind: Some("app".into()),
        provider: Some("jira".into()),
        subtitle: Some(format!("{issue_type} {key}")),
        fields: card_fields,
        fetched_at: Some(Utc::now()),
        ..Unfurl::default()
    })
}

#[cfg(test)]
mod tests {
    use super::{card_from_issue, match_issue, IssueRef};

    #[test]
    fn matches_issue_links() {
        assert_eq!(
            match_issue("https://acme.atlassian.net/browse/PT-262"),
            Some(IssueRef {
                site_url: "https://acme.atlassian.net".into(),
                key: "PT-262".into()
            })
        );
        assert_eq!(
            match_issue(
                "https://acme.atlassian.net/jira/software/projects/PT/boards/1?selectedIssue=PT-9"
            )
            .map(|r| r.key),
            Some("PT-9".into())
        );
        assert!(match_issue("https://acme.atlassian.net/browse/not-a-key").is_none());
        assert!(match_issue("https://evil.example/browse/PT-1").is_none());
        assert!(match_issue("http://acme.atlassian.net/browse/PT-1").is_none());
    }

    #[test]
    fn maps_an_issue_to_a_card() {
        let body = serde_json::json!({
            "fields": {
                "summary": "Focus Axon planning on four project types",
                "issuetype": { "name": "Story" },
                "status": { "name": "In Progress", "statusCategory": { "key": "indeterminate" } },
                "priority": { "name": "Medium" },
                "assignee": { "displayName": "Lazar Stojanović", "avatarUrls": { "24x24": "https://avatar.test/l.png" } }
            }
        });
        let card = card_from_issue(&body, "PT-262", "https://acme.atlassian.net/browse/PT-262")
            .expect("card");
        assert_eq!(card.kind.as_deref(), Some("app"));
        assert_eq!(card.subtitle.as_deref(), Some("Story PT-262"));
        let summary: Vec<(String, String)> = card
            .fields
            .iter()
            .map(|f| (f.label.clone(), f.value.clone()))
            .collect();
        assert_eq!(
            summary,
            vec![
                ("Status".into(), "In Progress".into()),
                ("Priority".into(), "Medium".into()),
                ("Assignee".into(), "Lazar Stojanović".into())
            ]
        );
        assert_eq!(card.fields[0].tone.as_deref(), Some("info"));
    }

    #[test]
    fn an_unassigned_issue_says_so() {
        let body = serde_json::json!({ "fields": { "summary": "x", "assignee": null } });
        let card = card_from_issue(&body, "A-1", "u").expect("card");
        assert_eq!(
            card.fields.last().map(|f| f.value.as_str()),
            Some("Unassigned")
        );
    }
}
