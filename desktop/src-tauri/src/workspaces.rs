use serde::{Deserialize, Serialize};
use url::Url;

use crate::navigation;

const MOST: usize = 100;
const LONGEST_NAME: usize = 80;

#[derive(Debug, Clone, Deserialize)]
pub struct Reported {
    id: String,
    name: String,
    icon_url: Option<String>,
    #[serde(default)]
    unread: u32,
    #[serde(default)]
    deleted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub unread: u32,
    pub deleted: bool,
}

pub fn is_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

pub fn accept(server: &Url, reported: Vec<Reported>) -> Vec<Workspace> {
    reported
        .into_iter()
        .filter(|w| is_id(&w.id))
        .take(MOST)
        .map(|w| Workspace {
            name: w.name.trim().chars().take(LONGEST_NAME).collect(),
            icon: w
                .icon_url
                .and_then(|icon| server.join(&icon).ok())
                .filter(|icon| icon.scheme() != "about" && navigation::same_origin(server, icon))
                .map(String::from),
            id: w.id,
            unread: w.unread,
            deleted: w.deleted,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reported(id: &str, icon: Option<&str>) -> Reported {
        Reported {
            id: id.into(),
            name: "  Northwind  ".into(),
            icon_url: icon.map(Into::into),
            unread: 3,
            deleted: false,
        }
    }

    #[test]
    fn a_workspace_icon_must_come_from_its_own_server() {
        let server = Url::parse("https://chat.example.com/").unwrap();
        let accepted = accept(
            &server,
            vec![
                reported("a1", Some("/api/files/1/icon")),
                reported("b2", Some("https://tracker.example.net/pixel.png")),
                reported("c3", Some("about:blank")),
            ],
        );
        assert_eq!(
            accepted[0].icon.as_deref(),
            Some("https://chat.example.com/api/files/1/icon")
        );
        assert_eq!(accepted[0].name, "Northwind");
        assert!(accepted[1].icon.is_none());
        assert!(accepted[2].icon.is_none());
    }

    #[test]
    fn an_id_that_could_break_out_of_a_script_is_dropped() {
        let server = Url::parse("https://chat.example.com/").unwrap();
        let accepted = accept(
            &server,
            vec![reported("a1');alert(1)//", None), reported("0f-9e", None)],
        );
        assert_eq!(accepted.len(), 1);
        assert_eq!(accepted[0].id, "0f-9e");
    }

    #[test]
    fn a_page_cannot_flood_the_rail() {
        let server = Url::parse("https://chat.example.com/").unwrap();
        let many = (0..500)
            .map(|i| reported(&format!("{i:x}"), None))
            .collect();
        assert_eq!(accept(&server, many).len(), MOST);
    }
}
