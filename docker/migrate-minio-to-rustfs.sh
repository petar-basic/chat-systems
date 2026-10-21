#!/bin/sh
set -eu

: "${OLD_VOLUME:?OLD_VOLUME must name the volume that held the MinIO data}"
: "${NETWORK:?NETWORK must name the Docker network the rustfs service is on}"
: "${MINIO_ROOT_USER:?MINIO_ROOT_USER must be set}"
: "${MINIO_ROOT_PASSWORD:?MINIO_ROOT_PASSWORD must be set}"

MINIO_IMAGE="${MINIO_IMAGE:-minio/minio:RELEASE.2025-04-22T22-12-26Z}"
RUSTFS_ENDPOINT="${RUSTFS_ENDPOINT:-http://rustfs:9000}"
BUCKET_NAME="${S3_BUCKET:-chatsystems}"
HELPER=minio-migrate-source

if ! docker image inspect "$MINIO_IMAGE" >/dev/null 2>&1; then
  echo "The MinIO image $MINIO_IMAGE is not on this host and can no longer be pulled." >&2
  echo "Run this on the host that ran MinIO, or set MINIO_IMAGE to an image that is." >&2
  exit 1
fi

docker rm -f "$HELPER" >/dev/null 2>&1 || true
docker run -d --name "$HELPER" --network "$NETWORK" \
  -e MINIO_ROOT_USER -e MINIO_ROOT_PASSWORD \
  -v "$OLD_VOLUME:/data" "$MINIO_IMAGE" server /data >/dev/null
trap 'docker rm -f "$HELPER" >/dev/null 2>&1 || true' EXIT

rclone() {
  docker run --rm --network "$NETWORK" \
    -e RCLONE_CONFIG_OLD_TYPE=s3 -e RCLONE_CONFIG_OLD_PROVIDER=Minio \
    -e RCLONE_CONFIG_OLD_ENDPOINT="http://$HELPER:9000" \
    -e RCLONE_CONFIG_OLD_ACCESS_KEY_ID="$MINIO_ROOT_USER" \
    -e RCLONE_CONFIG_OLD_SECRET_ACCESS_KEY="$MINIO_ROOT_PASSWORD" \
    -e RCLONE_CONFIG_NEW_TYPE=s3 -e RCLONE_CONFIG_NEW_PROVIDER=Other \
    -e RCLONE_CONFIG_NEW_ENDPOINT="$RUSTFS_ENDPOINT" \
    -e RCLONE_CONFIG_NEW_ACCESS_KEY_ID="$MINIO_ROOT_USER" \
    -e RCLONE_CONFIG_NEW_SECRET_ACCESS_KEY="$MINIO_ROOT_PASSWORD" \
    -e RCLONE_CONFIG_NEW_FORCE_PATH_STYLE=true \
    rclone/rclone:1.75.2 "$@"
}

until rclone lsd old: >/dev/null 2>&1; do
  echo "waiting for the old MinIO to start..."
  sleep 2
done

rclone mkdir "new:$BUCKET_NAME"
rclone copy --progress "old:$BUCKET_NAME" "new:$BUCKET_NAME"
rclone check "old:$BUCKET_NAME" "new:$BUCKET_NAME"
echo "Copied and verified bucket '$BUCKET_NAME'. The old volume $OLD_VOLUME is untouched."
