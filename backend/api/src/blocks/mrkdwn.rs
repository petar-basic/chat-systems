use std::collections::HashMap;

use uuid::Uuid;

use super::shortcodes;

pub struct Names<'a> {
    pub users: &'a HashMap<String, (Uuid, String)>,
    pub channels: &'a HashMap<String, String>,
}

/// Slack's `mrkdwn` into the markdown subset the composer produces.
///
/// Anything with no equivalent degrades to the text a person would read rather
/// than being dropped: a mention of somebody who is not in the export becomes
/// their Slack handle, a link with no label becomes the URL.
pub fn to_markdown(text: &str, names: &Names) -> String {
    let unescaped = text
        .replace("&lt;", "\u{0}lt\u{0}")
        .replace("&gt;", "\u{0}gt\u{0}")
        .replace("&amp;", "&");

    let with_entities = resolve_entities(&unescaped, names);

    with_entities
        .replace("\u{0}lt\u{0}", "<")
        .replace("\u{0}gt\u{0}", ">")
}

/// `<@U1>`, `<#C1|general>`, `<https://example.com|label>`, `<!here>`.
fn resolve_entities(text: &str, names: &Names) -> String {
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
        out.push_str(&render_entity(entity, names));
        rest = &after[end + 1..];
    }

    out.push_str(rest);
    out
}

fn render_entity(entity: &str, names: &Names) -> String {
    let (target, label) = match entity.split_once('|') {
        Some((target, label)) => (target, Some(label)),
        None => (entity, None),
    };

    if let Some(slack_id) = target.strip_prefix('@') {
        return match names.users.get(slack_id) {
            // The composer's own mention form, so the highlighter and the
            // notification path see an imported mention exactly as a live one.
            Some((user_id, name)) => format!("@[{name}]({user_id})"),
            None => format!("@{}", label.unwrap_or(slack_id)),
        };
    }

    if let Some(channel) = target.strip_prefix('#') {
        let name = names
            .channels
            .get(channel)
            .map(String::as_str)
            .or(label)
            .unwrap_or(channel);
        return format!("#{name}");
    }

    if let Some(special) = target.strip_prefix('!') {
        return match special {
            "here" | "channel" | "everyone" => format!("@{special}"),
            date if date.starts_with("date^") => label.unwrap_or_default().to_string(),
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
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '`' => {
                in_code = !in_code;
                out.push(c);
            }
            '*' | '~' if !in_code => {
                if chars.peek() == Some(&c) {
                    chars.next();
                }
                out.push(c);
                out.push(c);
            }
            _ => out.push(c),
        }
    }

    out
}

pub fn convert(text: &str, users: &HashMap<String, (Uuid, String)>) -> String {
    let channels = HashMap::new();
    to_markdown_marks(&to_markdown(
        text,
        &Names {
            users,
            channels: &channels,
        },
    ))
}

pub fn convert_block_text(text: &str, names: &Names) -> String {
    hard_breaks(&emojify(&to_markdown_marks(&to_markdown(text, names))))
}

