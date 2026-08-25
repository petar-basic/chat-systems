use std::time::Duration;

use async_trait::async_trait;

/// Fetching an attachment is the one part of an import that leaves the machine,
/// so it is behind a trait: the tests exercise the import without a network, and
/// an operator running without a Slack token gets a clear reason per file
/// instead of a stack trace.
#[async_trait]
pub trait FileFetcher: Send + Sync {
    async fn fetch(&self, url: &str) -> Result<Vec<u8>, String>;
}

pub struct HttpFileFetcher {
    client: reqwest::Client,
    token: Option<String>,
}

const FETCH_TIMEOUT: Duration = Duration::from_secs(60);
/// Slack's own limit is 1 GB; this is about what a chat attachment ever is, and
/// an import should not be the thing that fills the disk.
const MAX_FILE_BYTES: usize = 100 * 1024 * 1024;

impl HttpFileFetcher {
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
impl FileFetcher for HttpFileFetcher {
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
}

/// Used when the operator asked not to fetch files, and by the tests.
pub struct SkipFiles;

#[async_trait]
impl FileFetcher for SkipFiles {
    async fn fetch(&self, _url: &str) -> Result<Vec<u8>, String> {
        Err("files were not fetched for this run".into())
    }
}
