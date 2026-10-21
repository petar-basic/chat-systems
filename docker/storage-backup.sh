#!/bin/sh
set -eu

BUCKET_NAME="${BUCKET_NAME:-chatsystems}"
BACKUP_DIR=/backups
INTERVAL_SECS="${BACKUP_INTERVAL_SECS:-86400}"

until rclone lsd store: >/dev/null 2>&1; do
  echo "[storage-backup] waiting for the object store..."
  sleep 2
done

mkdir -p "$BACKUP_DIR/$BUCKET_NAME"

while true; do
  echo "[storage-backup] copying bucket '$BUCKET_NAME'"
  if rclone copy "store:$BUCKET_NAME" "$BACKUP_DIR/$BUCKET_NAME"; then
    echo "[storage-backup] ok"
  else
    echo "[storage-backup] FAILED" >&2
  fi
  sleep "$INTERVAL_SECS"
done
