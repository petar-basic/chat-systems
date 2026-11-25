use serde::{Deserialize, Serialize};
use tauri::Theme;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Colours {
    pub mode: Theme,
    pub rail: String,
    pub line: String,
    pub fg: String,
    pub muted: String,
    pub badge: String,
    pub badge_fg: String,
}

pub fn is_hex(colour: &str) -> bool {
    colour.len() == 7
        && colour.starts_with('#')
        && colour[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn accept(colours: Colours) -> Option<Colours> {
    let all = [
        &colours.rail,
        &colours.line,
        &colours.fg,
        &colours.muted,
        &colours.badge,
        &colours.badge_fg,
    ];
    all.iter().all(|colour| is_hex(colour)).then(|| Colours {
        rail: colours.rail.to_ascii_lowercase(),
        line: colours.line.to_ascii_lowercase(),
        fg: colours.fg.to_ascii_lowercase(),
        muted: colours.muted.to_ascii_lowercase(),
        badge: colours.badge.to_ascii_lowercase(),
        badge_fg: colours.badge_fg.to_ascii_lowercase(),
        mode: colours.mode,
    })
}

pub fn rgb(colour: &str) -> Option<(u8, u8, u8)> {
    if !is_hex(colour) {
        return None;
    }
    let channel = |at: usize| u8::from_str_radix(&colour[at..at + 2], 16).ok();
    Some((channel(1)?, channel(3)?, channel(5)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn colours(rail: &str) -> Colours {
        Colours {
            mode: Theme::Dark,
            rail: rail.to_owned(),
            line: "#5e355f".to_owned(),
            fg: "#FFFFFF".to_owned(),
            muted: "#c9bcca".to_owned(),
            badge: "#cd2553".to_owned(),
            badge_fg: "#ffffff".to_owned(),
        }
    }

    #[test]
    fn takes_six_digit_hex_colours_and_lowercases_them() {
        let accepted = accept(colours("#2C0A2D")).expect("hex colours");
        assert_eq!(accepted.rail, "#2c0a2d");
        assert_eq!(accepted.fg, "#ffffff");
        assert_eq!(accepted.mode, Theme::Dark);
    }

    #[test]
    fn refuses_anything_but_hex_colours() {
        for rail in [
            "",
            "red",
            "#2c0a2",
            "#2c0a2d0",
            "2c0a2d0",
            "#2c0a2g",
            "rgb(1,2,3)",
            "url(x)",
            "#2c0a2d;background:url(x)",
            "#ééé",
        ] {
            assert_eq!(accept(colours(rail)), None, "{rail}");
        }
    }

    #[test]
    fn reads_the_channels_of_a_colour() {
        assert_eq!(rgb("#2c0a2d"), Some((0x2c, 0x0a, 0x2d)));
        assert_eq!(rgb("#FFFFFF"), Some((255, 255, 255)));
        assert_eq!(rgb("red"), None);
    }
}
