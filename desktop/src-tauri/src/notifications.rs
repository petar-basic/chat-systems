use serde::Deserialize;

use crate::workspaces;

const LONGEST_TITLE: usize = 120;
const LONGEST_BODY: usize = 400;

#[derive(Debug, Clone, Deserialize)]
pub struct Requested {
    key: String,
    title: String,
    #[serde(default)]
    subtitle: String,
    #[serde(default)]
    body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub key: String,
    pub title: String,
    pub subtitle: String,
    pub body: String,
}

fn cut(text: &str, most: usize) -> String {
    text.trim().chars().take(most).collect()
}

pub fn accept(requested: Requested) -> Option<Note> {
    let title = cut(&requested.title, LONGEST_TITLE);
    (workspaces::is_id(&requested.key) && !title.is_empty()).then(|| Note {
        key: requested.key,
        title,
        subtitle: cut(&requested.subtitle, LONGEST_TITLE),
        body: cut(&requested.body, LONGEST_BODY),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn requested(key: &str, title: &str, body: &str) -> Requested {
        Requested {
            key: key.into(),
            title: title.into(),
            subtitle: "  Northwind · #general  ".into(),
            body: body.into(),
        }
    }

    #[test]
    fn a_notification_is_cut_to_what_a_banner_shows() {
        let note = accept(requested("1f", "  Maya  ", &"x".repeat(1000))).unwrap();
        assert_eq!(note.title, "Maya");
        assert_eq!(note.subtitle, "Northwind · #general");
        assert_eq!(note.body.chars().count(), LONGEST_BODY);
    }

    #[test]
    fn a_key_that_could_break_out_of_a_script_is_refused() {
        assert!(accept(requested("1');alert(1)//", "Maya", "hi")).is_none());
    }

    #[test]
    fn a_notification_without_a_title_is_refused() {
        assert!(accept(requested("1f", "   ", "hi")).is_none());
    }
}
