use std::sync::Arc;

use axum::body::Bytes;
use object_store::aws::AmazonS3Builder;
use object_store::local::LocalFileSystem;
use object_store::path::Path as ObjectPath;
use object_store::{
    Attribute, Attributes, ObjectStore, ObjectStoreExt, PutMultipartOptions, PutOptions,
    PutPayload, WriteMultipart,
};
use tracing::info;

use shared_common::errors::{AppError, AppResult};

use crate::config::{AppConfig, StorageBackend};

/// Below this an upload is a single PUT; above it the object is assembled
/// from parts, so peak memory stays at a few parts rather than one file.
const PART_SIZE: usize = 8 * 1024 * 1024;
const PARTS_IN_FLIGHT: usize = 2;

pub struct FileStorage {
    store: Arc<dyn ObjectStore>,
    public_url: String,
    stores_content_type: bool,
}

impl FileStorage {
    pub async fn from_config(config: &AppConfig) -> AppResult<Self> {
        match config.storage_backend {
            StorageBackend::Local => Self::local(&config.local_storage_path, &config.public_url),
            StorageBackend::S3 => Self::s3(config),
        }
    }

    pub fn local(base_path: &str, public_url: &str) -> AppResult<Self> {
        std::fs::create_dir_all(base_path)
            .map_err(|e| AppError::Internal(format!("Failed to create storage dir: {e}")))?;
        let store = LocalFileSystem::new_with_prefix(base_path)
            .map_err(|e| AppError::Internal(format!("Failed to open storage dir: {e}")))?;
        info!("Local storage initialized: path={base_path}");
        Ok(Self {
            store: Arc::new(store),
            public_url: public_url.to_string(),
            stores_content_type: false,
        })
    }

    fn s3(config: &AppConfig) -> AppResult<Self> {
        let store = AmazonS3Builder::new()
            .with_endpoint(&config.s3_endpoint)
            .with_region(&config.s3_region)
            .with_bucket_name(&config.s3_bucket)
            .with_access_key_id(&config.s3_access_key)
            .with_secret_access_key(&config.s3_secret_key)
            .with_virtual_hosted_style_request(false)
            .with_allow_http(config.s3_endpoint.starts_with("http://"))
            .build()
            .map_err(|e| AppError::Internal(format!("S3 storage config invalid: {e}")))?;
        info!("S3 storage initialized: bucket={}", config.s3_bucket);
        Ok(Self {
            store: Arc::new(store),
            public_url: config.public_url.clone(),
            stores_content_type: true,
        })
    }

    pub async fn upload(&self, key: &str, body: Vec<u8>, content_type: &str) -> AppResult<()> {
        let path = parse_key(key)?;
        let options = PutOptions {
            attributes: self.attributes_for(content_type),
            ..Default::default()
        };
        self.store
            .put_opts(&path, PutPayload::from(body), options)
            .await
            .map_err(|e| storage_error("upload", e))?;
        Ok(())
    }

    pub async fn begin_upload(&self, key: &str, content_type: &str) -> AppResult<Upload> {
        let path = parse_key(key)?;
        let attributes = self.attributes_for(content_type);
        Ok(Upload {
            store: self.store.clone(),
            path,
            attributes,
            buffer: Vec::new(),
            multipart: None,
            written: 0,
        })
    }

    pub async fn download(&self, key: &str) -> AppResult<(Bytes, String)> {
        let path = parse_key(key)?;
        let result = self
            .store
            .get(&path)
            .await
            .map_err(|e| storage_error("download", e))?;
        let content_type = result
            .attributes
            .get(&Attribute::ContentType)
            .map(|v| v.as_ref().to_string())
            .unwrap_or_else(|| {
                mime_guess::from_path(key)
                    .first_or_octet_stream()
                    .to_string()
            });
        let body = result.bytes().await.map_err(|e| storage_error("read", e))?;
        Ok((body, content_type))
    }

    pub async fn delete(&self, key: &str) -> AppResult<()> {
        let path = parse_key(key)?;
        self.store
            .delete(&path)
            .await
            .map_err(|e| storage_error("delete", e))
    }

