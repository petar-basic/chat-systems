use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use url::{Host, Url};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Server {
    pub url: String,
    pub name: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hidden: Vec<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Saved {
    servers: Vec<Server>,
    last: Option<String>,
}

pub struct Store {
    path: PathBuf,
    saved: Saved,
}

impl Store {
    pub fn load(dir: &Path) -> Self {
        let path = dir.join("servers.json");
        let saved = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self { path, saved }
    }

    pub fn servers(&self) -> &[Server] {
        &self.saved.servers
    }

    pub fn find(&self, url: &str) -> Option<&Server> {
        self.saved.servers.iter().find(|s| s.url == url)
    }

    pub fn last(&self) -> Option<&Server> {
        self.saved
            .last
            .as_deref()
            .and_then(|url| self.find(url))
            .or_else(|| self.saved.servers.first())
    }

    pub fn upsert(&mut self, server: Server) -> Result<(), String> {
        match self.saved.servers.iter_mut().find(|s| s.url == server.url) {
            Some(existing) => {
                existing.name = server.name;
                existing.icon = server.icon;
            }
            None => self.saved.servers.push(server),
        }
        self.save()
    }

    pub fn remove(&mut self, url: &str) -> Result<(), String> {
        self.saved.servers.retain(|s| s.url != url);
        if self.saved.last.as_deref() == Some(url) {
            self.saved.last = None;
        }
        self.save()
    }

    pub fn hide(&mut self, url: &str, workspace_id: &str) -> Result<(), String> {
        let Some(server) = self.saved.servers.iter_mut().find(|s| s.url == url) else {
            return Ok(());
        };
        if !server.hidden.iter().any(|id| id == workspace_id) {
            server.hidden.push(workspace_id.to_owned());
        }
        self.save()
    }

    pub fn show_hidden(&mut self, url: &str) -> Result<(), String> {
        if let Some(server) = self.saved.servers.iter_mut().find(|s| s.url == url) {
            server.hidden.clear();
        }
        self.save()
    }

    pub fn set_last(&mut self, url: &str) -> Result<(), String> {
        if self.saved.last.as_deref() == Some(url) || self.find(url).is_none() {
            return Ok(());
        }
        self.saved.last = Some(url.to_owned());
        self.save()
    }

    fn save(&self) -> Result<(), String> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let bytes = serde_json::to_vec_pretty(&self.saved).map_err(|e| e.to_string())?;
        std::fs::write(&self.path, bytes).map_err(|e| e.to_string())
    }
}

pub fn normalize(input: &str) -> Result<Url, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Enter your server's address".into());
    }
    let has_scheme = trimmed.contains("://");
    let with_scheme = if has_scheme {
        trimmed.to_owned()
    } else {
        format!("https://{trimmed}")
    };
    let mut parsed =
        Url::parse(&with_scheme).map_err(|_| format!("{trimmed} is not a web address"))?;
    if !matches!(parsed.scheme(), "https" | "http") || parsed.host_str().is_none() {
        return Err(format!("{trimmed} is not a web address"));
    }
    if !has_scheme && is_loopback(&parsed) {
        parsed
            .set_scheme("http")
            .map_err(|_| format!("{trimmed} is not a web address"))?;
    }
    Url::parse(&parsed.origin().ascii_serialization()).map_err(|e| e.to_string())
}

fn is_loopback(url: &Url) -> bool {
    match url.host() {
        Some(Host::Domain(name)) => name == "localhost" || name.ends_with(".localhost"),
        Some(Host::Ipv4(ip)) => ip.is_loopback(),
        Some(Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    }
}

#[derive(Deserialize)]
struct InstanceInfo {
    name: String,
    icon_url: Option<String>,
}

pub async fn probe(origin: &Url) -> Result<Server, String> {
    let address = origin.as_str().trim_end_matches('/').to_owned();
    let unreachable = || format!("Could not reach a Chat Systems server at {address}");
    let response = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?
        .get(
            origin
                .join("api/instance/info")
                .map_err(|e| e.to_string())?,
        )
        .send()
        .await
        .map_err(|_| unreachable())?;
    if !response.status().is_success() {
        return Err(unreachable());
    }
    let info: InstanceInfo = response.json().await.map_err(|_| unreachable())?;
    let icon = info
        .icon_url
        .and_then(|icon| origin.join(&icon).ok())
        .map(String::from);
    Ok(Server {
        url: address,
        name: info.name,
        icon,
        hidden: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bare_host_becomes_an_https_origin() {
        assert_eq!(
            normalize("chat.example.com").unwrap().as_str(),
            "https://chat.example.com/"
        );
    }

    #[test]
    fn the_path_query_and_fragment_are_dropped_and_the_port_kept() {
        assert_eq!(
            normalize(" http://localhost:8080/app/123?x=1#y ")
                .unwrap()
                .as_str(),
            "http://localhost:8080/"
        );
    }

    #[test]
    fn a_bare_address_on_this_machine_is_plain_http() {
        assert_eq!(
            normalize("localhost:8080").unwrap().as_str(),
            "http://localhost:8080/"
        );
        assert_eq!(
            normalize("127.0.0.1:3000").unwrap().as_str(),
            "http://127.0.0.1:3000/"
        );
        assert_eq!(
            normalize("chat.localhost").unwrap().as_str(),
            "http://chat.localhost/"
        );
        assert_eq!(
            normalize("https://localhost:8443").unwrap().as_str(),
            "https://localhost:8443/"
        );
    }

    #[test]
    fn anything_but_a_web_address_is_refused() {
        assert!(normalize("").is_err());
        assert!(normalize("ftp://chat.example.com").is_err());
        assert!(normalize("file:///etc/passwd").is_err());
        assert!(normalize("https://").is_err());
    }

    #[test]
    fn the_store_keeps_one_entry_per_server_and_forgets_a_removed_last() {
        let dir = std::env::temp_dir().join(format!("chat-systems-store-{}", std::process::id()));
        let mut store = Store::load(&dir);
        let server = |name: &str| Server {
            url: "https://chat.example.com".into(),
            name: name.into(),
            icon: None,
            hidden: Vec::new(),
        };

        store.upsert(server("Old")).unwrap();
        store.upsert(server("New")).unwrap();
        store.set_last("https://chat.example.com").unwrap();
        assert_eq!(Store::load(&dir).servers(), &[server("New")]);
        assert_eq!(Store::load(&dir).last(), Some(&server("New")));

        store.remove("https://chat.example.com").unwrap();
        assert!(Store::load(&dir).last().is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_hidden_workspace_stays_hidden_when_its_server_is_added_again() {
        let dir = std::env::temp_dir().join(format!("chat-systems-hidden-{}", std::process::id()));
        let mut store = Store::load(&dir);
        let server = Server {
            url: "https://chat.example.com".into(),
            name: "Chat".into(),
            icon: None,
            hidden: Vec::new(),
        };

        store.upsert(server.clone()).unwrap();
        store.hide("https://chat.example.com", "a1").unwrap();
        store.hide("https://chat.example.com", "a1").unwrap();
        store.upsert(server).unwrap();
        assert_eq!(Store::load(&dir).servers()[0].hidden, ["a1"]);

        store.show_hidden("https://chat.example.com").unwrap();
        assert!(Store::load(&dir).servers()[0].hidden.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }
}
