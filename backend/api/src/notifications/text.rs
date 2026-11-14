use std::sync::LazyLock;

use serde_json::Value;

use super::models::NotificationData;
use crate::profile::models::Language;

static CATALOG: LazyLock<Catalog> = LazyLock::new(|| {
    Catalog::parse(
        include_str!("../../locales/en.json"),
        include_str!("../../locales/ja.json"),
    )
});

pub struct Catalog {
    en: Value,
    ja: Value,
}

impl Catalog {
    pub fn parse(en: &str, ja: &str) -> Self {
        Self {
            en: serde_json::from_str(en).unwrap_or(Value::Null),
            ja: serde_json::from_str(ja).unwrap_or(Value::Null),
        }
    }

    pub fn words(&self, language: Language) -> Words<'_> {
        let (own, lang) = match language {
            Language::Ja => (Some(&self.ja), "ja"),
            Language::En | Language::Browser => (None, "en"),
        };
        Words {
            own,
            en: &self.en,
            lang,
        }
    }
}

pub fn words(language: Language) -> Words<'static> {
    CATALOG.words(language)
}

pub fn language(code: Option<&str>) -> Language {
    code.and_then(|code| serde_json::from_value(Value::String(code.to_string())).ok())
        .unwrap_or_default()
}

pub fn reader_language(preference: Language, accept_language: Option<&str>) -> Language {
    match (preference, accept_language) {
        (Language::Browser, Some(header)) => from_accept_language(header),
        (preference, _) => preference,
    }
}

fn from_accept_language(header: &str) -> Language {
    let mut ranges: Vec<(f32, &str)> = header
        .split(',')
        .filter_map(|range| {
            let mut parts = range.split(';');
            let tag = parts.next()?.trim();
            let quality = match parts.find_map(|p| p.trim().strip_prefix("q=")) {
                Some(q) => q.trim().parse::<f32>().ok()?,
                None => 1.0,
            };
            (quality > 0.0).then_some((quality, tag))
        })
        .collect();
    ranges.sort_by(|a, b| b.0.total_cmp(&a.0));
    ranges
        .into_iter()
        .find_map(
            |(_, tag)| match tag.split('-').next()?.to_ascii_lowercase().as_str() {
                "en" => Some(Language::En),
                "ja" => Some(Language::Ja),
                _ => None,
            },
        )
        .unwrap_or(Language::Browser)
}

#[derive(Clone, Copy)]
pub struct Words<'a> {
    own: Option<&'a Value>,
    en: &'a Value,
    lang: &'static str,
}

impl Words<'_> {
    pub fn lang(&self) -> &'static str {
        self.lang
    }

    pub fn get(&self, key: &str, vars: &[(&str, &str)]) -> String {
        fill(self.find(&[key]).unwrap_or(key), vars)
    }

    pub fn count(&self, key: &str, n: i64, vars: &[(&str, &str)]) -> String {
        let one = format!("{key}_one");
        let other = format!("{key}_other");
        let keys = if n == 1 {
            vec![one.as_str(), other.as_str(), key]
        } else {
            vec![other.as_str(), key]
        };
        let count = n.to_string();
        let mut all = vec![("count", count.as_str())];
        all.extend_from_slice(vars);
        fill(self.find(&keys).unwrap_or(key), &all)
    }

    fn find(&self, keys: &[&str]) -> Option<&str> {
        self.own
            .into_iter()
            .chain([self.en])
            .find_map(|table| keys.iter().find_map(|key| lookup(table, key)))
    }
}

fn lookup<'v>(table: &'v Value, key: &str) -> Option<&'v str> {
    key.split('.')
        .try_fold(table, |node, part| node.get(part))?
        .as_str()
}

fn fill(template: &str, vars: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else {
            out.push_str(&rest[start..]);
            return out;
        };
        let name = after[..end].trim();
        match vars.iter().find(|(var, _)| *var == name) {
            Some((_, value)) => out.push_str(value),
            None => out.push_str(&rest[start..start + 2 + end + 2]),
        }
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    out
}

pub fn title(words: &Words, kind: &str, data: &NotificationData) -> String {
    let sender = data
        .sender_name
        .clone()
        .unwrap_or_else(|| words.get("activity.someone", &[]));
    let channel = data.channel_name.as_deref().unwrap_or_default();
    match kind {
        "mention" => words.get("activity.mention", &[]),
        "reply" => words.get("activity.reply", &[]),
        "dm" => sender,
        "keyword" => {
            let keyword = data.keyword.as_deref().unwrap_or_default();
            match channel {
                "" => words.get("activity.keyword", &[("keyword", keyword)]),
                channel => words.get(
                    "activity.keywordIn",
                    &[("keyword", keyword), ("channel", channel)],
                ),
            }
        }
        "list" => {
            let item = data
                .item_title
                .clone()
                .unwrap_or_else(|| words.get("activity.untitled", &[]));
            let key = if data.assigned {
                "activity.listAssigned"
            } else {
                "activity.listMentioned"
            };
            words.get(key, &[("sender", &sender), ("item", &item)])
        }
        _ => words.get(
            "activity.message",
            &[("sender", &sender), ("channel", channel)],
        ),
    }
}

