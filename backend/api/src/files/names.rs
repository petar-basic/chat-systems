use unicode_normalization::UnicodeNormalization;

const DISPLAY_MAX_CHARS: usize = 200;
const STORAGE_MAX_CHARS: usize = 100;
const FALLBACK: &str = "file";

pub fn display_name(raw: &str) -> String {
    let basename = raw.rsplit(['/', '\\']).next().unwrap_or("");
    let cleaned: String = basename
        .nfc()
        .filter(|c| !c.is_control())
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .collect();
    let trimmed = cleaned.trim().trim_start_matches('.').trim_start();
    if trimmed.is_empty() {
        return FALLBACK.to_string();
    }
    shorten(trimmed, DISPLAY_MAX_CHARS)
}

pub fn storage_name(display: &str) -> String {
    let (stem, extension) = match display.rsplit_once('.') {
        Some((stem, ext))
            if !ext.is_empty()
                && ext.len() <= 10
                && ext.chars().all(|c| c.is_ascii_alphanumeric()) =>
        {
            (stem, Some(ext))
        }
        _ => (display, None),
    };
    let slug = ascii_slug(stem);
    let stem = if slug.is_empty() {
        FALLBACK.to_string()
    } else {
        slug
    };
    match extension {
        Some(ext) => format!(
            "{}.{ext}",
            stem.chars()
                .take(STORAGE_MAX_CHARS - ext.len() - 1)
                .collect::<String>()
        ),
        None => stem.chars().take(STORAGE_MAX_CHARS).collect(),
    }
}

fn ascii_slug(value: &str) -> String {
    let mut slug = String::with_capacity(value.len());
    for c in value.chars() {
        let mapped = if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
            c
        } else if c == '.' && !slug.ends_with('.') {
            '.'
        } else if !slug.ends_with('_') {
            '_'
        } else {
            continue;
        };
        slug.push(mapped);
    }
    slug.trim_matches(['.', '_']).to_string()
}

pub fn content_disposition(display: &str) -> String {
    let fallback: String = display
        .chars()
        .map(|c| match c {
            '"' | '\\' => '_',
            c if c.is_ascii() && !c.is_ascii_control() => c,
            _ => '_',
        })
        .collect();
    format!(
        "attachment; filename=\"{fallback}\"; filename*=UTF-8''{}",
        percent_encode(display)
    )
}

fn shorten(name: &str, max_chars: usize) -> String {
    if name.chars().count() <= max_chars {
        return name.to_string();
    }
    let extension = name
        .rsplit_once('.')
        .map(|(_, ext)| ext)
        .filter(|ext| !ext.is_empty() && ext.chars().count() <= 10);
    match extension {
        Some(ext) => {
            let keep = max_chars - ext.chars().count() - 1;
            let stem: String = name.chars().take(keep).collect();
            format!("{}.{ext}", stem.trim_end())
        }
        None => name.chars().take(max_chars).collect(),
    }
}

fn percent_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len() * 3);
    for byte in value.bytes() {
        let attr_char = byte.is_ascii_alphanumeric()
            || matches!(
                byte,
                b'!' | b'#' | b'$' | b'&' | b'+' | b'-' | b'.' | b'^' | b'_' | b'`' | b'|' | b'~'
            );
        if attr_char {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{content_disposition, display_name, storage_name};

    #[test]
    fn keeps_the_name_people_wrote() {
        assert_eq!(display_name("Izveštaj o radu.pdf"), "Izveštaj o radu.pdf");
        assert_eq!(
            display_name("Ćevapi & pivo (final).xlsx"),
            "Ćevapi & pivo (final).xlsx"
        );
        assert_eq!(display_name("会议记录.docx"), "会议记录.docx");
        assert_eq!(display_name("report..final.pdf"), "report..final.pdf");
    }

    #[test]
    fn composes_decomposed_letters() {
        let decomposed = "Izves\u{030C}taj.pdf";
        assert_eq!(display_name(decomposed), "Izveštaj.pdf");
    }

    #[test]
    fn drops_paths_controls_and_leading_dots() {
        assert_eq!(display_name("../../etc/passwd"), "passwd");
        assert_eq!(display_name("C:\\Users\\ana\\slika.png"), "slika.png");
        assert_eq!(display_name("bad\r\nname\u{0}.txt"), "badname.txt");
        assert_eq!(display_name("  .hidden.png "), "hidden.png");
        assert_eq!(display_name("tab\there.txt"), "tab here.txt");
        assert_eq!(display_name("..."), "file");
        assert_eq!(display_name(""), "file");
    }

    #[test]
    fn a_long_name_keeps_its_extension() {
        let long = format!("{}.pdf", "a".repeat(400));
        let shortened = display_name(&long);
        assert_eq!(shortened.chars().count(), 200);
        assert!(shortened.ends_with(".pdf"));
    }

    #[test]
    fn the_storage_key_is_ascii_and_one_segment() {
        assert_eq!(storage_name("Izveštaj o radu.pdf"), "Izve_taj_o_radu.pdf");
        assert_eq!(storage_name("report..final.pdf"), "report.final.pdf");
        assert_eq!(storage_name("会议记录.docx"), "file.docx");
        assert_eq!(storage_name("!!!"), "file");
        assert_eq!(storage_name("a/b\\c.png"), "a_b_c.png");
    }

    #[test]
    fn disposition_carries_both_forms() {
        assert_eq!(
            content_disposition("Izveštaj o radu.pdf"),
            "attachment; filename=\"Izve_taj o radu.pdf\"; filename*=UTF-8''Izve%C5%A1taj%20o%20radu.pdf"
        );
        assert_eq!(
            content_disposition("say \"hi\".txt"),
            "attachment; filename=\"say _hi_.txt\"; filename*=UTF-8''say%20%22hi%22.txt"
        );
    }
}
