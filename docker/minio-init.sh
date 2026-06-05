#!/bin/sh
# Wait for MinIO to be ready, then create the default bucket.
set -e

until mc alias set local http://minio:9000 "$MINIO_ROOT_USER" "$MINIO_ROOT_PASSWORD" >/dev/null 2>&1; do
  echo "Waiting for MinIO..."
  sleep 2
done

if mc ls local/"$BUCKET_NAME" >/dev/null 2>&1; then
  echo "Bucket '$BUCKET_NAME' already exists."
else
  mc mb local/"$BUCKET_NAME"
  echo "Bucket '$BUCKET_NAME' created."
fi

# Keep the bucket private. Downloads are served through the authenticated API
# (/api/files/download/...), which enforces workspace + channel access; objects
# must never be anonymously readable by URL.
mc anonymous set none local/"$BUCKET_NAME" 2>/dev/null || true

echo "MinIO init complete."