    pub fn public_url(&self, key: &str) -> String {
        format!("{}/api/files/download/{}", self.public_url, key)
    }

    fn attributes_for(&self, content_type: &str) -> Attributes {
        let mut attributes = Attributes::new();
        if self.stores_content_type {
            attributes.insert(Attribute::ContentType, content_type.to_string().into());
        }
        attributes
    }
}

pub struct Upload {
    store: Arc<dyn ObjectStore>,
    path: ObjectPath,
    attributes: Attributes,
    buffer: Vec<u8>,
    multipart: Option<WriteMultipart>,
    written: u64,
}

impl Upload {
    pub async fn write_chunk(&mut self, chunk: Bytes) -> AppResult<()> {
        self.written += chunk.len() as u64;
        if let Some(multipart) = &mut self.multipart {
            multipart
                .wait_for_capacity(PARTS_IN_FLIGHT)
                .await
                .map_err(|e| storage_error("part upload", e))?;
            multipart.put(chunk);
            return Ok(());
        }

        self.buffer.extend_from_slice(&chunk);
        if self.buffer.len() >= PART_SIZE {
            let options = PutMultipartOptions {
                attributes: self.attributes.clone(),
                ..Default::default()
            };
            let upload = self
                .store
                .put_multipart_opts(&self.path, options)
                .await
                .map_err(|e| storage_error("multipart start", e))?;
            let mut multipart = WriteMultipart::new_with_chunk_size(upload, PART_SIZE);
            multipart.put(Bytes::from(std::mem::take(&mut self.buffer)));
            self.multipart = Some(multipart);
        }
        Ok(())
    }

    pub async fn finish(self) -> AppResult<u64> {
        match self.multipart {
            Some(multipart) => {
                multipart
                    .finish()
                    .await
                    .map_err(|e| storage_error("multipart complete", e))?;
            }
            None => {
                let options = PutOptions {
                    attributes: self.attributes,
                    ..Default::default()
                };
                self.store
                    .put_opts(&self.path, PutPayload::from(self.buffer), options)
                    .await
                    .map_err(|e| storage_error("upload", e))?;
            }
        }
        Ok(self.written)
    }

    pub async fn abort(self) -> AppResult<()> {
        if let Some(multipart) = self.multipart {
            let _ = multipart.abort().await;
        }
        Ok(())
    }
}

fn parse_key(key: &str) -> AppResult<ObjectPath> {
    if key.starts_with('/') {
        return Err(AppError::BadRequest("invalid path".into()));
    }
    ObjectPath::parse(key).map_err(|_| AppError::BadRequest("invalid path".into()))
}

