use reqwest::Url;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use shared_common::errors::{AppError, AppResult};

pub const GIFS: &str = "gifs";
pub const GIF_RATING: &str = "gif_rating";
const KEY_MAX: usize = 128;
const URL_MAX: usize = 2048;
const TITLE_MAX: usize = 200;
const SIDE_MAX: i32 = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum GifProvider {
    Giphy,
}

impl GifProvider {
    fn serves(self, url: &Url) -> bool {
        match self {
            Self::Giphy => url.host_str().is_some_and(|host| {
                host == "i.giphy.com"
                    || host
                        .strip_suffix(".giphy.com")
                        .and_then(|sub| sub.strip_prefix("media"))
                        .is_some_and(|n| n.chars().all(|c| c.is_ascii_digit()))
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum GifRating {
    #[serde(rename = "g")]
    G,
    #[serde(rename = "pg")]
    Pg,
    #[serde(rename = "pg-13")]
    Pg13,
    #[serde(rename = "r")]
    R,
}

impl GifRating {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::G => "g",
            Self::Pg => "pg",
            Self::Pg13 => "pg-13",
            Self::R => "r",
        }
    }

    pub fn of(settings: &serde_json::Value) -> Self {
        settings
            .get(GIF_RATING)
            .cloned()
            .and_then(|rating| serde_json::from_value(rating).ok())
            .unwrap_or(Self::Pg)
    }
}

pub fn gifs_on(settings: &serde_json::Value) -> bool {
    settings.get(GIFS).and_then(|v| v.as_bool()) != Some(false)
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GifSettings {
    pub provider: GifProvider,
    pub api_key: String,
    pub rating: GifRating,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Gif {
    pub provider: GifProvider,
    pub id: String,
    pub title: String,
    pub url: String,
    pub still: String,
    pub width: i32,
    pub height: i32,
}

impl Gif {
    pub fn checked(self) -> AppResult<Self> {
        let refuse = |why: &str| Err(AppError::Validation(why.into()));
        if self.id.is_empty()
            || self.id.len() > 64
            || !self.id.chars().all(|c| c.is_ascii_alphanumeric())
        {
            return refuse("A GIF id is up to 64 letters and digits");
        }
        if self.title.chars().count() > TITLE_MAX {
            return refuse("A GIF title is at most 200 characters");
        }
        if !(1..=SIDE_MAX).contains(&self.width) || !(1..=SIDE_MAX).contains(&self.height) {
            return refuse("A GIF is between 1 and 4096 pixels on each side");
        }
        for url in [&self.url, &self.still] {
            if !self.media_url(url) {
                return refuse("A GIF comes from its provider's media host");
            }
        }
        Ok(Self {
            title: self.title.trim().to_string(),
            ..self
        })
    }

    fn media_url(&self, raw: &str) -> bool {
        raw.len() <= URL_MAX
            && Url::parse(raw).is_ok_and(|url| {
                url.scheme() == "https"
                    && url.port().is_none()
                    && url.username().is_empty()
                    && url.password().is_none()
                    && self.provider.serves(&url)
            })
    }
}

pub fn giphy_api_key(raw: &str) -> AppResult<Option<&str>> {
    let key = raw.trim();
    if key.is_empty() {
        return Ok(None);
    }
    if key.len() > KEY_MAX || !key.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(AppError::Validation(
            "A GIPHY API key is up to 128 letters and digits".into(),
        ));
    }
    Ok(Some(key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn gif(url: &str) -> Gif {
        Gif {
            provider: GifProvider::Giphy,
            id: "3o7TKSjRrfIPjeiVyM".into(),
            title: " Cat typing ".into(),
            url: url.into(),
            still: "https://media.giphy.com/media/3o7TKSjRrfIPjeiVyM/giphy_s.gif".into(),
            width: 480,
            height: 270,
        }
    }

    #[test]
    fn a_gif_comes_from_giphys_media_hosts_only() {
        for good in [
            "https://media.giphy.com/media/3o7TKSjRrfIPjeiVyM/giphy.mp4",
            "https://media4.giphy.com/media/3o7TKSjRrfIPjeiVyM/giphy.mp4?cid=abc&rid=giphy.mp4&ct=g",
            "https://i.giphy.com/3o7TKSjRrfIPjeiVyM.webp",
        ] {
            assert!(gif(good).checked().is_ok(), "{good}");
        }
        for bad in [
            "http://media.giphy.com/media/x/giphy.mp4",
            "https://media.giphy.com.evil.example/x.mp4",
            "https://evil.example/media.giphy.com/x.mp4",
            "https://giphy.com/gifs/x",
            "https://mediax.giphy.com/media/x/giphy.mp4",
            "https://media.giphy.com:8443/media/x/giphy.mp4",
            "https://user@media.giphy.com/media/x/giphy.mp4",
            "javascript:alert(1)",
        ] {
            assert!(gif(bad).checked().is_err(), "{bad}");
        }
    }

    #[test]
    fn a_gif_is_trimmed_and_bounded() {
        assert_eq!(
            gif("https://media.giphy.com/media/x/giphy.mp4")
                .checked()
                .expect("valid")
                .title,
            "Cat typing"
        );
        let mut wide = gif("https://media.giphy.com/media/x/giphy.mp4");
        wide.width = 5000;
        assert!(wide.checked().is_err());
        let mut odd = gif("https://media.giphy.com/media/x/giphy.mp4");
        odd.id = "../x".into();
        assert!(odd.checked().is_err());
    }

    #[test]
    fn a_workspace_has_gifs_at_pg_until_it_says_otherwise() {
        assert!(gifs_on(&json!({})));
        assert!(!gifs_on(&json!({ "gifs": false })));
        assert_eq!(GifRating::of(&json!({})), GifRating::Pg);
        assert_eq!(
            GifRating::of(&json!({ "gif_rating": "pg-13" })),
            GifRating::Pg13
        );
        assert_eq!(GifRating::of(&json!({ "gif_rating": "x" })), GifRating::Pg);
    }

    #[test]
    fn a_blank_key_clears_and_a_strange_one_is_refused() {
        assert_eq!(giphy_api_key("  ").expect("blank"), None);
        assert_eq!(giphy_api_key(" abc123 ").expect("key"), Some("abc123"));
        assert!(giphy_api_key("abc 123").is_err());
        assert!(giphy_api_key(&"a".repeat(129)).is_err());
    }
}
