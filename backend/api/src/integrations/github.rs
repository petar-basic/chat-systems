use chrono::Utc;
use serde_json::Value;

use shared_common::errors::{AppError, AppResult};

use super::http::{self, TokenSet};
use crate::messaging::models::{Unfurl, UnfurlField};

const RESERVED_OWNERS: &[&str] = &[
    "about",
    "account",
    "apps",
    "codespaces",
    "collections",
    "customer-stories",
    "dashboard",
    "enterprise",
    "explore",
    "features",
    "issues",
    "login",
    "marketplace",
    "new",
    "notifications",
    "orgs",
    "organizations",
    "pricing",
    "pulls",
    "readme",
    "search",
    "security",
    "settings",
    "site",
    "sponsors",
    "team",
    "topics",
    "trending",
    "users",
];

pub struct GitHub {
    pub web_base: String,
    pub api_base: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    Issue {
        owner: String,
        repo: String,
        number: u64,
    },
    PullRequest {
        owner: String,
        repo: String,
        number: u64,
    },
    Commit {
        owner: String,
        repo: String,
        sha: String,
    },
    Repository {
        owner: String,
        repo: String,
    },
}

pub enum Fetched {
    Card(Box<Unfurl>),
    NotVisible,
}

fn provider_error(what: &str, e: impl std::fmt::Display) -> AppError {
    http::provider_error("GitHub", what, e)
}

impl GitHub {
    pub fn authorize_url(
        &self,
        client_id: &str,
        redirect_uri: &str,
        state: &str,
        challenge: &str,
    ) -> String {
        let mut url = reqwest::Url::parse(&format!("{}/login/oauth/authorize", self.web_base))
            .expect("the GitHub web base is a valid url");
        url.query_pairs_mut()
            .append_pair("client_id", client_id)
            .append_pair("redirect_uri", redirect_uri)
            .append_pair("state", state)
            .append_pair("code_challenge", challenge)
            .append_pair("code_challenge_method", "S256");
        url.to_string()
    }

