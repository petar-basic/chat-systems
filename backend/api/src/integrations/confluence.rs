use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use shared_common::errors::{AppError, AppResult};

use super::http;
use super::jira::Atlassian;
use crate::messaging::models::{Unfurl, UnfurlField};

pub const SCOPES: [&str; 5] = [
    "read:page:confluence",
    "read:space:confluence",
    "read:user:confluence",
    "read:me",
    "offline_access",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentKind {
    Page,
    BlogPost,
}

impl ContentKind {
    fn collection(self) -> &'static str {
        match self {
            ContentKind::Page => "pages",
            ContentKind::BlogPost => "blogposts",
        }
    }

    fn label(self) -> &'static str {
        match self {
            ContentKind::Page => "Page",
            ContentKind::BlogPost => "Blog post",
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct ContentRef {
    pub site_url: String,
    pub kind: ContentKind,
    pub id: String,
}

fn provider_error(what: &str, e: impl std::fmt::Display) -> AppError {
    http::provider_error("Confluence", what, e)
}

impl Atlassian {
    async fn post(&self, path: &str, token: &str, body: &Value) -> AppResult<Option<Value>> {
        let response = http::client()?
            .post(format!("{}{path}", self.api_base))
            .bearer_auth(token)
            .header(reqwest::header::ACCEPT, "application/json")
            .json(body)
            .send()
            .await
            .map_err(|e| provider_error("request", e))?;
        if !response.status().is_success() {
            return Ok(None);
        }
        response
            .json::<Value>()
            .await
            .map(Some)
            .map_err(|e| provider_error("response", e))
    }

    pub async fn content_card(
        &self,
        token: &str,
        cloud_id: &str,
        content: &ContentRef,
        url: &str,
    ) -> AppResult<Option<Unfurl>> {
        let base = format!("/ex/confluence/{cloud_id}/wiki/api/v2");
        let Some(body) = self
            .get(
                &format!("{base}/{}/{}", content.kind.collection(), content.id),
                token,
            )
            .await?
        else {
            return Ok(None);
        };

        let text = |pointer: &str| {
            body.pointer(pointer)
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
        };
        let owner = text("/ownerId").or_else(|| text("/authorId"));
        let editor = text("/version/authorId");
        let accounts: Vec<&str> = owner.iter().chain(editor.iter()).copied().collect();

        let space = async {
            match text("/spaceId") {
                Some(id) => self.get(&format!("{base}/spaces/{id}"), token).await,
                None => Ok(None),
            }
        };
        let users = async {
            if accounts.is_empty() {
                return Ok(None);
            }
            self.post(
                &format!("{base}/users-bulk"),
                token,
                &json!({ "accountIds": accounts }),
            )
            .await
        };
        let (space, users) = tokio::join!(space, users);
        let space_name = space
            .ok()
            .flatten()
            .and_then(|s| s.get("name").and_then(Value::as_str).map(str::to_string));
        let names = users
            .ok()
            .flatten()
            .map(|u| display_names(&u))
            .unwrap_or_default();

        Ok(card_from_content(
            content.kind,
            &body,
            space_name.as_deref(),
            &names,
            url,
        ))
    }
}

pub fn match_content(url: &str) -> Option<ContentRef> {
    let parsed = reqwest::Url::parse(url).ok()?;
    if parsed.scheme() != "https" {
        return None;
    }
    let host = parsed.host_str()?;
    if !host.ends_with(".atlassian.net") {
        return None;
    }
    let segments: Vec<&str> = parsed.path_segments()?.filter(|s| !s.is_empty()).collect();
    let (kind, id) = match segments.as_slice() {
        ["wiki", "spaces", _, "pages", "edit-v2", id, ..] => (ContentKind::Page, id.to_string()),
        ["wiki", "spaces", _, "pages", id, ..] => (ContentKind::Page, id.to_string()),
        ["wiki", "spaces", _, "blog", "edit-v2", id, ..] => (ContentKind::BlogPost, id.to_string()),
        ["wiki", "spaces", _, "blog", _, _, _, id, ..] => (ContentKind::BlogPost, id.to_string()),
        ["wiki", "pages", "viewpage.action"] => (
            ContentKind::Page,
            parsed
                .query_pairs()
                .find(|(k, _)| k == "pageId")
                .map(|(_, v)| v.into_owned())?,
        ),
        _ => return None,
    };
    ((1..=20).contains(&id.len()) && id.chars().all(|c| c.is_ascii_digit())).then(|| ContentRef {
        site_url: format!("https://{host}"),
        kind,
        id,
    })
}

fn display_names(body: &Value) -> HashMap<String, String> {
    body.get("results")
        .and_then(Value::as_array)
        .map(|users| {
            users
                .iter()
                .filter_map(|user| {
                    Some((
                        user.get("accountId")?.as_str()?.to_string(),
                        user.get("displayName")
                            .or_else(|| user.get("publicName"))?
                            .as_str()?
                            .to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn card_from_content(
    kind: ContentKind,
    body: &Value,
    space: Option<&str>,
    names: &HashMap<String, String>,
    url: &str,
) -> Option<Unfurl> {
    let text = |pointer: &str| {
        body.pointer(pointer)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
    };
    let name_of = |pointer: &str| text(pointer).and_then(|id| names.get(id)).cloned();
    let title = text("/title")?.to_string();

    let mut fields = Vec::new();
    if let Some(owner) = name_of("/ownerId").or_else(|| name_of("/authorId")) {
        fields.push(UnfurlField {
            label: "Owner".into(),
            value: owner,
            ..UnfurlField::default()
        });
    }
    if let Some(updated) = text("/version/createdAt").and_then(|t| t.parse::<DateTime<Utc>>().ok())
    {
        let day = updated.format("%b %-d, %Y");
        fields.push(UnfurlField {
            label: "Last updated".into(),
            value: match name_of("/version/authorId") {
                Some(name) => format!("{day} by {name}"),
                None => day.to_string(),
            },
            ..UnfurlField::default()
        });
    }

    Some(Unfurl {
        url: url.to_string(),
        title,
        site_name: Some("Confluence".into()),
        kind: Some("app".into()),
        provider: Some("confluence".into()),
        subtitle: Some(match space {
            Some(space) => format!("{} in {space}", kind.label()),
            None => kind.label().to_string(),
        }),
        fields,
        fetched_at: Some(Utc::now()),
        ..Unfurl::default()
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::json;

    use super::{card_from_content, display_names, match_content, ContentKind, ContentRef};

    fn page(id: &str) -> Option<ContentRef> {
        Some(ContentRef {
            site_url: "https://acme.atlassian.net".into(),
            kind: ContentKind::Page,
            id: id.into(),
        })
    }

    #[test]
    fn matches_pages_and_blog_posts() {
        for url in [
            "https://acme.atlassian.net/wiki/spaces/ENG/pages/12345/Onboarding+guide",
            "https://acme.atlassian.net/wiki/spaces/ENG/pages/12345",
            "https://acme.atlassian.net/wiki/spaces/ENG/pages/edit-v2/12345",
            "https://acme.atlassian.net/wiki/pages/viewpage.action?pageId=12345",
        ] {
            assert_eq!(match_content(url), page("12345"), "{url}");
        }
        let blog = match_content(
            "https://acme.atlassian.net/wiki/spaces/ENG/blog/2026/10/21/999/Release+notes",
        )
        .expect("blog");
        assert_eq!(
            (blog.kind, blog.id.as_str()),
            (ContentKind::BlogPost, "999")
        );
    }

    #[test]
    fn leaves_everything_else_alone() {
        for url in [
            "http://acme.atlassian.net/wiki/spaces/ENG/pages/12345",
            "https://acme.atlassian.net.evil.test/wiki/spaces/ENG/pages/12345",
            "https://acme.atlassian.net/wiki/spaces/ENG/overview",
            "https://acme.atlassian.net/wiki/spaces/ENG/pages/not-a-number",
            "https://acme.atlassian.net/wiki/x/AbCd",
            "https://acme.atlassian.net/browse/ENG-1",
        ] {
            assert!(match_content(url).is_none(), "{url}");
        }
    }

    #[test]
    fn maps_a_page_to_a_card() {
        let body = json!({
            "title": "Onboarding guide",
            "spaceId": "777",
            "ownerId": "acc-ana",
            "authorId": "acc-old",
            "version": { "createdAt": "2026-10-21T09:30:00.000Z", "authorId": "acc-lazar" }
        });
        let names = display_names(&json!({ "results": [
            { "accountId": "acc-ana", "displayName": "Ana" },
            { "accountId": "acc-lazar", "displayName": "Lazar" }
        ]}));
        let card = card_from_content(ContentKind::Page, &body, Some("Engineering"), &names, "u")
            .expect("card");
        assert_eq!(card.title, "Onboarding guide");
        assert_eq!(card.subtitle.as_deref(), Some("Page in Engineering"));
        let fields: Vec<(String, String)> = card
            .fields
            .iter()
            .map(|f| (f.label.clone(), f.value.clone()))
            .collect();
        assert_eq!(
            fields,
            vec![
                ("Owner".into(), "Ana".into()),
                ("Last updated".into(), "Oct 21, 2026 by Lazar".into())
            ]
        );
    }

    #[test]
    fn a_post_without_names_or_space_still_gets_a_card() {
        let body = json!({ "title": "Release notes", "authorId": "acc-ana" });
        let card = card_from_content(ContentKind::BlogPost, &body, None, &HashMap::new(), "u")
            .expect("card");
        assert_eq!(card.subtitle.as_deref(), Some("Blog post"));
        assert!(card.fields.is_empty());
        assert!(
            card_from_content(ContentKind::Page, &json!({}), None, &HashMap::new(), "u").is_none()
        );
    }
}
