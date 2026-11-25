use serde::{Deserialize, Serialize};

const MOST: usize = 100;
const LONGEST_NAME: usize = 80;
const ICON_PREFIX: &str = "data:image/png;base64,";
const LARGEST_ICON: usize = 128 * 1024;

#[derive(Debug, Clone, Deserialize)]
pub struct Reported {
    id: String,
    name: String,
    icon: Option<String>,
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

fn is_icon(icon: &str) -> bool {
    icon.len() <= LARGEST_ICON
        && icon.strip_prefix(ICON_PREFIX).is_some_and(|data| {
            data.bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
        })
}

pub fn accept(reported: Vec<Reported>) -> Vec<Workspace> {
    reported
        .into_iter()
        .filter(|w| is_id(&w.id))
        .take(MOST)
        .map(|w| Workspace {
            name: w.name.trim().chars().take(LONGEST_NAME).collect(),
            icon: w.icon.filter(|icon| is_icon(icon)),
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
            icon: icon.map(Into::into),
            unread: 3,
            deleted: false,
        }
    }

    #[test]
    fn a_workspace_icon_is_a_small_png_the_page_drew() {
        let png = "data:image/png;base64,iVBORw0KGgo=";
        let huge = format!("{ICON_PREFIX}{}", "A".repeat(LARGEST_ICON));
        let accepted = accept(vec![
            reported("a1", Some(png)),
            reported("b2", Some("https://tracker.example.net/pixel.png")),
            reported("c3", Some("data:image/svg+xml;base64,PHN2Zz4=")),
            reported("d4", Some("data:image/png;base64,\"><img>")),
            reported("e5", Some(&huge)),
        ]);
        assert_eq!(accepted[0].icon.as_deref(), Some(png));
        assert_eq!(accepted[0].name, "Northwind");
        assert!(accepted[1..].iter().all(|w| w.icon.is_none()));
    }

    #[test]
    fn an_id_that_could_break_out_of_a_script_is_dropped() {
        let accepted = accept(vec![
            reported("a1');alert(1)//", None),
            reported("0f-9e", None),
        ]);
        assert_eq!(accepted.len(), 1);
        assert_eq!(accepted[0].id, "0f-9e");
    }

    #[test]
    fn a_page_cannot_flood_the_rail() {
        let many = (0..500)
            .map(|i| reported(&format!("{i:x}"), None))
            .collect();
        assert_eq!(accept(many).len(), MOST);
    }
}
