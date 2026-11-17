use super::models::ClipKind;

pub const CLIP_JSON_BYTES: usize = 8 * 1024;
pub const POSTER_MAX_BYTES: usize = 5 * 1024 * 1024;
const EBML: [u8; 4] = [0x1A, 0x45, 0xDF, 0xA3];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Container {
    Mp4,
    WebM,
}

impl Container {
    fn subtype(self) -> &'static str {
        match self {
            Container::Mp4 => "mp4",
            Container::WebM => "webm",
        }
    }

    pub fn sniff(head: &[u8]) -> Option<Self> {
        if head.get(4..8) == Some(b"ftyp".as_slice()) {
            return Some(Container::Mp4);
        }
        head.starts_with(&EBML).then_some(Container::WebM)
    }
}

pub fn media_type(declared: &str, kind: ClipKind) -> Option<(String, Container)> {
    let base = declared
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    let (top, subtype) = base.split_once('/')?;
    let wanted = match kind {
        ClipKind::Audio => "audio",
        ClipKind::Video => "video",
    };
    if top != wanted {
        return None;
    }
    let container = [Container::Mp4, Container::WebM]
        .into_iter()
        .find(|c| c.subtype() == subtype)?;
    Some((base, container))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_recording_is_named_by_its_kind_and_container() {
        assert_eq!(
            media_type("video/webm;codecs=vp8,opus", ClipKind::Video),
            Some(("video/webm".into(), Container::WebM))
        );
        assert_eq!(
            media_type(" Audio/MP4 ", ClipKind::Audio),
            Some(("audio/mp4".into(), Container::Mp4))
        );
        assert_eq!(media_type("video/mp4", ClipKind::Audio), None);
        assert_eq!(media_type("video/quicktime", ClipKind::Video), None);
        assert_eq!(
            media_type("application/octet-stream", ClipKind::Video),
            None
        );
    }

    #[test]
    fn the_bytes_say_which_container_they_are() {
        assert_eq!(
            Container::sniff(b"\0\0\0\x20ftypisom\0\0\0\0"),
            Some(Container::Mp4)
        );
        assert_eq!(
            Container::sniff(&[0x1A, 0x45, 0xDF, 0xA3, 0x9F]),
            Some(Container::WebM)
        );
        assert_eq!(Container::sniff(b"%PDF-1.7"), None);
        assert_eq!(Container::sniff(b"ftyp"), None);
    }
}
