#!/bin/sh
set -eu

BUCKET_NAME="${BUCKET_NAME:-chatsystems}"

until rclone lsd store: >/dev/null 2>&1; do
  echo "[storage-init] waiting for the object store..."
  sleep 2
done

rclone mkdir "store:$BUCKET_NAME"
echo "[storage-init] bucket '$BUCKET_NAME' ready"