    async fn token(&self, body: Value) -> AppResult<TokenSet> {
        let response = http::client()?
            .post(format!("{}/login/oauth/access_token", self.web_base))
            .header(reqwest::header::ACCEPT, "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| provider_error("token request", e))?;
        if !response.status().is_success() {
            return Err(provider_error("token request", response.status()));
        }
        let body: Value = response
            .json()
            .await
            .map_err(|e| provider_error("token response", e))?;
        if let Some(error) = body.get("error").and_then(Value::as_str) {
            return Err(provider_error("token request", error));
        }
        serde_json::from_value(body).map_err(|e| provider_error("token response", e))
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

    async fn get(&self, path: &str, token: Option<&str>) -> AppResult<Option<Value>> {
        let mut request = http::client()?
            .get(format!("{}{path}", self.api_base))
            .header(reqwest::header::ACCEPT, "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28");
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        let response = request
            .send()
            .await
            .map_err(|e| provider_error("request", e))?;
        let rate_limited = response
            .headers()
            .get("x-ratelimit-remaining")
            .and_then(|v| v.to_str().ok())
            == Some("0");
        match response.status() {
            status if status.is_success() => response
                .json::<Value>()
                .await
                .map(Some)
                .map_err(|e| provider_error("response", e)),
            reqwest::StatusCode::FORBIDDEN | reqwest::StatusCode::TOO_MANY_REQUESTS
                if rate_limited =>
            {
                Err(provider_error("request", "rate limited"))
            }
            reqwest::StatusCode::NOT_FOUND
            | reqwest::StatusCode::FORBIDDEN
            | reqwest::StatusCode::UNAUTHORIZED => Ok(None),
            status => Err(provider_error("request", status)),
        }
    }

    pub async fn account_name(&self, token: &str) -> AppResult<String> {
        let me = self.get("/user", Some(token)).await?.unwrap_or(Value::Null);
        let login = me.get("login").and_then(Value::as_str);
        let name = me.get("name").and_then(Value::as_str);
        Ok(match (name, login) {
            (Some(name), Some(login)) if name != login => format!("{name} (@{login})"),
            (_, Some(login)) => format!("@{login}"),
            (Some(name), None) => name.to_string(),
            (None, None) => "GitHub account".to_string(),
        })
    }

    pub async fn card(
        &self,
        target: &Target,
        url: &str,
        token: Option<&str>,
    ) -> AppResult<Fetched> {
        let fetched = match target {
            Target::Issue {
                owner,
                repo,
                number,
            } => match self.issue(owner, repo, *number, token).await? {
                Some(issue) if issue.get("pull_request").is_some() => self
                    .pull_request(owner, repo, *number, token)
                    .await?
                    .map(|pull| {
                        (
                            Target::PullRequest {
                                owner: owner.clone(),
                                repo: repo.clone(),
                                number: *number,
                            },
                            pull,
                        )
                    }),
                Some(issue) => Some((target.clone(), issue)),
                None => None,
            },
            Target::PullRequest {
                owner,
                repo,
                number,
            } => match self.pull_request(owner, repo, *number, token).await? {
                Some(pull) => Some((target.clone(), pull)),
                None => self.issue(owner, repo, *number, token).await?.map(|issue| {
                    (
                        Target::Issue {
                            owner: owner.clone(),
                            repo: repo.clone(),
                            number: *number,
                        },
                        issue,
                    )
                }),
            },
            Target::Commit { owner, repo, sha } => self
                .get(&format!("/repos/{owner}/{repo}/commits/{sha}"), token)
                .await?
                .map(|body| (target.clone(), body)),
            Target::Repository { owner, repo } => self
                .get(&format!("/repos/{owner}/{repo}"), token)
                .await?
                .map(|body| (target.clone(), body)),
        };
        Ok(
            match fetched.and_then(|(target, body)| card_from(&target, &body, url)) {
                Some(card) => Fetched::Card(Box::new(card)),
                None => Fetched::NotVisible,
            },
        )
    }

    async fn issue(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        token: Option<&str>,
    ) -> AppResult<Option<Value>> {
        self.get(&format!("/repos/{owner}/{repo}/issues/{number}"), token)
            .await
    }

    async fn pull_request(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        token: Option<&str>,
    ) -> AppResult<Option<Value>> {
        self.get(&format!("/repos/{owner}/{repo}/pulls/{number}"), token)
            .await
    }
}

pub fn match_url(url: &str) -> Option<Target> {
    let parsed = reqwest::Url::parse(url).ok()?;
    if parsed.scheme() != "https" || !matches!(parsed.host_str()?, "github.com" | "www.github.com")
    {
        return None;
    }
    let segments: Vec<&str> = parsed.path_segments()?.filter(|s| !s.is_empty()).collect();
    let (owner, repo) = (*segments.first()?, *segments.get(1)?);
    if RESERVED_OWNERS.contains(&owner.to_ascii_lowercase().as_str())
        || !is_name(owner)
        || !is_name(repo)
    {
        return None;
    }
    let (owner, repo) = (owner.to_string(), repo.trim_end_matches(".git").to_string());
    match segments.get(2..) {
        Some([]) => Some(Target::Repository { owner, repo }),
        Some(["issues", number, ..]) => number.parse().ok().map(|number| Target::Issue {
            owner,
            repo,
            number,
        }),
        Some(["pull", number, ..]) => number.parse().ok().map(|number| Target::PullRequest {
            owner,
            repo,
            number,
        }),
        Some(["commit", sha, ..])
            if sha.len() >= 7 && sha.len() <= 40 && sha.chars().all(|c| c.is_ascii_hexdigit()) =>
        {
            Some(Target::Commit {
                owner,
                repo,
                sha: sha.to_string(),
            })
        }
        _ => None,
    }
}

fn is_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

fn text(body: &Value, pointer: &str) -> Option<String> {
    body.pointer(pointer)
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn person(body: &Value, pointer: &str, label: &str) -> Option<UnfurlField> {
    Some(UnfurlField {
        label: label.into(),
        value: text(body, &format!("{pointer}/login"))?,
        avatar_url: text(body, &format!("{pointer}/avatar_url"))
            .filter(|u| u.starts_with("https://")),
        ..UnfurlField::default()
    })
}

fn state(value: &str, tone: &str) -> UnfurlField {
    UnfurlField {
        label: "State".into(),
        value: value.into(),
        tone: Some(tone.into()),
        avatar_url: None,
    }
}

fn plain(label: &str, value: String) -> UnfurlField {
    UnfurlField {
        label: label.into(),
        value,
        ..UnfurlField::default()
    }
}

fn labels(body: &Value) -> Option<UnfurlField> {
    let names: Vec<&str> = body
        .get("labels")?
        .as_array()?
        .iter()
        .filter_map(|l| l.get("name").and_then(Value::as_str))
        .collect();
    (!names.is_empty()).then(|| plain("Labels", names.join(", ")))
}

fn changes(body: &Value, additions: &str, deletions: &str) -> Option<UnfurlField> {
    let add = body.pointer(additions)?.as_u64()?;
    let del = body.pointer(deletions)?.as_u64()?;
    let files = body.pointer("/changed_files").and_then(Value::as_u64);
    Some(plain(
        "Changes",
        match files {
            Some(files) => format!("+{add} −{del} in {files} files"),
            None => format!("+{add} −{del}"),
        },
    ))
}

fn card_from(target: &Target, body: &Value, url: &str) -> Option<Unfurl> {
    let (title, subtitle, description, fields) =
        match target {
            Target::Issue {
                owner,
                repo,
                number,
            } => {
                let closed = text(body, "/state").as_deref() == Some("closed");
                let mut fields = vec![if closed {
                    state("Closed", "neutral")
                } else {
                    state("Open", "success")
                }];
                fields.extend(person(body, "/user", "Author"));
                fields.extend(labels(body));
                (
                    text(body, "/title")?,
                    format!("Issue #{number} in {owner}/{repo}"),
                    None,
                    fields,
                )
            }
            Target::PullRequest {
                owner,
                repo,
                number,
            } => {
                let merged = body.get("merged").and_then(Value::as_bool).unwrap_or(false)
                    || body.get("merged_at").is_some_and(|v| !v.is_null());
                let draft = body.get("draft").and_then(Value::as_bool).unwrap_or(false);
                let closed = text(body, "/state").as_deref() == Some("closed");
                let status = match (merged, closed, draft) {
                    (true, _, _) => state("Merged", "info"),
                    (false, true, _) => state("Closed", "neutral"),
                    (false, false, true) => state("Draft", "neutral"),
                    (false, false, false) => state("Open", "success"),
                };
                let mut fields = vec![status];
                fields.extend(person(body, "/user", "Author"));
                fields.extend(changes(body, "/additions", "/deletions"));
                fields.extend(labels(body));
                (
                    text(body, "/title")?,
                    format!("Pull request #{number} in {owner}/{repo}"),
                    None,
                    fields,
                )
            }
            Target::Commit { owner, repo, sha } => {
                let message = text(body, "/commit/message")?;
                let mut fields = Vec::new();
                fields.extend(person(body, "/author", "Author").or_else(|| {
                    text(body, "/commit/author/name").map(|name| plain("Author", name))
                }));
                fields.extend(changes(body, "/stats/additions", "/stats/deletions"));
                (
                    message.lines().next().unwrap_or_default().to_string(),
                    format!("Commit {} in {owner}/{repo}", &sha[..sha.len().min(7)]),
                    None,
                    fields,
                )
            }
            Target::Repository { .. } => {
                let mut fields = Vec::new();
                fields.extend(text(body, "/language").map(|language| plain("Language", language)));
                fields.extend(
                    body.get("stargazers_count")
                        .and_then(Value::as_u64)
                        .map(|stars| plain("Stars", stars.to_string())),
                );
                (
                    text(body, "/full_name")?,
                    if body.get("private").and_then(Value::as_bool) == Some(true) {
                        "Private repository".to_string()
                    } else {
                        "Repository".to_string()
                    },
                    text(body, "/description"),
                    fields,
                )
            }
        };
    Some(Unfurl {
        url: url.to_string(),
        title,
        description,
        site_name: Some("GitHub".into()),
        kind: Some("app".into()),
        provider: Some("github".into()),
        subtitle: Some(subtitle),
        fields,
        fetched_at: Some(Utc::now()),
        ..Unfurl::default()
    })
}

#[cfg(test)]
mod tests {
    use super::{card_from, match_url, Target};
    use serde_json::json;

    fn t(owner: &str, repo: &str) -> (String, String) {
        (owner.into(), repo.into())
    }

    #[test]
    fn matches_the_objects_it_can_card() {
        let (owner, repo) = t("acme", "rocket");
        assert_eq!(
            match_url("https://github.com/acme/rocket/pull/42/files"),
            Some(Target::PullRequest {
                owner: owner.clone(),
                repo: repo.clone(),
                number: 42
            })
        );
        assert_eq!(
            match_url("https://github.com/acme/rocket/issues/7"),
            Some(Target::Issue {
                owner: owner.clone(),
                repo: repo.clone(),
                number: 7
            })
        );
        assert_eq!(
            match_url("https://github.com/acme/rocket/commit/abcdef1234"),
            Some(Target::Commit {
                owner: owner.clone(),
                repo: repo.clone(),
                sha: "abcdef1234".into()
            })
        );
        assert_eq!(
            match_url("https://github.com/acme/rocket"),
            Some(Target::Repository { owner, repo })
        );
    }

    #[test]
    fn leaves_everything_else_alone() {
        for url in [
            "https://github.com/settings/profile",
            "https://github.com/orgs/acme/people",
            "https://github.com/acme",
            "https://github.com/acme/rocket/tree/main/src",
            "https://github.com/acme/rocket/commit/xyz",
            "http://github.com/acme/rocket/pull/1",
            "https://gitlab.com/acme/rocket/pull/1",
        ] {
            assert!(match_url(url).is_none(), "{url}");
        }
    }

    #[test]
    fn a_merged_pull_request_says_merged_with_its_size() {
        let body = json!({
            "title": "Ship it", "state": "closed", "merged": true, "draft": false,
            "user": { "login": "ana", "avatar_url": "https://avatars.test/a" },
            "additions": 120, "deletions": 8, "changed_files": 5,
            "labels": [{ "name": "backend" }]
        });
        let target = Target::PullRequest {
            owner: "acme".into(),
            repo: "rocket".into(),
            number: 42,
        };
        let card = card_from(&target, &body, "u").expect("card");
        let fields: Vec<(String, String)> = card
            .fields
            .iter()
            .map(|f| (f.label.clone(), f.value.clone()))
            .collect();
        assert_eq!(
            card.subtitle.as_deref(),
            Some("Pull request #42 in acme/rocket")
        );
        assert_eq!(
            fields,
            vec![
                ("State".into(), "Merged".into()),
                ("Author".into(), "ana".into()),
                ("Changes".into(), "+120 −8 in 5 files".into()),
                ("Labels".into(), "backend".into()),
            ]
        );
    }

    #[test]
    fn a_commit_card_uses_the_first_line_of_its_message() {
        let body = json!({
            "commit": { "message": "Fix the thing\n\nLong explanation", "author": { "name": "Ana" } },
            "author": null,
            "stats": { "additions": 3, "deletions": 1 }
        });
        let target = Target::Commit {
            owner: "acme".into(),
            repo: "rocket".into(),
            sha: "abcdef1234".into(),
        };
        let card = card_from(&target, &body, "u").expect("card");
        assert_eq!(card.title, "Fix the thing");
        assert_eq!(
            card.subtitle.as_deref(),
            Some("Commit abcdef1 in acme/rocket")
        );
        assert_eq!(card.fields[0].value, "Ana");
    }
}