fn storage_error(action: &str, e: object_store::Error) -> AppError {
    match e {
        object_store::Error::NotFound { .. } => AppError::NotFound("File not found".into()),
        other => AppError::Internal(format!("Storage {action} failed: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("chat_storage_test_{}_{}", std::process::id(), n));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn assert_rejected(key: &str) {
        match parse_key(key) {
            Err(AppError::BadRequest(_)) => {}
            Err(other) => panic!("key {key:?}: expected BadRequest, got {other:?}"),
            Ok(path) => panic!("key {key:?}: expected rejection, got Ok({path})"),
        }
    }

    #[test]
    fn keys_that_could_escape_the_store_are_rejected() {
        assert_rejected("../../etc/passwd");
        assert_rejected("/abs");
        assert_rejected("/etc/passwd");
        assert_rejected("ws/../../../etc/passwd");
        assert_rejected("ws//file.png");
    }

    #[test]
    fn a_normal_nested_key_is_accepted_as_is() {
        let key = "ws/550e8400-e29b-41d4-a716-446655440000/file.png";
        assert_eq!(parse_key(key).unwrap().as_ref(), key);
    }

    #[tokio::test]
    async fn a_small_upload_round_trips_and_stays_inside_the_base_dir() {
        let dir = temp_dir();
        let storage = FileStorage::local(dir.to_str().unwrap(), "http://x").unwrap();
        let key = "ws/abc/hello.txt";

        let mut upload = storage.begin_upload(key, "text/plain").await.unwrap();
        upload
            .write_chunk(Bytes::from_static(b"hello "))
            .await
            .unwrap();
        upload
            .write_chunk(Bytes::from_static(b"world"))
            .await
            .unwrap();
        assert_eq!(upload.finish().await.unwrap(), 11);

        assert!(dir.join(key).is_file());
        let (body, content_type) = storage.download(key).await.unwrap();
        assert_eq!(&body[..], b"hello world");
        assert_eq!(content_type, "text/plain");

        storage.delete(key).await.unwrap();
        assert!(matches!(
            storage.download(key).await,
            Err(AppError::NotFound(_))
        ));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn an_upload_larger_than_a_part_is_assembled_from_parts() {
        let dir = temp_dir();
        let storage = FileStorage::local(dir.to_str().unwrap(), "http://x").unwrap();
        let key = "ws/abc/big.bin";
        let chunk = Bytes::from(vec![7u8; 3 * 1024 * 1024]);

        let mut upload = storage
            .begin_upload(key, "application/octet-stream")
            .await
            .unwrap();
        for _ in 0..4 {
            upload.write_chunk(chunk.clone()).await.unwrap();
        }
        assert_eq!(upload.finish().await.unwrap(), 12 * 1024 * 1024);

        let (body, _) = storage.download(key).await.unwrap();
        assert_eq!(body.len(), 12 * 1024 * 1024);
        assert!(body.iter().all(|b| *b == 7));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn an_aborted_upload_leaves_nothing_behind() {
        let dir = temp_dir();
        let storage = FileStorage::local(dir.to_str().unwrap(), "http://x").unwrap();
        let key = "ws/abc/gone.bin";

        let mut upload = storage
            .begin_upload(key, "application/octet-stream")
            .await
            .unwrap();
        upload
            .write_chunk(Bytes::from(vec![1u8; PART_SIZE + 1]))
            .await
            .unwrap();
        upload.abort().await.unwrap();

        assert!(!dir.join(key).exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    fn minio_config() -> AppConfig {
        let mut config = crate::http_tests::common::test_config();
        config.storage_backend = StorageBackend::S3;
        config.s3_endpoint =
            std::env::var("S3_ENDPOINT").unwrap_or_else(|_| "http://127.0.0.1:9000".into());
        config.s3_bucket = std::env::var("S3_BUCKET").unwrap_or_else(|_| "chatsystems".into());
        config.s3_access_key =
            std::env::var("S3_ACCESS_KEY").unwrap_or_else(|_| "minioadmin".into());
        config.s3_secret_key =
            std::env::var("S3_SECRET_KEY").unwrap_or_else(|_| "minioadmin".into());
        config
    }

    #[tokio::test]
    #[ignore = "needs a MinIO on S3_ENDPOINT: docker compose up -d minio minio-init"]
    async fn s3_round_trip_including_a_multipart_object() {
        let storage = FileStorage::from_config(&minio_config()).await.unwrap();
        let key = format!("test/{}/big.bin", uuid::Uuid::new_v4());
        let chunk = Bytes::from(vec![9u8; 3 * 1024 * 1024]);

        let mut upload = storage
            .begin_upload(&key, "application/x-test")
            .await
            .unwrap();
        for _ in 0..4 {
            upload.write_chunk(chunk.clone()).await.unwrap();
        }
        assert_eq!(upload.finish().await.unwrap(), 12 * 1024 * 1024);

        let (body, content_type) = storage.download(&key).await.unwrap();
        assert_eq!(body.len(), 12 * 1024 * 1024);
        assert_eq!(content_type, "application/x-test");

        let small_key = format!("test/{}/small.txt", uuid::Uuid::new_v4());
        storage
            .upload(&small_key, b"hi".to_vec(), "text/plain")
            .await
            .unwrap();
        let (body, content_type) = storage.download(&small_key).await.unwrap();
        assert_eq!(&body[..], b"hi");
        assert_eq!(content_type, "text/plain");

        storage.delete(&key).await.unwrap();
        storage.delete(&small_key).await.unwrap();
        assert!(matches!(
            storage.download(&key).await,
            Err(AppError::NotFound(_))
        ));
    }
}
