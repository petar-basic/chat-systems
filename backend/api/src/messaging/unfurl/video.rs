use chrono::Utc;
use serde_json::Value;

use super::fetch::{TIMEOUT, USER_AGENT};
use crate::messaging::models::{Unfurl, VideoEmbed};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoProvider {
    YouTube,
    Vimeo,
    Loom,
}

impl VideoProvider {
    fn as_str(self) -> &'static str {
        match self {
            VideoProvider::YouTube => "youtube",
            VideoProvider::Vimeo => "vimeo",
            VideoProvider::Loom => "loom",
        }
    }

    fn name(self) -> &'static str {
        match self {
            VideoProvider::YouTube => "YouTube",
            VideoProvider::Vimeo => "Vimeo",
            VideoProvider::Loom => "Loom",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoRef {
    pub provider: VideoProvider,
    pub id: String,
    pub hash: Option<String>,
    pub start: Option<u32>,
}

impl VideoRef {
    fn canonical_url(&self) -> String {
        match (self.provider, &self.hash) {
            (VideoProvider::YouTube, _) => format!("https://www.youtube.com/watch?v={}", self.id),
            (VideoProvider::Vimeo, Some(hash)) => format!("https://vimeo.com/{}/{hash}", self.id),
            (VideoProvider::Vimeo, None) => format!("https://vimeo.com/{}", self.id),
            (VideoProvider::Loom, _) => format!("https://www.loom.com/share/{}", self.id),
        }
    }

    fn oembed_url(&self) -> Option<reqwest::Url> {
        let endpoint = match self.provider {
            VideoProvider::YouTube => "https://www.youtube.com/oembed?format=json",
            VideoProvider::Vimeo => "https://vimeo.com/api/oembed.json",
            VideoProvider::Loom => "https://www.loom.com/v1/oembed",
        };
        let mut url = reqwest::Url::parse(endpoint).ok()?;
        url.query_pairs_mut()
            .append_pair("url", &self.canonical_url());
        Some(url)
    }
}

pub fn match_video(url: &str) -> Option<VideoRef> {
    let parsed = reqwest::Url::parse(url).ok()?;
    if parsed.scheme() != "https" {
        return None;
    }
    let segments: Vec<&str> = parsed.path_segments()?.filter(|s| !s.is_empty()).collect();
    let query = |name: &str| {
        parsed
            .query_pairs()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.into_owned())
    };
    match parsed.host_str()? {
        "www.youtube.com" | "youtube.com" | "m.youtube.com" => {
            let id = match segments.as_slice() {
                ["watch"] => query("v")?,
                ["shorts" | "live" | "embed", id] => id.to_string(),
                _ => return None,
            };
            youtube(id, query("t").or_else(|| query("start")))
        }
        "youtu.be" => match segments.as_slice() {
            [id] => youtube(id.to_string(), query("t")),
            _ => None,
        },
        "vimeo.com" | "www.vimeo.com" => {
            let (id, hash) = match segments.as_slice() {
                [id] => (*id, None),
                [id, hash] if is_vimeo_id(id) => (*id, Some(*hash)),
                ["channels", _, id] => (*id, None),
                ["groups", _, "videos", id] => (*id, None),
                _ => return None,
            };
            vimeo(id, hash)
        }
        "player.vimeo.com" => match segments.as_slice() {
            ["video", id] => vimeo(id, query("h").as_deref()),
            _ => None,
        },
        "www.loom.com" | "loom.com" => match segments.as_slice() {
            ["share" | "embed", slug] => loom_id(slug).map(|id| VideoRef {
                provider: VideoProvider::Loom,
                id,
                hash: None,
                start: None,
            }),
            _ => None,
        },
        _ => None,
    }
}

fn youtube(id: String, start: Option<String>) -> Option<VideoRef> {
    let valid = id.len() == 11
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    valid.then(|| VideoRef {
        provider: VideoProvider::YouTube,
        id,
        hash: None,
        start: start.as_deref().and_then(seconds).filter(|s| *s > 0),
    })
}

fn is_vimeo_id(id: &str) -> bool {
    (1..=12).contains(&id.len()) && id.chars().all(|c| c.is_ascii_digit())
}