pub fn body(words: &Words, kind: &str, data: &NotificationData, content: &str) -> String {
    match (kind, data.assigned) {
        ("list", true) => words.get(
            "activity.listIn",
            &[("list", data.list_title.as_deref().unwrap_or_default())],
        ),
        _ => content.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::*;

    const JA: &str = r##"{
        "activity": {
            "mention": "メンションされました",
            "keywordIn": "#{{channel}} で「{{keyword}}」に一致"
        },
        "email": { "mentions_other": "{{count}} 件のメンション" }
    }"##;

    fn catalog() -> Catalog {
        Catalog::parse(include_str!("../../locales/en.json"), JA)
    }

    fn keyword_in(channel: &str) -> NotificationData {
        NotificationData {
            keyword: Some("deploy".into()),
            channel_name: Some(channel.into()),
            ..Default::default()
        }
    }

    #[test]
    fn the_shipped_catalogs_are_json() {
        for source in [
            include_str!("../../locales/en.json"),
            include_str!("../../locales/ja.json"),
        ] {
            assert!(serde_json::from_str::<Value>(source).is_ok());
        }
        assert_eq!(
            words(Language::En).get("activity.mention", &[]),
            "You were mentioned"
        );
    }

    #[test]
    fn the_server_says_what_the_client_says() {
        let client: Value = serde_json::from_str(include_str!(
            "../../../../frontend/src/locales/en/notifications.json"
        ))
        .expect("the client's strings");
        let server: Value =
            serde_json::from_str(include_str!("../../locales/en.json")).expect("the server's");
        let server = server["activity"].as_object().expect("activity");
        assert!(!server.is_empty());
        for (key, text) in server {
            assert_eq!(client["activity"][key], *text, "activity.{key}");
        }
    }

    #[test]
    fn a_reader_in_japanese_gets_the_japanese_strings_that_exist() {
        let catalog = catalog();
        let ja = catalog.words(Language::Ja);
        assert_eq!(
            title(&ja, "mention", &NotificationData::default()),
            "メンションされました"
        );
        assert_eq!(
            title(&ja, "keyword", &keyword_in("ops")),
            "#ops で「deploy」に一致"
        );
        assert_eq!(ja.count("email.mentions", 1, &[]), "1 件のメンション");
        assert_eq!(ja.count("email.mentions", 3, &[]), "3 件のメンション");
    }

    #[test]
    fn a_string_missing_from_a_language_falls_back_to_english() {
        let catalog = catalog();
        let ja = catalog.words(Language::Ja);
        assert_eq!(
            title(&ja, "reply", &NotificationData::default()),
            "New reply in a thread"
        );
        assert_eq!(ja.count("email.directMessages", 1, &[]), "1 direct message");
        let empty = Catalog::parse(include_str!("../../locales/en.json"), "{}");
        assert_eq!(
            title(
                &empty.words(Language::Ja),
                "mention",
                &NotificationData::default()
            ),
            "You were mentioned"
        );
    }

    #[test]
    fn the_browser_setting_and_english_read_english() {
        let catalog = catalog();
        for language in [Language::Browser, Language::En] {
            assert_eq!(
                title(
                    &catalog.words(language),
                    "mention",
                    &NotificationData::default()
                ),
                "You were mentioned"
            );
        }
        assert_eq!(language(Some("ja")), Language::Ja);
        assert_eq!(language(Some("klingon")), Language::Browser);
        assert_eq!(language(None), Language::Browser);
    }

    #[test]
    fn the_reader_language_is_the_setting_then_the_browser_then_english() {
        assert_eq!(
            reader_language(Language::Ja, Some("en-US,en;q=0.9")),
            Language::Ja
        );
        assert_eq!(reader_language(Language::En, Some("ja")), Language::En);
        assert_eq!(
            reader_language(Language::Browser, Some("ja-JP,ja;q=0.9,en;q=0.8")),
            Language::Ja
        );
        assert_eq!(
            reader_language(Language::Browser, Some("de-DE, en;q=0.5, ja;q=0.7")),
            Language::Ja
        );
        assert_eq!(
            reader_language(Language::Browser, Some("fr-FR, ja;q=0, *;q=0.1")),
            Language::Browser
        );
        assert_eq!(
            reader_language(Language::Browser, Some("EN-gb")),
            Language::En
        );
        assert_eq!(
            reader_language(Language::Browser, Some("ja;q=nonsense, en")),
            Language::En
        );
        assert_eq!(reader_language(Language::Browser, None), Language::Browser);
        assert_eq!(words(reader_language(Language::Browser, None)).lang(), "en");
    }

    #[test]
    fn each_kind_reads_as_it_did() {
        let en = words(Language::En);
        let sent = NotificationData {
            sender_name: Some("Ana".into()),
            channel_name: Some("general".into()),
            ..Default::default()
        };
        assert_eq!(title(&en, "dm", &sent), "Ana");
        assert_eq!(title(&en, "message", &sent), "Ana in #general");
        assert_eq!(
            title(&en, "keyword", &keyword_in("ops")),
            "Matched \"deploy\" in #ops"
        );
        assert_eq!(title(&en, "keyword", &keyword_in("")), "Matched \"deploy\"");
        assert_eq!(title(&en, "dm", &NotificationData::default()), "Somebody");

        let assigned = NotificationData {
            sender_name: Some("Ana".into()),
            item_title: Some("Fix login".into()),
            list_title: Some("Launch plan".into()),
            assigned: true,
            ..Default::default()
        };
        assert_eq!(
            title(&en, "list", &assigned),
            "Ana assigned you to Fix login"
        );
        assert_eq!(body(&en, "list", &assigned, ""), "in Launch plan");
        let commented = NotificationData {
            assigned: false,
            item_title: None,
            ..assigned
        };
        assert_eq!(
            title(&en, "list", &commented),
            "Ana mentioned you on Untitled"
        );
        assert_eq!(body(&en, "list", &commented, "look"), "look");
    }

    #[test]
    fn a_value_is_written_once_and_never_read_as_a_placeholder() {
        let en = words(Language::En);
        let sneaky = NotificationData {
            sender_name: Some("{{channel}}".into()),
            channel_name: Some("general".into()),
            ..Default::default()
        };
        assert_eq!(title(&en, "message", &sneaky), "{{channel}} in #general");
        assert_eq!(fill("{{unknown}} and {{", &[]), "{{unknown}} and {{");
    }

    fn leaves(value: &Value, path: &str) -> Vec<(String, String)> {
        match value {
            Value::Object(map) => map
                .iter()
                .flat_map(|(key, child)| leaves(child, &format!("{path}.{key}")))
                .collect(),
            Value::String(text) => vec![(path.to_string(), text.clone())],
            other => panic!("{path} is {other}, not a string"),
        }
    }

    fn base(key: &str) -> &str {
        key.strip_suffix("_one")
            .or_else(|| key.strip_suffix("_other"))
            .unwrap_or(key)
    }

    fn placeholders(text: &str) -> BTreeSet<&str> {
        text.split("{{")
            .skip(1)
            .filter_map(|rest| rest.split_once("}}").map(|(name, _)| name.trim()))
            .collect()
    }

    const AWAITING_JAPANESE: &[&str] = &[];

    #[test]
    fn every_email_string_has_japanese_with_the_same_placeholders() {
        let catalog = |source| {
            let value: Value = serde_json::from_str(source).expect("a catalog");
            leaves(&value["email"], "email")
        };
        let en = catalog(include_str!("../../locales/en.json"));
        let ja = catalog(include_str!("../../locales/ja.json"));
        let japanese: BTreeMap<&str, &str> = ja
            .iter()
            .map(|(key, text)| (base(key), text.as_str()))
            .collect();
        let english: BTreeSet<&str> = en.iter().map(|(key, _)| base(key)).collect();
        for key in AWAITING_JAPANESE {
            assert!(
                english.contains(key),
                "{key} awaits Japanese but is not in en.json"
            );
            assert!(
                !japanese.contains_key(key),
                "{key} has Japanese now, take it off AWAITING_JAPANESE"
            );
        }
        for (key, text) in &en {
            if AWAITING_JAPANESE.contains(&base(key)) {
                continue;
            }
            let translated = japanese
                .get(base(key))
                .unwrap_or_else(|| panic!("{key} has no Japanese"));
            assert_eq!(placeholders(translated), placeholders(text), "{key}");
        }
        for (key, _) in &ja {
            assert!(english.contains(base(key)), "{key} is not in en.json");
        }
        let ja = words(Language::Ja);
        assert_eq!(ja.count("email.mentions", 1, &[]), "1件のメンション");
        assert_eq!(
            ja.count("email.code.expiry", 1, &[]),
            "コードは1回だけ使えます。有効期限は1分です。"
        );
    }
}
