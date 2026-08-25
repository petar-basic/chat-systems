use std::collections::HashMap;

use uuid::Uuid;

/// Slack's `mrkdwn` into the markdown subset the composer produces.
///
/// Anything with no equivalent degrades to the text a person would read rather
/// than being dropped: a mention of somebody who is not in the export becomes
/// their Slack handle, a link with no label becomes the URL.
pub fn to_markdown(text: &str, users: &HashMap<String, (Uuid, String)>) -> String {
    let unescaped = text
        .replace("&lt;", "\u{0}lt\u{0}")
        .replace("&gt;", "\u{0}gt\u{0}")
        .replace("&amp;", "&");

    let with_entities = resolve_entities(&unescaped, users);

    with_entities
        .replace("\u{0}lt\u{0}", "<")
        .replace("\u{0}gt\u{0}", ">")
}

/// `<@U1>`, `<#C1|general>`, `<https://example.com|label>`, `<!here>`.
fn resolve_entities(text: &str, users: &HashMap<String, (Uuid, String)>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(start) = rest.find('<') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('>') else {
            // An unclosed angle bracket is text, not a broken entity.
            out.push_str(&rest[start..]);
            return out;
        };
        let entity = &after[..end];
        out.push_str(&render_entity(entity, users));
        rest = &after[end + 1..];
    }

    out.push_str(rest);
    out
}

fn render_entity(entity: &str, users: &HashMap<String, (Uuid, String)>) -> String {
    let (target, label) = match entity.split_once('|') {
        Some((target, label)) => (target, Some(label)),
        None => (entity, None),
    };

    if let Some(slack_id) = target.strip_prefix('@') {
        return match users.get(slack_id) {
            // The composer's own mention form, so the highlighter and the
            // notification path see an imported mention exactly as a live one.
            Some((user_id, name)) => format!("@[{name}]({user_id})"),
            None => format!("@{}", label.unwrap_or(slack_id)),
        };
    }

    if let Some(channel) = target.strip_prefix('#') {
        let name = label.unwrap_or_else(|| channel.split_once('|').map_or(channel, |(_, n)| n));
        return format!("#{name}");
    }

    if let Some(special) = target.strip_prefix('!') {
        return match special {
            "here" | "channel" | "everyone" => format!("@{special}"),
            other => format!("@{}", label.unwrap_or(other)),
        };
    }

    match label {
        Some(label) => format!("[{label}]({target})"),
        None => target.to_string(),
    }
}

/// `*bold*` is Slack's; `**bold**` is everyone else's. Italic and code already
/// agree, and strikethrough needs its second tilde.
pub fn to_markdown_marks(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_code = false;

    for c in text.chars() {
        match c {
            '`' => {
                in_code = !in_code;
                out.push(c);
            }
            '*' if !in_code => out.push_str("**"),
            '~' if !in_code => out.push_str("~~"),
            _ => out.push(c),
        }
    }

    out
}

pub fn convert(text: &str, users: &HashMap<String, (Uuid, String)>) -> String {
    to_markdown_marks(&to_markdown(text, users))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn users() -> HashMap<String, (Uuid, String)> {
        let mut map = HashMap::new();
        map.insert(
            "U1".to_string(),
            (
                Uuid::parse_str("11111111-1111-4111-8111-111111111111").expect("uuid"),
                "Ana".to_string(),
            ),
        );
        map
    }

    #[test]
    fn a_known_mention_becomes_the_composers_own_form() {
        assert_eq!(
            convert("hey <@U1> look", &users()),
            "hey @[Ana](11111111-1111-4111-8111-111111111111) look"
        );
    }

    #[test]
    fn an_unknown_mention_keeps_the_handle_rather_than_vanishing() {
        assert_eq!(convert("ping <@U9>", &users()), "ping @U9");
        assert_eq!(convert("ping <@U9|ivan>", &users()), "ping @ivan");
    }

    #[test]
    fn links_keep_their_label() {
        assert_eq!(
            convert("see <https://example.com|the docs>", &users()),
            "see [the docs](https://example.com)"
        );
        assert_eq!(
            convert("see <https://example.com>", &users()),
            "see https://example.com"
        );
    }

    #[test]
    fn channels_and_broadcasts_read_as_they_did() {
        assert_eq!(convert("in <#C1|general>", &users()), "in #general");
        assert_eq!(convert("<!here> please", &users()), "@here please");
    }

    #[test]
    fn marks_are_translated_outside_code() {
        assert_eq!(
            convert("*bold* and ~gone~", &users()),
            "**bold** and ~~gone~~"
        );
        assert_eq!(
            convert("`a * b` stays", &users()),
            "`a * b` stays",
            "a literal asterisk inside code is not emphasis"
        );
    }

    #[test]
    fn escaped_entities_come_back_as_characters() {
        assert_eq!(
            convert("2 &lt; 3 &amp;&amp; 4 &gt; 1", &users()),
            "2 < 3 && 4 > 1"
        );
    }

    #[test]
    fn an_unclosed_bracket_is_text() {
        assert_eq!(convert("a < b", &users()), "a < b");
    }
}