fn vimeo(id: &str, hash: Option<&str>) -> Option<VideoRef> {
    if !is_vimeo_id(id) {
        return None;
    }
    let hash = match hash {
        Some(hash)
            if (6..=20).contains(&hash.len()) && hash.chars().all(|c| c.is_ascii_hexdigit()) =>
        {
            Some(hash.to_string())
        }
        Some(_) => return None,
        None => None,
    };
    Some(VideoRef {
        provider: VideoProvider::Vimeo,
        id: id.to_string(),
        hash,
        start: None,
    })
}

fn loom_id(slug: &str) -> Option<String> {
    let id = slug.get(slug.len().checked_sub(32)?..)?;
    let separated = slug.len() == 32 || slug[..slug.len() - 32].ends_with('-');
    (separated && id.chars().all(|c| c.is_ascii_hexdigit())).then(|| id.to_ascii_lowercase())
}

fn seconds(raw: &str) -> Option<u32> {
    if let Ok(plain) = raw.parse::<u32>() {
        return Some(plain);
    }
    let mut total: u32 = 0;
    let mut number = String::new();
    for c in raw.chars() {
        if c.is_ascii_digit() {
            number.push(c);
            continue;
        }
        let unit = match c {
            'h' => 3600,
            'm' => 60,
            's' => 1,
            _ => return None,
        };
        total = total.checked_add(number.parse::<u32>().ok()?.checked_mul(unit)?)?;
        number.clear();
    }
    number.is_empty().then_some(total)
}

