use crate::messaging::models::Unfurl;

const TITLE_MAX: usize = 200;
const DESCRIPTION_MAX: usize = 300;
const SITE_MAX: usize = 100;

pub fn parse(html: &str, page: &reqwest::Url, requested: &str) -> Option<Unfurl> {
    let head = head_of(html);
    let mut meta = Meta::default();
    for tag in tags(head, "meta") {
        let key = attribute(tag, "property")
            .or_else(|| attribute(tag, "name"))
            .map(|key| key.to_ascii_lowercase());
        let (Some(key), Some(content)) = (key, attribute(tag, "content")) else {
            continue;
        };
        meta.record(&key, decode(&content));
    }

    let title = meta
        .og_title
        .or(meta.twitter_title)
        .or_else(|| title_element(head))
        .map(|title| clip(&title, TITLE_MAX))
        .filter(|title| !title.is_empty())?;
    let description = meta
        .og_description
        .or(meta.twitter_description)
        .or(meta.description)
        .map(|text| clip(&text, DESCRIPTION_MAX))
        .filter(|text| !text.is_empty());
    let site_name = meta
        .og_site_name
        .map(|site| clip(&site, SITE_MAX))
        .filter(|site| !site.is_empty())
        .or_else(|| {
            page.host_str()
                .map(|host| host.trim_start_matches("www.").to_string())
        });
    let image_url = meta
        .og_image
        .or(meta.twitter_image)
        .and_then(|image| page.join(image.trim()).ok())
        .filter(|image| matches!(image.scheme(), "http" | "https"))
        .map(|image| image.to_string());
    if description.is_none() && image_url.is_none() {
        return None;
    }

    Some(Unfurl {
        url: requested.to_string(),
        title,
        description,
        site_name,
        image_url,
        ..Unfurl::default()
    })
}

#[derive(Default)]
struct Meta {
    og_title: Option<String>,
    og_description: Option<String>,
    og_site_name: Option<String>,
    og_image: Option<String>,
    twitter_title: Option<String>,
    twitter_description: Option<String>,
    twitter_image: Option<String>,
    description: Option<String>,
}

impl Meta {
    fn record(&mut self, key: &str, value: String) {
        let slot = match key {
            "og:title" => &mut self.og_title,
            "og:description" => &mut self.og_description,
            "og:site_name" => &mut self.og_site_name,
            "og:image" | "og:image:url" | "og:image:secure_url" => &mut self.og_image,
            "twitter:title" => &mut self.twitter_title,
            "twitter:description" => &mut self.twitter_description,
            "twitter:image" | "twitter:image:src" => &mut self.twitter_image,
            "description" => &mut self.description,
            _ => return,
        };
        if slot.is_none() {
            *slot = Some(value);
        }
    }
}

fn head_of(html: &str) -> &str {
    let lower = html.to_ascii_lowercase();
    match lower.find("</head") {
        Some(end) => &html[..end],
        None => html,
    }
}

fn tags<'a>(html: &'a str, name: &str) -> Vec<&'a str> {
    let lower = html.to_ascii_lowercase();
    let opening = format!("<{name}");
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(offset) = lower[from..].find(&opening) {
        let start = from + offset + opening.len();
        let boundary = lower[start..].chars().next();
        if !boundary.is_some_and(|c| c.is_whitespace() || c == '/' || c == '>') {
            from = start;
            continue;
        }
        let Some(end) = tag_end(&html[start..]) else {
            break;
        };
        found.push(&html[start..start + end]);
        from = start + end;
    }
    found
}

fn tag_end(rest: &str) -> Option<usize> {
    let mut quote: Option<char> = None;
    for (index, c) in rest.char_indices() {
        match (quote, c) {
            (Some(open), c) if c == open => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '>') => return Some(index),
            (None, _) => {}
        }
    }
    None
}