pub fn emojify(text: &str) -> String {
    text.split('`')
        .enumerate()
        .map(|(i, segment)| {
            if i % 2 == 0 {
                shortcodes_in(segment)
            } else {
                segment.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("`")
}

fn shortcodes_in(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    let mut rest = segment;
    while let Some(start) = rest.find(':') {
        let after = &rest[start + 1..];
        let native = emoji_name(after)
            .and_then(|len| shortcodes::standard(&after[..len]).map(|native| (len, native)));
        match native {
            Some((len, native)) => {
                out.push_str(&rest[..start]);
                out.push_str(native);
                rest = &after[len + 1..];
            }
            None => {
                out.push_str(&rest[..=start]);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

fn emoji_name(after: &str) -> Option<usize> {
    let end = after.find(':')?;
    let name = &after[..end];
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '+' | '-'))
    {
        return None;
    }
    const TONE: &str = ":skin-tone-";
    let toned = after[end + 1..].strip_prefix(TONE).and_then(|tone| {
        let close = tone.find(':')?;
        (close > 0 && tone[..close].chars().all(|c| c.is_ascii_digit()))
            .then_some(end + 1 + TONE.len() + close)
    });
    Some(toned.unwrap_or(end))
}

fn hard_breaks(text: &str) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out = String::with_capacity(text.len() + lines.len());
    let mut in_code = false;
    for (i, line) in lines.iter().enumerate() {
        out.push_str(line);
        let fence = line.contains("```");
        if line.matches("```").count() % 2 == 1 {
            in_code = !in_code;
        }
        let Some(next) = lines.get(i + 1) else {
            break;
        };
        if !in_code && !fence && !line.trim().is_empty() && continues(next) {
            out.push('\\');
        }
        out.push('\n');
    }
    out
}

fn continues(line: &str) -> bool {
    let line = line.trim_start();
    let ordered = line
        .split_once(". ")
        .is_some_and(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
    !(line.is_empty()
        || ordered
        || line.starts_with("```")
        || line.starts_with('>')
        || line.starts_with("- ")
        || line.starts_with("* ")
        || line.starts_with("+ "))
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

    fn names_of<'a>(
        users: &'a HashMap<String, (Uuid, String)>,
        channels: &'a HashMap<String, String>,
    ) -> Names<'a> {
        Names { users, channels }
    }

    #[test]
    fn block_text_names_people_and_channels_of_this_workspace() {
        let ana = "11111111-1111-4111-8111-111111111111";
        let mut users = HashMap::new();
        users.insert(
            ana.to_string(),
            (Uuid::parse_str(ana).expect("uuid"), "Ana".to_string()),
        );
        let deploys = "22222222-2222-4222-8222-222222222222";
        let mut channels = HashMap::new();
        channels.insert(deploys.to_string(), "deploys".to_string());
        assert_eq!(
            convert_block_text(
                &format!("<@{ana}> shipped to <#{deploys}>"),
                &names_of(&users, &channels)
            ),
            format!("@[Ana]({ana}) shipped to #deploys")
        );
        assert_eq!(
            convert_block_text(
                "<!date^1700000000^{date}|14 Nov 2023> done",
                &names_of(&users, &channels)
            ),
            "14 Nov 2023 done",
            "a date reads as its fallback, not as a mention"
        );
    }

    #[test]
    fn block_text_turns_shortcodes_into_emoji_outside_code() {
        let none = HashMap::new();
        let channels = HashMap::new();
        let names = names_of(&none, &channels);
        assert_eq!(
            convert_block_text(":white_check_mark: passed :+1::skin-tone-3:", &names),
            "✅ passed 👍🏼"
        );
        assert_eq!(
            convert_block_text("`:tada:` and :not_a_real_emoji:", &names),
            "`:tada:` and :not_a_real_emoji:",
            "code stays literal, and an unknown name is left for custom emoji"
        );
        assert_eq!(
            convert_block_text("at 10:30: done", &names),
            "at 10:30: done"
        );
    }

    #[test]
    fn block_text_keeps_its_line_breaks_but_not_inside_code_or_before_a_list() {
        let none = HashMap::new();
        let channels = HashMap::new();
        let names = names_of(&none, &channels);
        assert_eq!(
            convert_block_text("*Status:*\nSuccess", &names),
            "**Status:**\\\nSuccess"
        );
        assert_eq!(
            convert_block_text("```\nline 1\nline 2\n```\nafter", &names),
            "```\nline 1\nline 2\n```\nafter"
        );
        assert_eq!(
            convert_block_text("Changes:\n- one\n- two", &names),
            "Changes:\n- one\n- two"
        );
        assert_eq!(convert_block_text("a\n\nb", &names), "a\n\nb");
    }

    #[test]
    fn marks_already_doubled_are_not_doubled_again() {
        assert_eq!(
            convert("**Firing** and ~~gone~~", &users()),
            "**Firing** and ~~gone~~"
        );
    }
}
