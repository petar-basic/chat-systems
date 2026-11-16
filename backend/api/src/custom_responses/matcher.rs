use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

pub fn fold(text: &str) -> String {
    let flat = flatten_mentions(text);
    let lowered: String = flat.nfkc().flat_map(char::to_lowercase).collect();
    lowered.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn flatten_mentions(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("@[") {
        let after = &rest[start + 2..];
        let Some(close) = after.find("](") else {
            break;
        };
        let tail = &after[close + 2..];
        let Some(end) = tail.find(')') else {
            break;
        };
        out.push_str(&rest[..start]);
        out.push('@');
        out.push_str(&after[..close]);
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    out
}

fn unspaced(c: char) -> bool {
    matches!(
        c as u32,
        0x0E00..=0x0EFF
            | 0x1000..=0x109F
            | 0x1780..=0x17FF
            | 0x2E80..=0x2FDF
            | 0x3005..=0x3007
            | 0x3040..=0x30FF
            | 0x31F0..=0x31FF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xF900..=0xFAFF
            | 0xFF66..=0xFF9F
            | 0x20000..=0x3FFFF
    )
}

fn glued(outside: Option<char>, edge: char) -> bool {
    outside.is_some_and(|c| {
        c.is_alphanumeric() && edge.is_alphanumeric() && !unspaced(c) && !unspaced(edge)
    })
}

pub fn find(text: &str, phrase: &str) -> Option<usize> {
    let first = phrase.chars().next()?;
    let last = phrase.chars().next_back()?;
    let mut from = 0;
    while let Some(offset) = text[from..].find(phrase) {
        let at = from + offset;
        let before = text[..at].chars().next_back();
        let after = text[at + phrase.len()..].chars().next();
        if !glued(before, first) && !glued(after, last) {
            return Some(at);
        }
        from = at + first.len_utf8();
    }
    None
}

pub struct Matchable<'a> {
    pub id: Uuid,
    pub triggers: &'a [String],
}

pub fn ranked(text: &str, responses: &[Matchable<'_>]) -> Vec<Uuid> {
    let text = fold(text);
    let mut hits: Vec<(usize, std::cmp::Reverse<usize>, Uuid)> = responses
        .iter()
        .filter_map(|response| {
            response
                .triggers
                .iter()
                .map(|trigger| fold(trigger))
                .filter_map(|phrase| {
                    find(&text, &phrase).map(|at| (at, std::cmp::Reverse(phrase.len())))
                })
                .min()
                .map(|(at, len)| (at, len, response.id))
        })
        .collect();
    hits.sort();
    hits.into_iter().map(|(_, _, id)| id).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(text: &str, phrase: &str) -> bool {
        find(&fold(text), &fold(phrase)).is_some()
    }

    #[test]
    fn a_phrase_matches_whole_words_whatever_the_case() {
        assert!(hit("what's the WiFi password?", "wifi password"));
        assert!(hit("Wifi   password", "wifi password"));
        assert!(hit("wifi password", "WIFI PASSWORD"));
        assert!(!hit("the wifi passwords", "wifi password"));
        assert!(!hit("mywifi password", "wifi password"));
        assert!(!hit("wifi", "wifi password"));
    }

    #[test]
    fn punctuation_around_the_phrase_is_a_boundary() {
        assert!(hit("(wifi password)", "wifi password"));
        assert!(hit("\"wifi password\".", "wifi password"));
        assert!(hit("**wifi password**", "wifi password"));
        assert!(hit("vpn?", "vpn"));
        assert!(hit("vpn-guide", "vpn"));
        assert!(!hit("vpns", "vpn"));
    }

    #[test]
    fn a_phrase_with_punctuation_at_its_edge_needs_no_boundary_there() {
        assert!(hit("ok?help", "?help"));
        assert!(hit("say !lunch now", "!lunch"));
    }

    #[test]
    fn japanese_matches_inside_running_text() {
        assert!(hit("会議室の予約方法を教えて", "予約方法"));
        assert!(hit("Wi-Fiのパスワードは？", "パスワード"));
        assert!(hit("wifiのパスワード", "wifi"));
        assert!(hit("ＷｉＦｉパスワード", "wifi"));
        assert!(!hit("予約の方法", "予約方法"));
    }

    #[test]
    fn a_later_occurrence_counts_when_the_first_is_inside_a_word() {
        assert!(hit("xha ha ha", "ha ha"));
        assert!(hit("wifiwifi wifi", "wifi"));
    }

    #[test]
    fn mentions_match_by_their_label() {
        assert_eq!(fold("ask @[Mia Wong](abc) now"), "ask @mia wong now");
        assert!(hit(
            "@[Help Desk](123) wifi password",
            "help desk wifi password"
        ));
    }

    #[test]
    fn the_earliest_then_longest_match_wins() {
        let a = Uuid::from_u128(1);
        let b = Uuid::from_u128(2);
        let c = Uuid::from_u128(3);
        let wifi = vec!["wifi".to_string()];
        let password = vec!["wifi password".to_string()];
        let vpn = vec!["vpn".to_string(), "printer".to_string()];
        let responses = [
            Matchable {
                id: a,
                triggers: &wifi,
            },
            Matchable {
                id: b,
                triggers: &password,
            },
            Matchable {
                id: c,
                triggers: &vpn,
            },
        ];
        assert_eq!(
            ranked("the printer and the wifi password", &responses),
            vec![c, b, a]
        );
        assert!(ranked("nothing here", &responses).is_empty());
    }
}
