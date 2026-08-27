use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use shared_common::errors::{AppError, AppResult};

/// Where the export is read from. A ZIP is what Slack hands you; a directory is
/// what you have five minutes later, and on a 20 GB export that difference
/// matters enough to support both.
pub trait ExportSource: Send {
    fn label(&self) -> &str;
    fn read_bytes(&mut self, path: &str) -> AppResult<Vec<u8>>;
    /// Whether the export carries this listing at all. `groups.json`, `dms.json`
    /// and `mpims.json` are absent from plenty of legitimate exports.
    fn has(&mut self, path: &str) -> bool;
    /// The per-day message files for a channel, in the order Slack wrote them.
    fn channel_days(&mut self, channel_name: &str) -> AppResult<Vec<String>>;
    /// Every file in the archive with its size, for checking it against what the
    /// manifest says should be there. Slack's own manifest is not counted: it
    /// does not count itself either.
    fn entries(&mut self) -> Vec<(String, u64)>;

    fn read_json<T: DeserializeOwned>(&mut self, path: &str) -> AppResult<T>
    where
        Self: Sized,
    {
        let bytes = self.read_bytes(path)?;
        serde_json::from_slice(&bytes)
            .map_err(|e| AppError::BadRequest(format!("{path} is not the JSON expected: {e}")))
    }
}

/// `read_json` on a trait object, which the generic method cannot be.
pub fn read_json<T: DeserializeOwned>(source: &mut dyn ExportSource, path: &str) -> AppResult<T> {
    let bytes = source.read_bytes(path)?;
    serde_json::from_slice(&bytes)
        .map_err(|e| AppError::BadRequest(format!("{path} is not the JSON expected: {e}")))
}

pub struct DirectorySource {
    root: PathBuf,
    label: String,
}

impl DirectorySource {
    pub fn new(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref().to_path_buf();
        Self {
            label: root.display().to_string(),
            root,
        }
    }
}

impl ExportSource for DirectorySource {
    fn label(&self) -> &str {
        &self.label
    }

    fn read_bytes(&mut self, path: &str) -> AppResult<Vec<u8>> {
        fs::read(self.root.join(path))
            .map_err(|e| AppError::BadRequest(format!("cannot read {path}: {e}")))
    }

    fn has(&mut self, path: &str) -> bool {
        self.root.join(path).is_file()
    }

    fn entries(&mut self) -> Vec<(String, u64)> {
        let mut found = Vec::new();
        let mut stack = vec![self.root.clone()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.') {
                    continue;
                }
                if path.is_dir() {
                    stack.push(path);
                } else if let Ok(meta) = entry.metadata() {
                    found.push((name, meta.len()));
                }
            }
        }
        found
    }

    fn channel_days(&mut self, channel_name: &str) -> AppResult<Vec<String>> {
        let dir = self.root.join(channel_name);
        let Ok(entries) = fs::read_dir(&dir) else {
            return Ok(Vec::new());
        };

        let mut days: Vec<String> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().to_string();
                name.ends_with(".json")
                    .then(|| format!("{channel_name}/{name}"))
            })
            .collect();
        days.sort();
        Ok(days)
    }
}

pub struct ZipSource {
    archive: zip::ZipArchive<fs::File>,
    label: String,
}

impl ZipSource {
    pub fn open(path: impl AsRef<Path>) -> AppResult<Self> {
        let path = path.as_ref();
        let file = fs::File::open(path)
            .map_err(|e| AppError::BadRequest(format!("cannot open {}: {e}", path.display())))?;
        let archive = zip::ZipArchive::new(file)
            .map_err(|e| AppError::BadRequest(format!("{} is not a zip: {e}", path.display())))?;
        Ok(Self {
            archive,
            label: path.display().to_string(),
        })
    }
}

impl ExportSource for ZipSource {
    fn label(&self) -> &str {
        &self.label
    }

    fn read_bytes(&mut self, path: &str) -> AppResult<Vec<u8>> {
        let mut entry = self
            .archive
            .by_name(path)
            .map_err(|e| AppError::BadRequest(format!("cannot read {path} from the zip: {e}")))?;
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry
            .read_to_end(&mut bytes)
            .map_err(|e| AppError::BadRequest(format!("cannot read {path} from the zip: {e}")))?;
        Ok(bytes)
    }

    fn has(&mut self, path: &str) -> bool {
        self.archive.by_name(path).is_ok()
    }

    fn entries(&mut self) -> Vec<(String, u64)> {
        (0..self.archive.len())
            .filter_map(|i| {
                let entry = self.archive.by_index(i).ok()?;
                let name = entry.name().to_string();
                let base = name.rsplit('/').next().unwrap_or(&name);
                (!entry.is_dir() && !base.starts_with('.')).then(|| (name.clone(), entry.size()))
            })
            .collect()
    }

    fn channel_days(&mut self, channel_name: &str) -> AppResult<Vec<String>> {
        let prefix = format!("{channel_name}/");
        let mut days: Vec<String> = self
            .archive
            .file_names()
            .filter(|name| name.starts_with(&prefix) && name.ends_with(".json"))
            .map(str::to_string)
            .collect();
        days.sort();
        Ok(days)
    }
}

/// A directory or a zip, decided by what is actually there.
pub fn open(path: &Path) -> AppResult<Box<dyn ExportSource>> {
    if path.is_dir() {
        return Ok(Box::new(DirectorySource::new(path)));
    }
    Ok(Box::new(ZipSource::open(path)?))
}
