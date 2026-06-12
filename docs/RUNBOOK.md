# Operations Runbook

Day-2 operations for a self-hosted Chat Systems instance: backups, restore,
upgrades, rollback, and the handful of alerts worth wiring up. Assumes the
production stack from [CONTRIBUTING.md](CONTRIBUTING.md):

```bash
docker compose -f docker-compose.yml -f docker-compose.prod.yml \
  --profile frontend --profile s3 up -d --build
```

---

## What gets backed up

| Data | Where it lives | Backed up by | Backup volume |
|------|----------------|--------------|---------------|
| Postgres (messages, users, workspaces, …) | `postgres_data` | `db-backup` sidecar (`pg_dump`, hourly/daily) | `pg_backups` |
| File uploads (attachments, avatars) | MinIO `minio_data` | `minio-backup` sidecar (`mc mirror`) | `minio_backups` |
| Caddy TLS certs | `caddy_data` | not automated — see below | — |

Both sidecars run on `BACKUP_INTERVAL_SECS` (default `86400` = daily). The
Postgres dump is **verified with `gzip -t` before it is kept** — a truncated or
corrupt dump is discarded and logged as `FAILED`, never reported as `ok`.

> **The backup volumes sit on the same host as the live data.** A disk or host
> loss takes both with it. For real durability set an off-site target (below).

### Off-site copy (strongly recommended)

Set `BACKUP_OFFSITE_REMOTE` to an [rclone](https://rclone.org/) remote and mount
rclone + its config into the `db-backup` container; each verified dump is then
copied off-host. Example `.env`:

```
BACKUP_OFFSITE_REMOTE=s3:my-org-backups/chat-systems
```

Caddy certs re-issue automatically from Let's Encrypt on a fresh host, so they
don't strictly need backing up; to avoid rate limits during a rebuild, also copy
the `caddy_data` volume off-host periodically.

---

## Restore onto a fresh host

Goal: bring a new machine up from the backup volumes (or off-site copies). Do
this with the app **stopped** so nothing writes mid-restore.

### 0. Prerequisites
- Docker + the repo checked out, `.env` restored with the **same** `POSTGRES_PASSWORD`, `JWT_SECRET`, `MINIO_ROOT_*` as the old host.
- The backup files available: a `chatsystems-*.sql.gz` dump and the mirrored bucket tree.

### 1. Start only the data services
```bash
docker compose -f docker-compose.yml -f docker-compose.prod.yml \
  --profile s3 up -d postgres redis minio
```

### 2. Restore Postgres
Copy your chosen dump into the `db-backup` container (or any psql-capable one),
then load it:
```bash
# pick the newest verified dump
docker compose ... exec db-backup sh -c 'ls -1 /backups/chatsystems-*.sql.gz | tail -1'

# verify integrity, then restore into an empty DB
docker compose ... exec db-backup sh -c '
  gzip -t /backups/chatsystems-YYYYMMDD-HHMMSS.sql.gz &&
  gunzip -c /backups/chatsystems-YYYYMMDD-HHMMSS.sql.gz |
  PGPASSWORD="$POSTGRES_PASSWORD" psql -h postgres -U chat -d chatsystems'
```
If restoring into a DB that already has schema, drop/recreate it first:
`PGPASSWORD=… psql -h postgres -U chat -d postgres -c 'DROP DATABASE chatsystems; CREATE DATABASE chatsystems OWNER chat;'`

### 3. Restore MinIO objects
Mirror the backed-up bucket tree back into MinIO:
```bash
docker compose ... exec minio-backup sh -c '
  mc alias set local http://minio:9000 "$MINIO_ROOT_USER" "$MINIO_ROOT_PASSWORD" &&
  mc mb --ignore-existing local/$BUCKET_NAME &&
  mc mirror --overwrite /backups/$BUCKET_NAME local/$BUCKET_NAME'
```

### 4. Start the app and verify
```bash
docker compose -f docker-compose.yml -f docker-compose.prod.yml \
  --profile frontend --profile s3 up -d
# both must report ready:
curl -fsS http://127.0.0.1:3000/readyz   # api  → "ready"
curl -fsS http://127.0.0.1:3004/readyz   # realtime → "ready"
```
Then log in and spot-check: a channel with history, an uploaded file opens, a new message broadcasts in real time.

---

## Upgrade

The API runs `sqlx` migrations forward on startup; they are **forward-only**.
Always snapshot before upgrading so a bad migration is recoverable.

```bash
# 1. Snapshot first (on-demand dump, independent of the timer)
docker compose ... exec db-backup sh -c '
  PGPASSWORD="$POSTGRES_PASSWORD" pg_dump -h postgres -U chat -d chatsystems |
  gzip > /backups/pre-upgrade-$(date +%Y%m%d-%H%M%S).sql.gz'

# 2. Pull the new code and rebuild tagged images
git pull
VERSION=$(git rev-parse --short HEAD) docker compose \
  -f docker-compose.yml -f docker-compose.prod.yml --profile frontend --profile s3 \
  up -d --build

# 3. Gate on health
curl -fsS http://127.0.0.1:3000/readyz && curl -fsS http://127.0.0.1:3004/readyz
```

`VERSION` tags the `chat-api` / `chat-realtime` / `chat-frontend` images (default
`latest`). Use the git short SHA (or a release tag) so the previous image stays
addressable for rollback.

## Rollback

```bash
# Code-only regression (migrations unchanged): just re-point to the old images
VERSION=<previous-sha> docker compose \
  -f docker-compose.yml -f docker-compose.prod.yml --profile frontend --profile s3 up -d

# Bad migration: restore the pre-upgrade dump (see "Restore" above) onto the
# previous VERSION, since migrations don't run backward.
```

---

## Alerts worth wiring

Both services expose Prometheus metrics at `/metrics` (api on `:3000`, realtime
on `:3004`). At minimum, alert on:

| Alert | Condition | Why |
|-------|-----------|-----|
| Service unhealthy | `/readyz` returns 5xx for > 2 min | DB/Redis down, or (realtime) the event consumer stalled — no real-time delivery |
| Stale backup | newest `chatsystems-*.sql.gz` older than `26h` | the backup timer has silently stopped |
| Disk pressure | backup/data volume > 85 % full | dumps will start failing |

Useful series exposed today: `http_requests_total`,
`http_request_duration_seconds` (api); `realtime_ws_connections`,
`realtime_consumer_heartbeat_age_seconds`, `realtime_events_total` (realtime).
A rising `realtime_consumer_heartbeat_age_seconds` is the early signal that
real-time delivery is wedged even while the process is still alive.

The prod stack already runs an `autoheal` sidecar that restarts any container
Docker marks unhealthy (api/realtime via their `/readyz` healthcheck), so a
wedged process self-heals without paging anyone.
