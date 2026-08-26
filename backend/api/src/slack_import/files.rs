use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;

/// Reaching Slack is the one part of an import that leaves the machine, so it is
/// behind a trait: the tests exercise the import without a network, and an
/// operator running without a token gets a clear reason per item instead of a
/// stack trace.
#[async_trait]
pub trait SlackClient: Send + Sync {
    async fn fetch(&self, url: &str) -> Result<Vec<u8>, String>;

    /// Custom emoji are not in the export. `emoji.list` is where they live, and
    /// it needs a token with `emoji:read`; the map is name → url, or
    /// `alias:other` where one name points at another.
    async fn custom_emoji(&self) -> Result<HashMap<String, String>, String>;
}

pub struct HttpSlackClient {
    client: reqwest::Client,
    token: Option<String>,
}

const FETCH_TIMEOUT: Duration = Duration::from_secs(60);
/// Slack's own limit is 1 GB; this is about what a chat attachment ever is, and
/// an import should not be the thing that fills the disk.
const MAX_FILE_BYTES: usize = 100 * 1024 * 1024;

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
    async fn fetch(&self, url: &str) -> Result<Vec<u8>, String> {
        let mut request = self.client.get(url);
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }

        let response = request.send().await.map_err(|e| e.to_string())?;
        if !response.status().is_success() {
            // A private file without a token comes back as Slack's HTML sign-in
            // page with a 200 or a 403; either way there is nothing to store.
            return Err(format!("{} from Slack", response.status()));
        }

        let body = response.bytes().await.map_err(|e| e.to_string())?;
        if body.len() > MAX_FILE_BYTES {
            return Err(format!("{} bytes is over the import limit", body.len()));
        }
        Ok(body.to_vec())
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
    async fn fetch(&self, _url: &str) -> Result<Vec<u8>, String> {
        Err("files were not fetched for this run".into())
    }

    async fn custom_emoji(&self) -> Result<HashMap<String, String>, String> {
        Err("custom emoji were not fetched for this run".into())
    }
}
