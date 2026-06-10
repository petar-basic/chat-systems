#!/bin/sh
# Periodic MinIO bucket backup. Mirrors the object store into /backups on an
# interval so file uploads survive a host/volume loss alongside the Postgres
# dump. Additive (no --remove): objects are retained in the backup even if
# later deleted from the live bucket.
set -eu

: "${MINIO_ROOT_USER:?MINIO_ROOT_USER must be set}"
: "${MINIO_ROOT_PASSWORD:?MINIO_ROOT_PASSWORD must be set}"

BUCKET_NAME="${BUCKET_NAME:-chatsystems}"
BACKUP_DIR=/backups
INTERVAL_SECS="${BACKUP_INTERVAL_SECS:-86400}"

until mc alias set local http://minio:9000 "$MINIO_ROOT_USER" "$MINIO_ROOT_PASSWORD" >/dev/null 2>&1; do
  echo "[minio-backup] waiting for MinIO..."
  sleep 2
done

mkdir -p "$BACKUP_DIR/$BUCKET_NAME"

while true; do
  echo "[minio-backup] mirroring bucket '$BUCKET_NAME'"
  if mc mirror --overwrite "local/$BUCKET_NAME" "$BACKUP_DIR/$BUCKET_NAME"; then
    echo "[minio-backup] ok"
  else
    echo "[minio-backup] FAILED" >&2
  fi
  sleep "$INTERVAL_SECS"
done
