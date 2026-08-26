use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;

use crate::hooks::ssrf;

/// Reaching Slack is the one part of an import that leaves the machine, so it is
/// behind a trait: the tests exercise the import without a network, and an
/// operator running without a token gets a clear reason per item instead of a
/// stack trace.
#[async_trait]
pub trait SlackClient: Send + Sync {
    /// `max_bytes` is enforced while reading, not after: an import should not be
    /// able to be handed a four-gigabyte answer and find out afterwards.
    async fn fetch(&self, url: &str, max_bytes: usize) -> Result<Vec<u8>, String>;

    /// Custom emoji are not in the export. `emoji.list` is where they live, and
    /// it needs a token with `emoji:read`; the map is name → url, or
    /// `alias:other` where one name points at another.
    async fn custom_emoji(&self) -> Result<HashMap<String, String>, String>;
}

pub struct HttpSlackClient {
    client: reqwest::Client,
    token: Option<String>,
}

/// The token is Slack's, and it goes to Slack. An export is a file that arrives
/// from outside — from Slack, from a consultant, from somebody's laptop — and a
/// single doctored `url_private` would otherwise hand a live token with
/// `files:read` to whoever wrote it.
fn is_slack_host(url: &reqwest::Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.to_lowercase();
    host == "slack.com" || host.ends_with(".slack.com") || host.ends_with(".slack-edge.com")
}

const FETCH_TIMEOUT: Duration = Duration::from_secs(60);

impl HttpSlackClient {
    pub fn new(token: Option<String>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(FETCH_TIMEOUT)
                .build()
                .unwrap_or_default(),
            token,
        }
    }
}

#[async_trait]
impl SlackClient for HttpSlackClient {
    async fn fetch(&self, url: &str, max_bytes: usize) -> Result<Vec<u8>, String> {
        // The same guard the webhook path uses: an export names the URLs, and
        // without this an import is a request-forgery primitive against
        // everything the server can reach, cloud metadata first among them.
        let url = ssrf::validate_outbound_url(url)
            .await
            .map_err(|e| e.to_string())?;

        let mut request = self.client.get(url.clone());
        match (&self.token, is_slack_host(&url)) {
            (Some(token), true) => request = request.bearer_auth(token),
            (Some(_), false) => {
                return Err(format!(
                    "{} is not a Slack host, so the token was not sent",
                    url.host_str().unwrap_or("that host")
                ))
            }
            (None, _) => {}
        }

        let mut response = request.send().await.map_err(|e| e.to_string())?;
        if !response.status().is_success() {
            // A private file without a token comes back as Slack's HTML sign-in
            // page with a 200 or a 403; either way there is nothing to store.
            return Err(format!("{} from Slack", response.status()));
        }
        if let Some(length) = response.content_length() {
            if length > max_bytes as u64 {
                return Err(format!("{length} bytes is over the limit for this import"));
            }
        }

        // Streamed with the cap applied as it arrives: `content-length` is the
        // sender's claim, not a promise.
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
            if body.len() + chunk.len() > max_bytes {
                return Err(format!("more than {max_bytes} bytes, so it was not stored"));
            }
            body.extend_from_slice(&chunk);
        }
        Ok(body)
    }

    async fn custom_emoji(&self) -> Result<HashMap<String, String>, String> {
        let Some(token) = &self.token else {
            return Err("no Slack token, so custom emoji cannot be read".into());
        };

        let response = self
            .client
            .get("https://slack.com/api/emoji.list")
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let body: EmojiList = response.json().await.map_err(|e| e.to_string())?;
        if !body.ok {
            // Slack answers 200 with `ok: false` and a reason, which is the only
            // place the real problem — a missing scope, usually — is written.
            return Err(body.error.unwrap_or_else(|| "emoji.list refused".into()));
        }
        Ok(body.emoji)
    }
}

#[derive(serde::Deserialize)]
struct EmojiList {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    emoji: HashMap<String, String>,
}

/// Used when the operator asked not to reach Slack, and by the tests.
pub struct OfflineSlack;

#[async_trait]
impl SlackClient for OfflineSlack {
    async fn fetch(&self, _url: &str, _max_bytes: usize) -> Result<Vec<u8>, String> {
        Err("files were not fetched for this run".into())
    }

    async fn custom_emoji(&self) -> Result<HashMap<String, String>, String> {
        Err("custom emoji were not fetched for this run".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(raw: &str) -> reqwest::Url {
        reqwest::Url::parse(raw).expect("a url")
    }

    #[test]
    fn the_token_goes_to_slack_and_nowhere_else() {
        assert!(is_slack_host(&url("https://files.slack.com/a.png")));
        assert!(is_slack_host(&url(
            "https://emoji.slack-edge.com/T1/shipit/1.png"
        )));
        assert!(is_slack_host(&url("https://slack.com/api/emoji.list")));

        assert!(!is_slack_host(&url("https://attacker.test/a.png")));
        assert!(
            !is_slack_host(&url("https://slack.com.attacker.test/a.png")),
            "a suffix that merely contains the name is not the name"
        );
        assert!(
            !is_slack_host(&url("https://notslack.com/a.png")),
            "and neither is one that ends with it without the dot"
        );
    }
}
