use std::path::PathBuf;

use async_trait::async_trait;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;
use axum::body::Bytes;
use tokio::io::AsyncWriteExt;
use tracing::info;

use shared_common::errors::{AppError, AppResult};

use crate::config::{AppConfig, StorageBackend};

/// Accepts an upload one chunk at a time so a large file never has to exist in
/// memory. `finish` returns the number of bytes actually written; `abort`
/// removes whatever was already stored.
#[async_trait]
pub trait UploadSink: Send {
    async fn write_chunk(&mut self, chunk: Bytes) -> AppResult<()>;
    async fn finish(self: Box<Self>) -> AppResult<u64>;
    async fn abort(self: Box<Self>) -> AppResult<()>;
}

#[async_trait]
pub trait FileStorage: Send + Sync {
    async fn upload(&self, key: &str, body: Vec<u8>, content_type: &str) -> AppResult<()>;
    async fn begin_upload(&self, key: &str, content_type: &str) -> AppResult<Box<dyn UploadSink>>;
    async fn download(&self, key: &str) -> AppResult<(Vec<u8>, String)>;
    async fn delete(&self, key: &str) -> AppResult<()>;
    fn public_url(&self, key: &str) -> String;
}

/// Below this an S3 upload is a single PUT; above it the object is assembled
/// from parts, so peak memory stays at one part rather than one file.
const S3_PART_SIZE: usize = 8 * 1024 * 1024;

pub async fn create_storage(config: &AppConfig) -> AppResult<Box<dyn FileStorage>> {
    match config.storage_backend {
        StorageBackend::Local => {
            let storage = LocalStorage::new(&config.local_storage_path, &config.public_url)?;
            Ok(Box::new(storage))
        }
        StorageBackend::S3 => {
            let storage = S3Storage::new(config).await?;
            Ok(Box::new(storage))
        }
    }
}

pub struct LocalStorage {
    base_path: PathBuf,
    public_url: String,
}

impl LocalStorage {
    pub fn new(base_path: &str, public_url: &str) -> AppResult<Self> {
        let path = PathBuf::from(base_path);
        std::fs::create_dir_all(&path)
            .map_err(|e| AppError::Internal(format!("Failed to create storage dir: {e}")))?;
        info!("Local storage initialized: path={}", path.display());
        Ok(Self {
            base_path: path,
            public_url: public_url.to_string(),
        })
    }

    fn key_path(&self, key: &str) -> AppResult<PathBuf> {
        if key.contains("..") || key.starts_with('/') {
            return Err(AppError::BadRequest("invalid path".into()));
        }

        let path = self.base_path.join(key);

        let parent = path
            .parent()
            .ok_or_else(|| AppError::BadRequest("invalid path".into()))?;

        let canonical_base = self
            .base_path
            .canonicalize()
            .map_err(|e| AppError::Internal(format!("Failed to resolve storage dir: {e}")))?;

        if parent.exists() {
            let canonical_parent = parent
                .canonicalize()
                .map_err(|e| AppError::Internal(format!("Failed to resolve path: {e}")))?;
            if !canonical_parent.starts_with(&canonical_base) {
                return Err(AppError::BadRequest("invalid path".into()));
            }
        } else if !parent.starts_with(&self.base_path) {
            return Err(AppError::BadRequest("invalid path".into()));
        }

        Ok(path)
    }
}

#[async_trait]
impl FileStorage for LocalStorage {
    async fn upload(&self, key: &str, body: Vec<u8>, _content_type: &str) -> AppResult<()> {
        let path = self.key_path(key)?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| AppError::Internal(format!("mkdir failed: {e}")))?;
        }
        tokio::fs::write(&path, &body)
            .await
            .map_err(|e| AppError::Internal(format!("File write failed: {e}")))?;
        Ok(())
    }

    async fn begin_upload(&self, key: &str, _content_type: &str) -> AppResult<Box<dyn UploadSink>> {
        let path = self.key_path(key)?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| AppError::Internal(format!("mkdir failed: {e}")))?;
        }
        let file = tokio::fs::File::create(&path)
            .await
            .map_err(|e| AppError::Internal(format!("File create failed: {e}")))?;
        Ok(Box::new(LocalUploadSink {
            file,
            path,
            written: 0,
        }))
    }

    async fn download(&self, key: &str) -> AppResult<(Vec<u8>, String)> {
        let path = self.key_path(key)?;
        let body = tokio::fs::read(&path)
            .await
            .map_err(|e| AppError::NotFound(format!("File not found: {e}")))?;
        let content_type = mime_guess::from_path(&path)
            .first_or_octet_stream()
            .to_string();
        Ok((body, content_type))
    }

    async fn delete(&self, key: &str) -> AppResult<()> {
        let path = self.key_path(key)?;
        tokio::fs::remove_file(&path)
            .await
            .map_err(|e| AppError::Internal(format!("File delete failed: {e}")))?;
        Ok(())
    }

    fn public_url(&self, key: &str) -> String {
        format!("{}/api/files/download/{}", self.public_url, key)
    }
}