fn attribute(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let mut from = 0;
    while let Some(offset) = lower[from..].find(name) {
        let start = from + offset;
        from = start + name.len();
        let preceded = lower[..start]
            .chars()
            .next_back()
            .is_none_or(|c| c.is_whitespace() || c == '/');
        let rest = lower[from..].trim_start();
        if !preceded || !rest.starts_with('=') {
            continue;
        }
        let value_start = tag.len() - rest.len() + 1;
        let value = tag[value_start..].trim_start();
        return Some(match value.chars().next() {
            Some(quote @ ('"' | '\'')) => value[1..]
                .split(quote)
                .next()
                .unwrap_or_default()
                .to_string(),
            _ => value
                .split(|c: char| c.is_whitespace() || c == '>')
                .next()
                .unwrap_or_default()
                .to_string(),
        });
    }
    None
}

fn title_element(head: &str) -> Option<String> {
    let lower = head.to_ascii_lowercase();
    let open = lower.find("<title")?;
    let start = open + tag_end(&head[open..])? + 1;
    let end = start + lower[start..].find("</title")?;
    Some(decode(&head[start..end]))
}

fn decode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let Some(semi) = rest.bytes().take(12).position(|b| b == b';') else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..semi];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            _ => entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| entity.strip_prefix('#').and_then(|dec| dec.parse().ok()))
                .and_then(char::from_u32),
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn clip(text: &str, max: usize) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= max {
        return collapsed;
    }
    let mut clipped: String = collapsed.chars().take(max - 1).collect();
    clipped.push('…');
    clipped
}

#[cfg(test)]
mod tests {
    use super::parse;

    fn page() -> reqwest::Url {
        reqwest::Url::parse("https://www.example.com/posts/1").expect("url")
    }

    #[test]
    fn reads_opengraph() {
        let html = r#"<html><head>
            <meta property="og:title" content="Shipping &amp; handling">
            <meta property="og:description" content='A "quoted" summary'>
            <meta property="og:site_name" content="Example">
            <meta property="og:image" content="/img/cover.png">
            <title>Ignored</title></head><body><meta property="og:title" content="body"></body>"#;
        let unfurl = parse(html, &page(), "https://example.com/posts/1").expect("unfurl");
        assert_eq!(unfurl.title, "Shipping & handling");
        assert_eq!(unfurl.description.as_deref(), Some("A \"quoted\" summary"));
        assert_eq!(unfurl.site_name.as_deref(), Some("Example"));
        assert_eq!(
            unfurl.image_url.as_deref(),
            Some("https://www.example.com/img/cover.png")
        );
        assert_eq!(unfurl.url, "https://example.com/posts/1");
    }

    #[test]
    fn falls_back_to_title_and_description() {
        let html = "<HEAD><TITLE>\n  Plain   page &#8212; home </TITLE>\
                    <META NAME=description CONTENT=\"Short one\"></HEAD>";
        let unfurl = parse(html, &page(), "https://www.example.com/").expect("unfurl");
        assert_eq!(unfurl.title, "Plain page — home");
        assert_eq!(unfurl.description.as_deref(), Some("Short one"));
        assert_eq!(unfurl.site_name.as_deref(), Some("example.com"));
        assert_eq!(unfurl.image_url, None);
    }

    #[test]
    fn nothing_for_a_bare_title() {
        let html = "<head><title>Jira</title></head>";
        assert!(parse(html, &page(), "x").is_none());
    }

    #[test]
    fn nothing_without_a_title() {
        assert!(parse("<html><head></head><body>hi</body></html>", &page(), "x").is_none());
    }

    #[test]
    fn ignores_non_http_images_and_metadata_lookalikes() {
        let html = r#"<head><metadata name="og:title"><meta data-name="x" property="og:title" content="Real">
            <meta name="description" content="Something">
            <meta property="og:image" content="javascript:alert(1)"></head>"#;
        let unfurl = parse(html, &page(), "x").expect("unfurl");
        assert_eq!(unfurl.title, "Real");
        assert_eq!(unfurl.image_url, None);
    }

    #[test]
    fn clips_long_titles() {
        let long = "word ".repeat(100);
        let html = format!("<head><title>{long}</title><meta name=description content=x></head>");
        let unfurl = parse(&html, &page(), "x").expect("unfurl");
        assert_eq!(unfurl.title.chars().count(), 200);
        assert!(unfurl.title.ends_with('…'));
    }
}
