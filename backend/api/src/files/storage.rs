use std::path::PathBuf;

use async_trait::async_trait;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;
use tracing::info;

use shared_common::errors::{AppError, AppResult};

use crate::config::{AppConfig, StorageBackend};

#[async_trait]
pub trait FileStorage: Send + Sync {
    async fn upload(&self, key: &str, body: Vec<u8>, content_type: &str) -> AppResult<()>;
    async fn download(&self, key: &str) -> AppResult<(Vec<u8>, String)>;
    async fn delete(&self, key: &str) -> AppResult<()>;
    fn public_url(&self, key: &str) -> String;
}

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