pub async fn describe(video: &VideoRef, url: &str) -> Option<Unfurl> {
    let client = reqwest::Client::builder()
        .timeout(TIMEOUT)
        .user_agent(USER_AGENT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .ok()?;
    let response = client.get(video.oembed_url()?).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let body = response.json::<Value>().await.ok()?;
    card(video, &body, url)
}

fn card(video: &VideoRef, body: &Value, url: &str) -> Option<Unfurl> {
    let text = |name: &str| {
        body.get(name)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    Some(Unfurl {
        url: url.to_string(),
        title: text("title")?,
        site_name: Some(video.provider.name().into()),
        image_url: text("thumbnail_url").filter(|u| u.starts_with("https://")),
        kind: Some("video".into()),
        subtitle: text("author_name"),
        embed: Some(VideoEmbed {
            provider: video.provider.as_str().into(),
            id: video.id.clone(),
            hash: video.hash.clone(),
            start: video.start,
        }),
        fetched_at: Some(Utc::now()),
        ..Unfurl::default()
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{card, match_video, VideoProvider, VideoRef};

    fn video(provider: VideoProvider, id: &str) -> VideoRef {
        VideoRef {
            provider,
            id: id.into(),
            hash: None,
            start: None,
        }
    }

    #[test]
    fn matches_youtube_links() {
        for url in [
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://youtube.com/watch?feature=share&v=dQw4w9WgXcQ",
            "https://m.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://youtu.be/dQw4w9WgXcQ",
            "https://www.youtube.com/shorts/dQw4w9WgXcQ",
            "https://www.youtube.com/live/dQw4w9WgXcQ",
            "https://www.youtube.com/embed/dQw4w9WgXcQ",
        ] {
            assert_eq!(
                match_video(url),
                Some(video(VideoProvider::YouTube, "dQw4w9WgXcQ")),
                "{url}"
            );
        }
    }

    #[test]
    fn keeps_a_youtube_start_time() {
        for (url, start) in [
            ("https://youtu.be/dQw4w9WgXcQ?t=90", Some(90)),
            (
                "https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=1m30s",
                Some(90),
            ),
            (
                "https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=1h2m3s",
                Some(3723),
            ),
            (
                "https://www.youtube.com/embed/dQw4w9WgXcQ?start=42",
                Some(42),
            ),
            ("https://youtu.be/dQw4w9WgXcQ?t=0", None),
            ("https://youtu.be/dQw4w9WgXcQ?t=soon", None),
        ] {
            assert_eq!(match_video(url).expect(url).start, start, "{url}");
        }
    }

    #[test]
    fn matches_vimeo_links_with_their_private_hash() {
        assert_eq!(
            match_video("https://vimeo.com/76979871"),
            Some(video(VideoProvider::Vimeo, "76979871"))
        );
        assert_eq!(
            match_video("https://vimeo.com/channels/staffpicks/76979871"),
            Some(video(VideoProvider::Vimeo, "76979871"))
        );
        assert_eq!(
            match_video("https://vimeo.com/groups/motion/videos/76979871"),
            Some(video(VideoProvider::Vimeo, "76979871"))
        );
        for url in [
            "https://vimeo.com/76979871/a1b2c3d4e5",
            "https://player.vimeo.com/video/76979871?h=a1b2c3d4e5",
        ] {
            let found = match_video(url).expect(url);
            assert_eq!(found.id, "76979871");
            assert_eq!(found.hash.as_deref(), Some("a1b2c3d4e5"), "{url}");
        }
    }

    #[test]
    fn matches_loom_links() {
        let id = "0123456789abcdef0123456789abcdef";
        for url in [
            format!("https://www.loom.com/share/{id}"),
            format!("https://loom.com/embed/{id}"),
            format!("https://www.loom.com/share/weekly-demo-{id}?sid=1"),
        ] {
            assert_eq!(
                match_video(&url),
                Some(video(VideoProvider::Loom, id)),
                "{url}"
            );
        }
    }

    #[test]
    fn leaves_everything_else_alone() {
        for url in [
            "http://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://www.youtube.com/watch?v=short",
            "https://www.youtube.com/@channel",
            "https://www.youtube.com/playlist?list=PL123",
            "https://youtube.evil.test/watch?v=dQw4w9WgXcQ",
            "https://vimeo.com/about",
            "https://vimeo.com/76979871/not-a-hash",
            "https://www.loom.com/share/notanid",
            "https://www.loom.com/share/x0123456789abcdef0123456789abcdef",
            "https://www.loom.com/looms/videos",
        ] {
            assert!(match_video(url).is_none(), "{url}");
        }
    }

    #[test]
    fn asks_oembed_about_the_canonical_link() {
        let found = match_video("https://youtu.be/dQw4w9WgXcQ?t=90&si=tracking").expect("match");
        let oembed = found.oembed_url().expect("url").to_string();
        assert!(oembed.starts_with("https://www.youtube.com/oembed?format=json&url="));
        assert!(oembed.ends_with("watch%3Fv%3DdQw4w9WgXcQ"), "{oembed}");
        let private = match_video("https://vimeo.com/76979871/a1b2c3d4e5").expect("match");
        assert!(private.oembed_url().expect("url").to_string().starts_with(
            "https://vimeo.com/api/oembed.json?url=https%3A%2F%2Fvimeo.com%2F76979871%2Fa1b2c3d4e5"
        ));
    }

    #[test]
    fn maps_an_oembed_answer_to_a_video_card() {
        let found = match_video("https://youtu.be/dQw4w9WgXcQ?t=90").expect("match");
        let body = json!({
            "title": "Never Gonna Give You Up",
            "author_name": "Rick Astley",
            "provider_name": "YouTube",
            "thumbnail_url": "https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg",
            "html": "<iframe src=\"https://evil.test\"></iframe>"
        });
        let mapped = card(&found, &body, "https://youtu.be/dQw4w9WgXcQ?t=90").expect("card");
        assert_eq!(mapped.kind.as_deref(), Some("video"));
        assert_eq!(mapped.title, "Never Gonna Give You Up");
        assert_eq!(mapped.subtitle.as_deref(), Some("Rick Astley"));
        assert_eq!(mapped.site_name.as_deref(), Some("YouTube"));
        let embed = mapped.embed.as_ref().expect("embed");
        assert_eq!(
            (embed.provider.as_str(), embed.id.as_str(), embed.start),
            ("youtube", "dQw4w9WgXcQ", Some(90))
        );
        assert!(!serde_json::to_string(&mapped)
            .unwrap_or_default()
            .contains("evil"));

        let insecure = json!({ "title": "Clip", "thumbnail_url": "http://cdn.test/t.jpg" });
        assert_eq!(card(&found, &insecure, "u").expect("card").image_url, None);
        assert!(card(&found, &json!({ "author_name": "x" }), "u").is_none());
    }
}
