use std::collections::BTreeMap;

use serde::Serialize;
use sha2::{Digest, Sha256};

/// One file per entity type, one JSON object per line, plus a manifest.
///
/// JSONL because an export is processed a row at a time by whoever receives it;
/// a single JSON document would have to be loaded whole. The manifest carries
/// per-file row counts and SHA-256 digests — that is what makes an export
/// defensible rather than merely present, because the recipient can check that
/// what they were given is what was produced.
#[derive(Default)]
pub struct ExportArchive {
    files: BTreeMap<String, Vec<u8>>,
    counts: BTreeMap<String, usize>,
}

#[derive(Debug, Serialize)]
pub struct ManifestFile {
    pub rows: usize,
    pub bytes: usize,
    pub sha256: String,
}

impl ExportArchive {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn append<T: Serialize>(&mut self, file: &str, row: &T) {
        let Ok(mut line) = serde_json::to_vec(row) else {
            return;
        };
        line.push(b'\n');
        self.files.entry(file.to_string()).or_default().extend(line);
        *self.counts.entry(file.to_string()).or_default() += 1;
    }

    /// Records a file that exists in the archive even when it has no rows —
    /// "conversations.jsonl: 0" is a different statement from its absence, and
    /// the difference matters when the question is whether DMs were included.
    pub fn declare(&mut self, file: &str) {
        self.files.entry(file.to_string()).or_default();
        self.counts.entry(file.to_string()).or_default();
    }

    pub fn put_blob(&mut self, path: &str, bytes: Vec<u8>) {
        self.files.insert(path.to_string(), bytes);
    }

    pub fn rows(&self, file: &str) -> usize {
        self.counts.get(file).copied().unwrap_or(0)
    }

    pub fn manifest_files(&self) -> BTreeMap<String, ManifestFile> {
        self.files
            .iter()
            .map(|(name, bytes)| {
                let mut hasher = Sha256::new();
                hasher.update(bytes);
                (
                    name.clone(),
                    ManifestFile {
                        rows: self.counts.get(name).copied().unwrap_or(0),
                        bytes: bytes.len(),
                        sha256: to_hex(&hasher.finalize()),
                    },
                )
            })
            .collect()
    }

    /// A tar archive, written by hand: the alternative is a zip crate for a
    /// format with no compression requirement and a well-understood header.
    pub fn into_tar(self, manifest: &serde_json::Value) -> Vec<u8> {
        let mut out = Vec::new();
        let manifest_bytes = serde_json::to_vec_pretty(manifest).unwrap_or_default();

        for (name, bytes) in self.files.into_iter().chain(std::iter::once((
            "manifest.json".to_string(),
            manifest_bytes,
        ))) {
            write_tar_entry(&mut out, &name, &bytes);
        }

        // Two zero blocks end the archive.
        out.extend(std::iter::repeat_n(0u8, 1024));
        out
    }
}

fn write_tar_entry(out: &mut Vec<u8>, name: &str, body: &[u8]) {
    let mut header = [0u8; 512];

    let name_bytes = name.as_bytes();
    let len = name_bytes.len().min(100);
    header[..len].copy_from_slice(&name_bytes[..len]);

    write_octal(&mut header[100..108], 0o644, 7); // mode
    write_octal(&mut header[108..116], 0, 7); // uid
    write_octal(&mut header[116..124], 0, 7); // gid
    write_octal(&mut header[124..136], body.len() as u64, 11); // size
    write_octal(&mut header[136..148], 0, 11); // mtime
    header[156] = b'0'; // regular file
    header[257..263].copy_from_slice(b"ustar\0");
    header[263..265].copy_from_slice(b"00");

    // The checksum is computed with its own field read as spaces.
    header[148..156].fill(b' ');
    let checksum: u32 = header.iter().map(|b| u32::from(*b)).sum();
    write_octal(&mut header[148..154], u64::from(checksum), 5);
    header[154] = 0;
    header[155] = b' ';

    out.extend_from_slice(&header);
    out.extend_from_slice(body);
    let padding = (512 - (body.len() % 512)) % 512;
    out.extend(std::iter::repeat_n(0u8, padding));
}

fn write_octal(field: &mut [u8], value: u64, digits: usize) {
    let text = format!("{value:0digits$o}");
    let bytes = text.as_bytes();
    let len = bytes.len().min(digits);
    field[..len].copy_from_slice(&bytes[..len]);
    if len < field.len() {
        field[len] = 0;
    }
}

/// digest 0.11 returns a plain byte array, which has no hex formatting of its own.
fn to_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut out, byte| {
            use std::fmt::Write;
            let _ = write!(out, "{byte:02x}");
            out
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize)]
    struct Row {
        id: u32,
    }

    #[test]
    fn counts_and_digests_describe_what_was_written() {
        let mut archive = ExportArchive::new();
        archive.append("messages.jsonl", &Row { id: 1 });
        archive.append("messages.jsonl", &Row { id: 2 });
        archive.declare("conversations.jsonl");

        let files = archive.manifest_files();
        assert_eq!(files["messages.jsonl"].rows, 2);
        assert_eq!(
            files["conversations.jsonl"].rows, 0,
            "an empty file is a statement, not an absence"
        );
        assert_eq!(files["messages.jsonl"].sha256.len(), 64);
    }

    #[test]
    fn each_row_is_its_own_line() {
        let mut archive = ExportArchive::new();
        archive.append("messages.jsonl", &Row { id: 1 });
        archive.append("messages.jsonl", &Row { id: 2 });
        let tar = archive.into_tar(&serde_json::json!({}));
        let text = String::from_utf8_lossy(&tar);
        assert!(text.contains("{\"id\":1}\n{\"id\":2}\n"));
    }

    #[test]
    fn the_archive_is_a_readable_tar() {
        let mut archive = ExportArchive::new();
        archive.append("messages.jsonl", &Row { id: 7 });
        let tar = archive.into_tar(&serde_json::json!({ "scope": "workspace" }));

        // Header, then the body, then the manifest — and the trailing blocks.
        assert!(
            tar.len().is_multiple_of(512),
            "tar is written in 512-byte blocks"
        );
        let text = String::from_utf8_lossy(&tar);
        assert!(text.contains("messages.jsonl"));
        assert!(text.contains("manifest.json"));
        assert!(text.contains("\"scope\": \"workspace\""));
    }
}