pub struct S3Storage {
    client: Client,
    bucket: String,
    public_url: String,
}

impl S3Storage {
    pub async fn new(config: &AppConfig) -> AppResult<Self> {
        let creds = aws_sdk_s3::config::Credentials::new(
            &config.s3_access_key,
            &config.s3_secret_key,
            None,
            None,
            "env",
        );

        let s3_config = aws_sdk_s3::Config::builder()
            .endpoint_url(&config.s3_endpoint)
            .region(aws_sdk_s3::config::Region::new(config.s3_region.clone()))
            .credentials_provider(creds)
            .force_path_style(true)
            .behavior_version_latest()
            .build();

        let client = Client::from_conf(s3_config);

        info!("S3 storage initialized: bucket={}", config.s3_bucket);
        Ok(Self {
            client,
            bucket: config.s3_bucket.clone(),
            public_url: config.public_url.clone(),
        })
    }
}

#[async_trait]
impl FileStorage for S3Storage {
    async fn upload(&self, key: &str, body: Vec<u8>, content_type: &str) -> AppResult<()> {
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(ByteStream::from(body))
            .content_type(content_type)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("S3 upload failed: {e}")))?;
        Ok(())
    }

    async fn begin_upload(&self, key: &str, content_type: &str) -> AppResult<Box<dyn UploadSink>> {
        Ok(Box::new(S3UploadSink {
            client: self.client.clone(),
            bucket: self.bucket.clone(),
            key: key.to_string(),
            content_type: content_type.to_string(),
            upload_id: None,
            buffer: Vec::with_capacity(S3_PART_SIZE),
            parts: Vec::new(),
            written: 0,
        }))
    }

    async fn download(&self, key: &str) -> AppResult<(Vec<u8>, String)> {
        let resp = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("S3 download failed: {e}")))?;

        let content_type = resp
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_string();
        let body = resp
            .body
            .collect()
            .await
            .map_err(|e| AppError::Internal(format!("S3 body read failed: {e}")))?
            .into_bytes()
            .to_vec();

        Ok((body, content_type))
    }

    async fn delete(&self, key: &str) -> AppResult<()> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("S3 delete failed: {e}")))?;
        Ok(())
    }

    fn public_url(&self, key: &str) -> String {
        format!("{}/api/files/download/{}", self.public_url, key)
    }
}

struct LocalUploadSink {
    file: tokio::fs::File,
    path: PathBuf,
    written: u64,
}

#[async_trait]
impl UploadSink for LocalUploadSink {
    async fn write_chunk(&mut self, chunk: Bytes) -> AppResult<()> {
        self.file
            .write_all(&chunk)
            .await
            .map_err(|e| AppError::Internal(format!("File write failed: {e}")))?;
        self.written += chunk.len() as u64;
        Ok(())
    }

    async fn finish(mut self: Box<Self>) -> AppResult<u64> {
        self.file
            .flush()
            .await
            .map_err(|e| AppError::Internal(format!("File flush failed: {e}")))?;
        Ok(self.written)
    }

    async fn abort(self: Box<Self>) -> AppResult<()> {
        let _ = tokio::fs::remove_file(&self.path).await;
        Ok(())
    }
}

struct S3UploadSink {
    client: Client,
    bucket: String,
    key: String,
    content_type: String,
    upload_id: Option<String>,
    buffer: Vec<u8>,
    parts: Vec<aws_sdk_s3::types::CompletedPart>,
    written: u64,
}

impl S3UploadSink {
    async fn flush_part(&mut self) -> AppResult<()> {
        if self.upload_id.is_none() {
            let started = self
                .client
                .create_multipart_upload()
                .bucket(&self.bucket)
                .key(&self.key)
                .content_type(&self.content_type)
                .send()
                .await
                .map_err(|e| AppError::Internal(format!("S3 multipart start failed: {e}")))?;
            self.upload_id = started.upload_id().map(std::string::ToString::to_string);
        }
        let upload_id = self
            .upload_id
            .clone()
            .ok_or_else(|| AppError::Internal("S3 multipart upload has no id".into()))?;

        let part_number = self.parts.len() as i32 + 1;
        let body = std::mem::replace(&mut self.buffer, Vec::with_capacity(S3_PART_SIZE));
        let uploaded = self
            .client
            .upload_part()
            .bucket(&self.bucket)
            .key(&self.key)
            .upload_id(&upload_id)
            .part_number(part_number)
            .body(ByteStream::from(body))
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("S3 part upload failed: {e}")))?;

        self.parts.push(
            aws_sdk_s3::types::CompletedPart::builder()
                .set_e_tag(uploaded.e_tag().map(std::string::ToString::to_string))
                .part_number(part_number)
                .build(),
        );
        Ok(())
    }
}

