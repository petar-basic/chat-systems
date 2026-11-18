pub mod models;
pub mod repo;
pub mod routes;

use std::collections::{BTreeMap, HashSet};

use serde_json::{json, Value};
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};
use shared_events::Event;

use crate::app_events::{self, repo::NewDelivery};
use crate::bots::models::{AppEvent, Scope};
use crate::integrations::models::Provider;
use crate::messaging::models::UnfurlTarget;
use crate::state::AppState;
use models::{Claim, SHARE_TTL_SECS};

const ATLASSIAN: &str = "atlassian.net";
const PROVIDER_HOSTS: [(Provider, &[&str]); 3] = [
    (Provider::Github, &["github.com", "www.github.com"]),
    (
        Provider::GoogleDrive,
        &["docs.google.com", "drive.google.com"],
    ),
    (Provider::Figma, &["figma.com", "www.figma.com"]),
];

pub fn normalize_domain(raw: &str) -> AppResult<String> {
    let domain = raw.trim().trim_end_matches('.').to_ascii_lowercase();
    let refuse = |why: &str| AppError::Validation(format!("domains: {raw:?} {why}"));
    if domain.len() > 253 {
        return Err(refuse("is too long"));
    }
    let labels: Vec<&str> = domain.split('.').collect();
    if labels.len() < 2 {
        return Err(refuse("needs a dot, as in tracker.example.com"));
    }
    let valid = labels.iter().all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    });
    if !valid {
        return Err(refuse(
            "is not a domain name: give the host alone, as in tracker.example.com",
        ));
    }
    if labels
        .last()
        .is_some_and(|tld| tld.chars().all(|c| c.is_ascii_digit()))
    {
        return Err(refuse("is an address, not a domain name"));
    }
    Ok(domain)
}

pub fn covers(domain: &str, host: &str) -> bool {
    host == domain || host.ends_with(&format!(".{domain}"))
}

pub fn host_of(url: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(url).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    Some(
        parsed
            .host_str()?
            .trim_end_matches('.')
            .to_ascii_lowercase(),
    )
}

pub fn providers_claiming(domain: &str) -> Vec<Provider> {
    let mut found = Vec::new();
    if covers(ATLASSIAN, domain) || covers(domain, ATLASSIAN) {
        found.extend([Provider::Jira, Provider::Confluence]);
    }
    for (provider, hosts) in PROVIDER_HOSTS {
        if hosts.iter().any(|host| covers(domain, host)) {
            found.push(provider);
        }
    }
    found
}

fn providers_of_host(host: &str) -> Vec<Provider> {
    let mut found = Vec::new();
    if host.ends_with(&format!(".{ATLASSIAN}")) {
        found.extend([Provider::Jira, Provider::Confluence]);
    }
    for (provider, hosts) in PROVIDER_HOSTS {
        if hosts.contains(&host) {
            found.push(provider);
        }
    }
    found
}

pub async fn providers_on(
    state: &AppState,
    workspace_id: Uuid,
    providers: &[Provider],
) -> AppResult<Vec<Provider>> {
    let mut on = Vec::new();
    for provider in providers {
        if state
            .integration_repo
            .app(workspace_id, provider.as_str())
            .await?
            .is_some_and(|app| app.enabled)
        {
            on.push(*provider);
        }
    }
    Ok(on)
}

fn listens(claim: &Claim) -> bool {
    claim.reachable
        && claim.is_member
        && claim
            .events
            .iter()
            .any(|e| e == AppEvent::LinkShared.as_str())
        && claim.scopes.iter().any(|s| s == Scope::LinksRead.as_str())
}

fn answered_urls(metadata: &Value) -> HashSet<String> {
    metadata
        .get("unfurls")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|unfurl| unfurl.get("app").is_some_and(|app| !app.is_null()))
        .filter_map(|unfurl| {
            unfurl
                .get("url")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect()
}

pub struct Routed {
    pub claimed: HashSet<String>,
    asked: BTreeMap<Uuid, Vec<(String, String)>>,
}

/// Finds the links an app claims, which the unfurl consumer leaves to the
/// app, and which of them to tell each claiming app about.
pub async fn route(state: &AppState, target: &UnfurlTarget, urls: &[String]) -> AppResult<Routed> {
    let mut routed = Routed {
        claimed: HashSet::new(),
        asked: BTreeMap::new(),
    };
    let claims = state
        .app_unfurl_repo
        .claims(target.workspace_id, target.channel_id)
        .await?;
    if claims.is_empty() {
        return Ok(routed);
    }
    let answered = answered_urls(&target.metadata);
    for url in urls {
        let Some(host) = host_of(url) else {
            continue;
        };
        let Some(claim) = claims
            .iter()
            .filter(|claim| covers(&claim.domain, &host))
            .max_by_key(|claim| claim.domain.len())
        else {
            continue;
        };
        if !providers_on(state, target.workspace_id, &providers_of_host(&host))
            .await?
            .is_empty()
        {
            continue;
        }
        routed.claimed.insert(url.clone());
        if !answered.contains(url) && listens(claim) {
            routed
                .asked
                .entry(claim.app_id)
                .or_default()
                .push((claim.domain.clone(), url.clone()));
        }
    }
    Ok(routed)
}

/// Sends `link_shared` to each app with links to answer for, and opens its
/// 30 minutes to answer.
pub async fn ask(
    state: &AppState,
    source: &Event,
    message_id: Uuid,
    target: &UnfurlTarget,
    routed: Routed,
) -> AppResult<()> {
    for (app_id, links) in routed.asked {
        let urls: Vec<String> = links.iter().map(|(_, url)| url.clone()).collect();
        state
            .app_unfurl_repo
            .share(app_id, message_id, &urls, SHARE_TTL_SECS)
            .await?;
        let body = json!({
            "channel_id": target.channel_id,
            "message_id": message_id,
            "user_id": target.poster_id,
            "links": links
                .iter()
                .map(|(domain, url)| json!({ "domain": domain, "url": url }))
                .collect::<Vec<_>>(),
        });
        let envelope =
            app_events::envelope(source, target.workspace_id, AppEvent::LinkShared, &body);
        state
            .app_event_repo
            .enqueue(NewDelivery {
                app_id,
                event_id: app_events::event_id(source.id, AppEvent::LinkShared),
                channel_id: target.channel_id,
                kind: app_events::KIND_EVENT,
                body: &envelope,
                ttl_secs: SHARE_TTL_SECS,
            })
            .await?;
    }
    Ok(())
}
