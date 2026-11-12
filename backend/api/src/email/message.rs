use askama::Template;

use shared_common::errors::{AppError, AppResult};

use crate::config::AppConfig;
use crate::notifications::text::Words;

pub struct Button {
    pub label: String,
    pub url: String,
}

#[derive(Default)]
pub struct Email {
    pub subject: String,
    pub heading: String,
    pub paragraphs: Vec<String>,
    pub code: Option<String>,
    pub button: Option<Button>,
    pub notes: Vec<String>,
    pub reason: Option<String>,
    pub unsubscribe: Option<String>,
}

pub struct Rendered {
    pub subject: String,
    pub text: String,
    pub html: String,
    pub list_unsubscribe: Option<String>,
}

#[derive(Template)]
#[template(path = "email/layout.html")]
struct Layout<'a> {
    lang: &'a str,
    instance_name: &'a str,
    icon_url: Option<String>,
    preheader: &'a str,
    email: &'a Email,
    fallback: String,
    sent_to: String,
    unsubscribe_label: String,
}

impl Email {
    pub fn render(self, to: &str, config: &AppConfig, words: &Words) -> AppResult<Rendered> {
        let instance = config.instance_name.as_str();
        let sent_to = words.get(
            "email.common.sentTo",
            &[("email", to), ("instance", instance)],
        );
        let fallback = words.get("email.common.fallback", &[]);
        let unsubscribe_label = words.get("email.common.unsubscribe", &[]);
        let text = self.text(&sent_to, &unsubscribe_label);
        let html = Layout {
            lang: words.lang(),
            instance_name: instance,
            icon_url: icon_url(config),
            preheader: self.preheader(),
            email: &self,
            fallback,
            sent_to,
            unsubscribe_label,
        }
        .render()
        .map_err(|e| AppError::Internal(format!("Failed to render an email: {e}")))?;
        Ok(Rendered {
            subject: self.subject,
            text,
            html,
            list_unsubscribe: self.unsubscribe,
        })
    }

    fn preheader(&self) -> &str {
        self.paragraphs
            .first()
            .or(self.notes.first())
            .map_or("", String::as_str)
    }

    fn text(&self, sent_to: &str, unsubscribe_label: &str) -> String {
        let mut blocks = vec![self.heading.clone()];
        blocks.extend(self.paragraphs.iter().cloned());
        if let Some(code) = &self.code {
            blocks.push(code.clone());
        }
        if let Some(button) = &self.button {
            blocks.push(format!("{}:\n{}", button.label, button.url));
        }
        blocks.extend(self.notes.iter().cloned());
        let mut footer = vec![sent_to.to_string()];
        footer.extend(self.reason.iter().cloned());
        if let Some(url) = &self.unsubscribe {
            footer.push(format!("{unsubscribe_label}: {url}"));
        }
        blocks.push(format!("--\n{}", footer.join("\n")));
        let mut text = blocks.join("\n\n");
        text.push('\n');
        text
    }
}

fn icon_url(config: &AppConfig) -> Option<String> {
    match config.instance_icon_url.as_deref() {
        Some(url) if url.starts_with("https://") => Some(url.to_string()),
        _ => config.public_url.starts_with("https://").then(|| {
            format!(
                "{}/icons/icon-192.png",
                config.public_url.trim_end_matches('/')
            )
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notifications::text;
    use crate::profile::models::Language;

    fn config(public_url: &str, icon: Option<&str>) -> AppConfig {
        AppConfig {
            public_url: public_url.into(),
            instance_name: "Acme <b>".into(),
            instance_icon_url: icon.map(String::from),
            ..AppConfig::for_tests()
        }
    }

    fn email() -> Email {
        Email {
            subject: "Join Ops".into(),
            heading: "Join Ops & <script>alert(1)</script>".into(),
            paragraphs: vec!["You were invited.".into()],
            button: Some(Button {
                label: "Join".into(),
                url: "https://chat.example/invite/abc?x=1&y=2".into(),
            }),
            notes: vec!["It expires in 7 days.".into()],
            ..Email::default()
        }
    }

    #[test]
    fn the_html_escapes_what_it_interpolates() {
        let words = text::words(Language::En);
        let out = email()
            .render("a@b.test", &config("https://chat.example", None), &words)
            .unwrap();
        assert!(!out.html.contains("<script>"));
        assert!(
            out.html.contains("Ops &#38; &#60;script&#62;")
                || out.html.contains("Ops &amp; &lt;script&gt;")
        );
        assert!(out.html.contains("x=1&#38;y=2") || out.html.contains("x=1&amp;y=2"));
        assert!(!out.html.contains("Acme <b>"));
    }

    #[test]
    fn the_text_part_has_the_link_on_a_line_of_its_own() {
        let words = text::words(Language::En);
        let out = email()
            .render("a@b.test", &config("http://localhost", None), &words)
            .unwrap();
        assert!(
            out.text
                .lines()
                .any(|line| line == "https://chat.example/invite/abc?x=1&y=2"),
            "{}",
            out.text
        );
        assert!(out.text.contains("a@b.test"), "{}", out.text);
    }

    #[test]
    fn the_icon_is_the_instance_one_over_https_else_the_app_one_over_https() {
        assert_eq!(
            icon_url(&config(
                "https://chat.example",
                Some("https://cdn.example/i.png")
            ))
            .as_deref(),
            Some("https://cdn.example/i.png")
        );
        assert_eq!(
            icon_url(&config(
                "https://chat.example/",
                Some("http://cdn.example/i.png")
            ))
            .as_deref(),
            Some("https://chat.example/icons/icon-192.png")
        );
        assert_eq!(icon_url(&config("http://localhost:8080", None)), None);
    }
}