#[async_trait]
impl UploadSink for S3UploadSink {
    async fn write_chunk(&mut self, chunk: Bytes) -> AppResult<()> {
        self.written += chunk.len() as u64;
        self.buffer.extend_from_slice(&chunk);
        if self.buffer.len() >= S3_PART_SIZE {
            self.flush_part().await?;
        }
        Ok(())
    }

    async fn finish(mut self: Box<Self>) -> AppResult<u64> {
        // Never grew past one part: a plain PUT is cheaper and avoids S3's
        // 5 MiB minimum for non-final parts.
        if self.upload_id.is_none() {
            let body = std::mem::take(&mut self.buffer);
            self.client
                .put_object()
                .bucket(&self.bucket)
                .key(&self.key)
                .body(ByteStream::from(body))
                .content_type(&self.content_type)
                .send()
                .await
                .map_err(|e| AppError::Internal(format!("S3 upload failed: {e}")))?;
            return Ok(self.written);
        }

        if !self.buffer.is_empty() {
            self.flush_part().await?;
        }
        let upload_id = self
            .upload_id
            .clone()
            .ok_or_else(|| AppError::Internal("S3 multipart upload has no id".into()))?;

        self.client
            .complete_multipart_upload()
            .bucket(&self.bucket)
            .key(&self.key)
            .upload_id(&upload_id)
            .multipart_upload(
                aws_sdk_s3::types::CompletedMultipartUpload::builder()
                    .set_parts(Some(std::mem::take(&mut self.parts)))
                    .build(),
            )
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("S3 multipart complete failed: {e}")))?;

        Ok(self.written)
    }

    async fn abort(self: Box<Self>) -> AppResult<()> {
        if let Some(upload_id) = &self.upload_id {
            let _ = self
                .client
                .abort_multipart_upload()
                .bucket(&self.bucket)
                .key(&self.key)
                .upload_id(upload_id)
                .send()
                .await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_storage() -> (LocalStorage, PathBuf) {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);

        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("chat_storage_test_{}_{}", std::process::id(), n));
        let _ = std::fs::remove_dir_all(&dir);
        let storage = LocalStorage::new(dir.to_str().unwrap(), "http://x")
            .expect("LocalStorage::new should create the base dir");
        (storage, dir)
    }

    fn assert_rejected(result: AppResult<PathBuf>, key: &str) {
        match result {
            Err(AppError::BadRequest(_)) => {}
            Err(other) => panic!("key {key:?}: expected BadRequest, got {other:?}"),
            Ok(path) => panic!(
                "key {key:?}: expected rejection, got Ok({})",
                path.display()
            ),
        }
    }

    #[test]
    fn key_path_rejects_parent_dir_traversal() {
        let (storage, _dir) = temp_storage();
        assert_rejected(storage.key_path("../../etc/passwd"), "../../etc/passwd");
    }

    #[test]
    fn key_path_rejects_absolute_path() {
        let (storage, _dir) = temp_storage();
        assert_rejected(storage.key_path("/abs"), "/abs");
        assert_rejected(storage.key_path("/etc/passwd"), "/etc/passwd");
    }

    #[test]
    fn key_path_rejects_embedded_dotdot() {
        let (storage, _dir) = temp_storage();
        assert_rejected(
            storage.key_path("ws/../../../etc/passwd"),
            "ws/../../../etc/passwd",
        );
    }

    #[test]
    fn key_path_accepts_normal_key_and_stays_within_base() {
        let (storage, dir) = temp_storage();
        let key = "ws/550e8400-e29b-41d4-a716-446655440000/file.png";

        let resolved = storage
            .key_path(key)
            .expect("a normal nested key should be accepted");

        assert_eq!(resolved, dir.join(key));
        assert!(
            resolved.starts_with(&dir),
            "resolved path {} escaped base {}",
            resolved.display(),
            dir.display(),
        );

        std::fs::remove_dir_all(&dir).ok();
    }
}
