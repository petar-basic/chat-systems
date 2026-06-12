#!/bin/sh
# Periodic Postgres backup. Dumps to /backups on an interval, verifies the dump
# is intact before keeping it, prunes old dumps, and (optionally) mirrors each
# dump off-host. Runs as a long-lived sidecar in the prod compose stack.
set -eu

: "${POSTGRES_PASSWORD:?POSTGRES_PASSWORD must be set}"

BACKUP_DIR=/backups
RETENTION_DAYS="${BACKUP_RETENTION_DAYS:-7}"
INTERVAL_SECS="${BACKUP_INTERVAL_SECS:-86400}"
# Off-host copy: an rclone remote like "s3:my-bucket/chat-backups". Empty disables
# it. Requires rclone + its config to be mounted into this container.
OFFSITE_REMOTE="${BACKUP_OFFSITE_REMOTE:-}"

mkdir -p "$BACKUP_DIR"

backup_once() {
  ts=$(date +%Y%m%d-%H%M%S)
  file="$BACKUP_DIR/chatsystems-$ts.sql.gz"
  raw="$BACKUP_DIR/.chatsystems-$ts.sql.partial"
  tmp="$file.partial"
  echo "[backup] dumping to $file"

  # Dump to a plain file first: a piped `pg_dump | gzip` reports success even
  # when pg_dump dies mid-stream, because gzip exits 0 on truncated input.
  if ! PGPASSWORD="$POSTGRES_PASSWORD" pg_dump -h postgres -U chat -d chatsystems >"$raw"; then
    echo "[backup] FAILED: pg_dump error" >&2
    rm -f "$raw"
    return 1
  fi

  if ! gzip -c "$raw" >"$tmp"; then
    echo "[backup] FAILED: gzip error" >&2
    rm -f "$raw" "$tmp"
    return 1
  fi
  rm -f "$raw"

  if ! gzip -t "$tmp"; then
    echo "[backup] FAILED: integrity check failed" >&2
    rm -f "$tmp"
    return 1
  fi

  mv "$tmp" "$file"
  echo "[backup] ok ($(du -h "$file" | cut -f1))"

  if [ -n "$OFFSITE_REMOTE" ]; then
    if command -v rclone >/dev/null 2>&1 && rclone copy "$file" "$OFFSITE_REMOTE"; then
      echo "[backup] mirrored to $OFFSITE_REMOTE"
    else
      echo "[backup] WARNING: off-site mirror to $OFFSITE_REMOTE failed (rclone missing or error)" >&2
    fi
  fi
}

while true; do
  backup_once || echo "[backup] cycle failed; retrying next interval" >&2
  find "$BACKUP_DIR" -name 'chatsystems-*.sql.gz' -mtime +"$RETENTION_DAYS" -delete
  find "$BACKUP_DIR" -name '.chatsystems-*.partial' -mtime +1 -delete 2>/dev/null || true
  sleep "$INTERVAL_SECS"
done
