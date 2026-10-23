use std::io::{Cursor, Read};

use quick_xml::escape::resolve_predefined_entity;
use quick_xml::events::Event;
use quick_xml::Reader;
use serde::Serialize;
use unicode_normalization::UnicodeNormalization;
use utoipa::ToSchema;

use super::models::{FileKind, FileTextStatus};

pub const MAX_SOURCE_BYTES: i64 = 25 * 1024 * 1024;
pub const MAX_TEXT_BYTES: usize = 1024 * 1024;
const MAX_ARCHIVE_ENTRIES: usize = 10_000;
const MAX_ARCHIVE_BYTES: u64 = 64 * 1024 * 1024;

pub const HIT_START: char = '\u{2}';
pub const HIT_END: char = '\u{3}';

#[derive(Debug, PartialEq, Eq)]
pub enum Extraction {
    Text(String),
    Empty,
    Unsupported,
    TooLarge,
    Failed(String),
}

impl Extraction {
    pub fn status(&self) -> FileTextStatus {
        match self {
            Extraction::Text(_) => FileTextStatus::Done,
            Extraction::Empty => FileTextStatus::Empty,
            Extraction::Unsupported => FileTextStatus::Unsupported,
            Extraction::TooLarge => FileTextStatus::TooLarge,
            Extraction::Failed(_) => FileTextStatus::Failed,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Format {
    Pdf,
    Plain,
    Docx,
    Xlsx,
    Pptx,
}

fn extension(filename: &str) -> String {
    filename
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default()
}

fn format_of(kind: FileKind, mime: &str, filename: &str) -> Option<Format> {
    let ext = extension(filename);
    let mime = mime.to_ascii_lowercase();
    match kind {
        FileKind::Pdf => Some(Format::Pdf),
        FileKind::Text => Some(Format::Plain),
        FileKind::Spreadsheet if ext == "csv" || mime == "text/csv" => Some(Format::Plain),
        FileKind::Spreadsheet
            if ext == "xlsx" || ext == "xlsm" || mime.contains("spreadsheetml") =>
        {
            Some(Format::Xlsx)
        }
        FileKind::Presentation if ext == "pptx" || mime.contains("presentationml") => {
            Some(Format::Pptx)
        }
        FileKind::Document if ext == "docx" || mime.contains("wordprocessingml") => {
            Some(Format::Docx)
        }
        _ => None,
    }
}

pub fn readable(kind: FileKind, mime: &str, filename: &str) -> bool {
    format_of(kind, mime, filename).is_some()
}

pub fn extract(kind: FileKind, mime: &str, filename: &str, bytes: &[u8]) -> Extraction {
    let raw = match format_of(kind, mime, filename) {
        None => return Extraction::Unsupported,
        Some(Format::Pdf) => pdf(bytes),
        Some(Format::Plain) => plain(bytes),
        Some(Format::Docx) => ooxml(bytes, |name| {
            name == "word/document.xml"
                || (name.starts_with("word/")
                    && ["header", "footer", "footnotes", "endnotes"]
                        .iter()
                        .any(|part| name[5..].starts_with(part)))
        }),
        Some(Format::Xlsx) => ooxml(bytes, |name| {
            name == "xl/sharedStrings.xml" || name.starts_with("xl/worksheets/sheet")
        }),
        Some(Format::Pptx) => ooxml(bytes, |name| {
            name.starts_with("ppt/slides/slide") || name.starts_with("ppt/notesSlides/notesSlide")
        }),
    };
    match raw {
        Ok(text) => finish(&text),
        Err(outcome) => outcome,
    }
}

fn finish(raw: &str) -> Extraction {
    let mut text = String::with_capacity(raw.len().min(MAX_TEXT_BYTES));
    let mut pending_space = false;
    for ch in raw.nfkc() {
        if ch.is_whitespace() || ch.is_control() {
            pending_space = !text.is_empty();
            continue;
        }
        if text.len() + ch.len_utf8() + usize::from(pending_space) > MAX_TEXT_BYTES {
            break;
        }
        if pending_space {
            text.push(' ');
            pending_space = false;
        }
        text.push(ch);
    }
    if text.is_empty() {
        Extraction::Empty
    } else {
        Extraction::Text(text)
    }
}

fn pdf(bytes: &[u8]) -> Result<String, Extraction> {
    pdf_extract::extract_text_from_mem(bytes).map_err(|e| match e {
        pdf_extract::OutputError::PdfError(
            pdf_extract::Error::Decryption(_) | pdf_extract::Error::InvalidPassword,
        ) => Extraction::Unsupported,
        other => Extraction::Failed(other.to_string()),
    })
}

fn plain(bytes: &[u8]) -> Result<String, Extraction> {
    if bytes.contains(&0) {
        return Err(Extraction::Unsupported);
    }
    let text = String::from_utf8_lossy(bytes);
    let replaced = text
        .chars()
        .filter(|&c| c == char::REPLACEMENT_CHARACTER)
        .count();
    if replaced * 100 > text.chars().count().max(1) {
        return Err(Extraction::Unsupported);
    }
    Ok(text.into_owned())
}

fn ooxml(bytes: &[u8], wanted: impl Fn(&str) -> bool) -> Result<String, Extraction> {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| Extraction::Failed(e.to_string()))?;
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err(Extraction::TooLarge);
    }
    let mut names: Vec<String> = archive
        .file_names()
        .filter_map(Result::ok)
        .filter(|name| wanted(name))
        .map(|name| name.into_owned())
        .collect();
    names.sort_by_key(|name| part_order(name));

    let mut budget = MAX_ARCHIVE_BYTES;
    let mut text = String::new();
    for name in names {
        let entry = archive
            .by_name(&name)
            .map_err(|e| Extraction::Failed(e.to_string()))?;
        if entry.size() > budget {
            return Err(Extraction::TooLarge);
        }
        let mut xml = Vec::new();
        entry
            .take(budget + 1)
            .read_to_end(&mut xml)
            .map_err(|e| Extraction::Failed(e.to_string()))?;
        if xml.len() as u64 > budget {
            return Err(Extraction::TooLarge);
        }
        budget -= xml.len() as u64;
        xml_text(&String::from_utf8_lossy(&xml), &mut text)?;
        if text.len() > MAX_TEXT_BYTES * 2 {
            break;
        }
    }
    Ok(text)
}

fn part_order(name: &str) -> (String, u32) {
    let digits: String = name.chars().filter(char::is_ascii_digit).collect();
    let stem: String = name.chars().filter(|c| !c.is_ascii_digit()).collect();
    (stem, digits.parse().unwrap_or(0))
}

fn xml_text(xml: &str, out: &mut String) -> Result<(), Extraction> {
    let mut reader = Reader::from_str(xml);
    let mut in_text = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => in_text = e.local_name().into_inner() == "t",
            Ok(Event::End(e)) => {
                in_text = false;
                if matches!(
                    e.local_name().into_inner(),
                    "p" | "si" | "is" | "tc" | "row"
                ) {
                    out.push(' ');
                }
            }
            Ok(Event::Empty(e)) => {
                if matches!(e.local_name().into_inner(), "tab" | "br" | "cr") {
                    out.push(' ');
                }
            }
            Ok(Event::Text(e)) if in_text => out.push_str(&e.xml10_content()),
            Ok(Event::CData(e)) if in_text => out.push_str(&e.into_inner()),
            Ok(Event::GeneralRef(e)) if in_text => {
                if let Ok(Some(ch)) = e.resolve_char_ref() {
                    out.push(ch);
                } else if let Some(entity) = resolve_predefined_entity(&e.into_inner()) {
                    out.push_str(entity);
                }
            }
            Ok(Event::Eof) => return Ok(()),
            Ok(_) => {}
            Err(e) => return Err(Extraction::Failed(e.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct SnippetPart {
    pub text: String,
    pub hit: bool,
}

pub fn snippet_parts(headline: &str) -> Vec<SnippetPart> {
    let mut parts: Vec<SnippetPart> = Vec::new();
    let mut hit = false;
    let mut current = String::new();
    for ch in headline.chars() {
        let flip = match ch {
            HIT_START => Some(true),
            HIT_END => Some(false),
            _ => None,
        };
        match flip {
            Some(next) => {
                if !current.is_empty() {
                    parts.push(SnippetPart {
                        text: std::mem::take(&mut current),
                        hit,
                    });
                }
                hit = next;
            }
            None => current.push(ch),
        }
    }
    if !current.is_empty() {
        parts.push(SnippetPart { text: current, hit });
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(format!(
            "{}/fixtures/file_text/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .expect("fixture")
    }

    fn text_of(extraction: Extraction) -> String {
        match extraction {
            Extraction::Text(text) => text,
            other => panic!("expected text, got {other:?}"),
        }
    }

    #[test]
    fn a_pdf_keeps_its_words_and_diacritics() {
        let text = text_of(extract(
            FileKind::Pdf,
            "application/pdf",
            "contract.pdf",
            &fixture("contract.pdf"),
        ));
        assert!(text.contains("Šćepanović"), "{text}");
        assert!(text.contains("Ugovor o saradnji"), "{text}");
        assert!(!text.contains("  "), "whitespace is collapsed");
    }

    #[test]
    fn a_pdf_with_no_text_is_empty() {
        assert_eq!(
            extract(
                FileKind::Pdf,
                "application/pdf",
                "blank.pdf",
                &fixture("blank.pdf")
            ),
            Extraction::Empty
        );
    }

    #[test]
    fn a_password_protected_pdf_is_unsupported() {
        assert_eq!(
            extract(
                FileKind::Pdf,
                "application/pdf",
                "encrypted.pdf",
                &fixture("encrypted.pdf")
            ),
            Extraction::Unsupported
        );
    }

    #[test]
    fn a_malformed_pdf_fails_without_panicking() {
        for bytes in [
            fixture("truncated.pdf"),
            b"%PDF-1.7\ngarbage".to_vec(),
            Vec::new(),
        ] {
            assert!(matches!(
                extract(FileKind::Pdf, "application/pdf", "x.pdf", &bytes),
                Extraction::Failed(_)
            ));
        }
    }

    #[test]
    fn a_docx_reads_its_paragraphs() {
        let text = text_of(extract(
            FileKind::Document,
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "contract.docx",
            &fixture("contract.docx"),
        ));
        assert!(text.contains("Ugovor o saradnji"), "{text}");
        assert!(text.contains("Đorđe Šćepanović"), "{text}");
        assert!(text.contains("Žarko Ćirić"), "{text}");
        assert!(
            text.contains("Poverljivo & interno Žig"),
            "the header too: {text}"
        );
    }

    #[test]
    fn an_xlsx_joins_runs_and_keeps_cells_apart() {
        let text = text_of(extract(
            FileKind::Spreadsheet,
            "application/octet-stream",
            "budget.xlsx",
            &fixture("budget.xlsx"),
        ));
        for word in ["Marketing", "Konferencija", "Putni troškovi", "Beograd"] {
            assert!(text.contains(word), "{word} in {text}");
        }
        assert!(!text.contains("12500"), "numbers are not text: {text}");
    }

    #[test]
    fn a_pptx_reads_slides_in_order_and_the_notes() {
        let text = text_of(extract(
            FileKind::Presentation,
            "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            "roadmap.pptx",
            &fixture("roadmap.pptx"),
        ));
        let first = text.find("Plan za 2027").expect("slide 1");
        let second = text.find("Huddles sa SFU").expect("slide 2");
        assert!(first < second, "{text}");
        assert!(text.contains("Kragujevac"), "{text}");
    }

    #[test]
    fn a_zip_bomb_is_too_large() {
        assert_eq!(
            extract(
                FileKind::Spreadsheet,
                "application/octet-stream",
                "bomb.xlsx",
                &fixture("bomb.xlsx")
            ),
            Extraction::TooLarge
        );
    }

    #[test]
    fn a_docx_that_is_not_a_zip_fails() {
        assert!(matches!(
            extract(
                FileKind::Document,
                "application/octet-stream",
                "fake.docx",
                b"plain words"
            ),
            Extraction::Failed(_)
        ));
    }

    #[test]
    fn plain_text_csv_and_code_are_read_as_they_are() {
        assert_eq!(
            extract(
                FileKind::Text,
                "text/markdown",
                "notes.md",
                "# Sastanak\n\n- budžet\t2027".as_bytes()
            ),
            Extraction::Text("# Sastanak - budžet 2027".into())
        );
        assert_eq!(
            extract(
                FileKind::Spreadsheet,
                "text/csv",
                "q3.csv",
                b"name,amount\nkonferencija,12500"
            ),
            Extraction::Text("name,amount konferencija,12500".into())
        );
        assert_eq!(
            extract(
                FileKind::Text,
                "application/octet-stream",
                "main.rs",
                b"fn ligature() { \"\xef\xac\x81le\" }"
            ),
            Extraction::Text("fn ligature() { \"file\" }".into())
        );
    }

    #[test]
    fn binary_dressed_as_text_and_unknown_formats_are_unsupported() {
        assert_eq!(
            extract(FileKind::Text, "text/plain", "x.txt", b"MZ\0\0\x90\0"),
            Extraction::Unsupported
        );
        assert_eq!(
            extract(
                FileKind::Spreadsheet,
                "application/vnd.ms-excel",
                "old.xls",
                b"\xd0\xcf\x11\xe0"
            ),
            Extraction::Unsupported
        );
        assert_eq!(
            extract(FileKind::Image, "image/png", "a.png", b"x"),
            Extraction::Unsupported
        );
        assert_eq!(
            extract(FileKind::Text, "text/plain", "blank.txt", b" \n\t "),
            Extraction::Empty
        );
    }

    #[test]
    fn text_stops_at_the_cap_on_a_character_boundary() {
        let long = "ž".repeat(MAX_TEXT_BYTES);
        let text = text_of(extract(
            FileKind::Text,
            "text/plain",
            "long.txt",
            long.as_bytes(),
        ));
        assert!(text.len() <= MAX_TEXT_BYTES);
        assert!(text.len() > MAX_TEXT_BYTES - 4);
    }

    #[test]
    fn the_markers_never_survive_extraction() {
        let text = text_of(extract(
            FileKind::Text,
            "text/plain",
            "x.txt",
            "a\u{2}b\u{3}c".as_bytes(),
        ));
        assert_eq!(text, "a b c");
    }

    #[test]
    fn a_headline_becomes_parts() {
        assert_eq!(
            snippet_parts("Klijent \u{2}Šćepanović\u{3}, iz \u{2}Čačka\u{3}"),
            vec![
                SnippetPart {
                    text: "Klijent ".into(),
                    hit: false
                },
                SnippetPart {
                    text: "Šćepanović".into(),
                    hit: true
                },
                SnippetPart {
                    text: ", iz ".into(),
                    hit: false
                },
                SnippetPart {
                    text: "Čačka".into(),
                    hit: true
                },
            ]
        );
    }
}
