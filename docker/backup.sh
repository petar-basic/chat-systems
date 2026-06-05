#!/bin/sh
# Periodic Postgres backup. Dumps to /backups on an interval and prunes old
# dumps. Runs as a long-lived sidecar in the prod compose stack.
set -eu

: "${POSTGRES_PASSWORD:?POSTGRES_PASSWORD must be set}"

BACKUP_DIR=/backups
RETENTION_DAYS="${BACKUP_RETENTION_DAYS:-7}"
INTERVAL_SECS="${BACKUP_INTERVAL_SECS:-86400}"

mkdir -p "$BACKUP_DIR"

while true; do
  ts=$(date +%Y%m%d-%H%M%S)
  file="$BACKUP_DIR/chatsystems-$ts.sql.gz"
  echo "[backup] dumping to $file"
  if PGPASSWORD="$POSTGRES_PASSWORD" pg_dump -h postgres -U chat -d chatsystems | gzip > "$file"; then
    echo "[backup] ok ($(du -h "$file" | cut -f1))"
  else
    echo "[backup] FAILED" >&2
    rm -f "$file"
  fi
  find "$BACKUP_DIR" -name 'chatsystems-*.sql.gz' -mtime +"$RETENTION_DAYS" -delete
  sleep "$INTERVAL_SECS"
done
