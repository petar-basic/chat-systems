use std::collections::HashMap;
use std::sync::LazyLock;

static TABLE: LazyLock<HashMap<&'static str, Vec<&'static str>>> = LazyLock::new(|| {
    include_str!("emoji_shortcodes.tsv")
        .lines()
        .filter_map(|line| {
            let mut fields = line.split('\t');
            let name = fields.next()?;
            let skins: Vec<&'static str> = fields.collect();
            (!skins.is_empty()).then_some((name, skins))
        })
        .collect()
});

pub fn standard(name: &str) -> Option<&'static str> {
    let (base, tone) = match name.split_once("::") {
        Some((base, tone)) => (base, Some(tone)),
        None => (name, None),
    };
    let skins = TABLE.get(base)?;
    let index = tone
        .and_then(|tone| tone.strip_prefix("skin-tone-"))
        .and_then(|n| n.parse::<usize>().ok())
        .and_then(|n| n.checked_sub(1))
        .filter(|&index| index < skins.len())
        .unwrap_or(0);
    Some(skins[index])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slack_names_resolve_to_the_character_with_their_skin_tone() {
        assert_eq!(standard("tada"), Some("🎉"));
        assert_eq!(standard("thumbsup"), Some("👍"));
        assert_eq!(standard("+1::skin-tone-3"), Some("👍🏼"));
        assert_eq!(standard("+1::skin-tone-9"), Some("👍"));
        assert_eq!(standard("white_check_mark"), Some("✅"));
        assert_eq!(standard("face_with_rolling_eyes"), Some("🙄"));
        assert_eq!(standard("shipit"), None);
    }
}
