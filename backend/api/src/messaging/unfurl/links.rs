const MAX_LINKS: usize = 3;
const TRAILING_PUNCTUATION: &[char] = &['.', ',', ';', ':', '!', '?', '\'', '"'];

pub fn extract(content: &str) -> Vec<String> {
    let mut links: Vec<String> = Vec::new();
    for segment in prose_segments(content) {
        let mut rest = segment;
        while let Some(start) = next_scheme(rest) {
            let candidate = &rest[start..];
            let end = candidate
                .find(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"' | '`' | ']'))
                .unwrap_or(candidate.len());
            let link = trim_link(&candidate[..end]);
            rest = &candidate[end..];
            if reqwest::Url::parse(link).is_ok_and(|url| url.host_str().is_some())
                && !links.iter().any(|known| known == link)
            {
                links.push(link.to_string());
                if links.len() == MAX_LINKS {
                    return links;
                }
            }
        }
    }
    links
}

fn next_scheme(text: &str) -> Option<usize> {
    let http = text.find("http://");
    let https = text.find("https://");
    match (http, https) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

fn trim_link(link: &str) -> &str {
    let mut link = link.trim_end_matches(TRAILING_PUNCTUATION);
    while link.ends_with(')') && link.matches(')').count() > link.matches('(').count() {
        link = link[..link.len() - 1].trim_end_matches(TRAILING_PUNCTUATION);
    }
    link
}

fn prose_segments(content: &str) -> Vec<&str> {
    let mut segments = Vec::new();
    for (index, block) in content.split("```").enumerate() {
        if index % 2 == 1 {
            continue;
        }
        for (inline, piece) in block.split('`').enumerate() {
            if inline % 2 == 0 {
                segments.push(piece);
            }
        }
    }
    segments
}

#[cfg(test)]
mod tests {
    use super::extract;

    #[test]
    fn finds_links_in_prose() {
        assert_eq!(
            extract("see https://example.com/a and http://example.org/b."),
            vec!["https://example.com/a", "http://example.org/b"]
        );
    }

    #[test]
    fn ignores_code() {
        assert!(extract("`https://example.com` and\n```\nhttps://example.org\n```").is_empty());
    }

    #[test]
    fn keeps_balanced_parentheses_and_drops_the_closing_one() {
        assert_eq!(
            extract("(see https://en.wikipedia.org/wiki/Rust_(programming_language))"),
            vec!["https://en.wikipedia.org/wiki/Rust_(programming_language)"]
        );
    }

    #[test]
    fn reads_markdown_links() {
        assert_eq!(
            extract("[the doc](https://example.com/doc)"),
            vec!["https://example.com/doc"]
        );
    }

    #[test]
    fn stops_at_three_and_skips_duplicates() {
        let content = "https://a.test https://a.test https://b.test https://c.test https://d.test";
        assert_eq!(
            extract(content),
            vec!["https://a.test", "https://b.test", "https://c.test"]
        );
    }

    #[test]
    fn ignores_a_bare_scheme() {
        assert!(extract("type https:// then the host").is_empty());
    }
}
