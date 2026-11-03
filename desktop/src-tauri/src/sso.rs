use std::collections::HashMap;
use std::time::{Duration, Instant};

use url::Url;

pub const SCHEME: &str = "chat-systems";
const LONGEST_SIGN_IN: Duration = Duration::from_secs(600);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finished {
    pub server: String,
    pub code: String,
}

#[derive(Debug, Default)]
pub struct Pending {
    attempts: HashMap<String, (String, Instant)>,
}

fn is_token(value: &str) -> bool {
    (16..=128).contains(&value.len())
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn new_nonce() -> Result<String, String> {
    let mut bytes = [0u8; 24];
    getrandom::fill(&mut bytes).map_err(|e| e.to_string())?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

impl Pending {
    pub fn begin(&mut self, server: &str, nonce: &str, now: Instant) {
        self.attempts
            .retain(|_, (_, started)| now.duration_since(*started) < LONGEST_SIGN_IN);
        self.attempts
            .insert(nonce.to_owned(), (server.to_owned(), now));
    }

    pub fn finish(&mut self, link: &Url, now: Instant) -> Option<Finished> {
        if link.scheme() != SCHEME || link.host_str() != Some("sso") {
            return None;
        }
        let value = |name: &str| {
            link.query_pairs()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.into_owned())
        };
        let nonce = value("nonce")?;
        let code = value("code").filter(|code| is_token(code))?;
        let (server, started) = self.attempts.remove(&nonce)?;
        (now.duration_since(started) < LONGEST_SIGN_IN).then_some(Finished { server, code })
    }
}

pub fn start_url(server: &Url, nonce: &str, invite: Option<&str>) -> Option<Url> {
    let mut url = server.join("/api/auth/oidc/start").ok()?;
    {
        let mut query = url.query_pairs_mut();
        query
            .append_pair("client", "desktop")
            .append_pair("nonce", nonce);
        if let Some(invite) = invite.filter(|invite| !invite.is_empty()) {
            query.append_pair("invite", invite);
        }
    }
    Some(url)
}

pub fn complete_url(server: &Url, code: &str) -> Option<Url> {
    let mut url = server.join("/sso/complete").ok()?;
    url.query_pairs_mut().append_pair("code", code);
    Some(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SERVER: &str = "https://chat.example.com/";
    const CODE: &str = "c29tZS1vbmUtdGltZS1jb2Rl";

    fn link(code: &str, nonce: &str) -> Url {
        Url::parse(&format!("chat-systems://sso?code={code}&nonce={nonce}")).unwrap()
    }

    #[test]
    fn a_sign_in_this_app_started_finishes_once_on_its_server() {
        let mut pending = Pending::default();
        let now = Instant::now();
        let nonce = new_nonce().unwrap();
        pending.begin(SERVER, &nonce, now);

        assert_eq!(
            pending.finish(&link(CODE, &nonce), now),
            Some(Finished {
                server: SERVER.into(),
                code: CODE.into()
            })
        );
        assert_eq!(pending.finish(&link(CODE, &nonce), now), None);
    }

    #[test]
    fn a_link_this_app_did_not_ask_for_is_ignored() {
        let mut pending = Pending::default();
        let now = Instant::now();
        pending.begin(SERVER, "0123456789abcdef0123", now);

        assert_eq!(
            pending.finish(&link(CODE, "someone-elses-nonce"), now),
            None
        );
        assert_eq!(
            pending.finish(&link("x');alert(1)//", "0123456789abcdef0123"), now),
            None
        );
        assert_eq!(
            pending.finish(
                &Url::parse(
                    "chat-systems://other?code=c29tZS1vbmUtdGltZS1jb2Rl&nonce=0123456789abcdef0123"
                )
                .unwrap(),
                now
            ),
            None
        );
    }

    #[test]
    fn a_sign_in_left_too_long_is_forgotten() {
        let mut pending = Pending::default();
        let then = Instant::now();
        pending.begin(SERVER, "0123456789abcdef0123", then);

        let later = then + LONGEST_SIGN_IN;
        assert_eq!(
            pending.finish(&link(CODE, "0123456789abcdef0123"), later),
            None
        );
    }

    #[test]
    fn the_browser_starts_on_the_server_and_the_app_finishes_there() {
        let server = Url::parse("https://chat.example.com/team/").unwrap();
        let start = start_url(&server, "n0nce", Some("in vite&x=1")).unwrap();
        assert_eq!(
            start.as_str(),
            "https://chat.example.com/api/auth/oidc/start?client=desktop&nonce=n0nce&invite=in+vite%26x%3D1"
        );
        assert_eq!(
            complete_url(&server, CODE).unwrap().as_str(),
            "https://chat.example.com/sso/complete?code=c29tZS1vbmUtdGltZS1jb2Rl"
        );
    }
}
