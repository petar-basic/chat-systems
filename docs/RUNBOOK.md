# Operations Runbook

Day-2 operations for a self-hosted Chat Systems instance: backups, restore,
upgrades, rollback, and the handful of alerts worth wiring up. Assumes the
production stack from [CONTRIBUTING.md](CONTRIBUTING.md):

```bash
docker compose -f docker-compose.yml -f docker-compose.prod.yml \
  --profile frontend --profile s3 up -d --build
```

---

## Processes

| Container | What it does | Safe to scale? |
|---|---|---|
| `api` | REST API; applies migrations at startup | yes |
| `worker` | Background consumers: outgoing webhooks, reminders, notifications, huddle history, scheduled messages | **no — keep at one replica** |
| `realtime` | WebSocket gateway | yes |

Running two `worker` replicas duplicates every side effect they produce: two
notification rows per mention, two POSTs per outgoing webhook, two reminder
deliveries. The compose files pin `replicas: 1`; leave it there. If `worker` is down,
nothing is lost permanently — messages still send and read — but webhooks, reminders,
scheduled messages and notification rows stop until it returns. Its `/readyz` is
wired to the autoheal sidecar like the others.

## What self-hosting actually costs

### Footprint

Measured on an idle instance with nobody connected:

| Container | Resident memory |
|---|---|
| `api` | ~250 MB |
| `postgres` | ~165 MB |
| `redis` | ~13 MB |
| `frontend` (nginx) | ~13 MB |
| `worker` | ~5 MB |
| `realtime` | ~3 MB |

Roughly 450 MB for the core, plus MinIO and Caddy in the production stack — call it
under 1 GB idle. The per-container limits in `docker-compose.prod.yml` total about 4 GB
and exist to stop one runaway process taking the host down, not because the stack needs
that much. Two CPU cores and 4 GB of RAM is a comfortable starting point for a team of a
few dozen; disk is the part that grows, and it grows with attachments rather than with
messages.

### On somebody's laptop, or on a server?

A laptop is fine for trying it out with a couple of people on the same network. For a team
that depends on it, four things break, and none of them is about performance:

- **Sleep.** A closed lid stops the containers. Everyone loses real-time delivery; messages
  sent meanwhile are still delivered on reconnect, but push notifications stop, because the
  worker that sends them is asleep too.
- **A reachable address.** Home connections change IP. Without a stable name, every client
  has to be reconfigured when the ISP renumbers you. Dynamic DNS solves this; port
  forwarding for 80 and 443 is then required for Caddy to obtain a certificate at all.
- **TLS.** Service workers — and therefore Web Push and the installable PWA — need a secure
  context. `http://192.168.x.x` does not get one, so a laptop deployment without a real
  certificate loses notifications entirely on top of the sleep problem.
- **Huddles across networks.** WebRTC needs TURN when both sides are behind NAT. The
  `coturn` service is there for it, but it has to be reachable on a public address; on a
  home connection that means forwarding its ports too.

None of this is exotic — a €5/month VPS with a DNS name removes all four at once. The
honest summary: a laptop is a demo, not a deployment.

### What upkeep looks like

Automated already: migrations run forward at api startup, an `autoheal` sidecar restarts
anything Docker marks unhealthy, `db-backup` dumps Postgres and verifies each dump with
`gzip -t` before keeping it, `minio-backup` mirrors uploads, and retention/cleanup jobs run
in the worker.

What still needs a person:

| Task | How often | Roughly |
|---|---|---|
| Apply an upgrade (snapshot, pull, rebuild, check `/readyz`) | per release | 15–30 min |
| Read the release's upgrade note in this file | per release | 5 min |
| Confirm backups are current and off-site | monthly | 10 min |
| Practice a restore onto a scratch host | quarterly | an hour, and worth it |
| Act on a dependency advisory | as they land | varies |

Budget an hour a month in the steady state, plus the upgrade time for whatever cadence you
choose to follow. The two things that actually bite people are skipping the off-site backup
target — the backup volumes sit on the same host as the live data — and never testing a
restore until they need one.

## Upgrade note: Wave 1 expires every outstanding invite

The `20240305000016_invite_lifecycle` migration backfills an expiry onto every invite that
had none, which retires every outstanding invite link. Invites were previously unlimited
and eternal, and there is no way to tell a legitimate outstanding link from a leaked one.
Tell admins to re-send before you deploy, or expect a short window of "my invite link
stopped working".

`20240305000017_conversation_attachments` attributes existing DM attachments to the
message that posted them. Anything it cannot attribute becomes readable only by its
uploader — that is the intended fail-closed default, not data loss; the file is untouched.

## Upgrade note: Wave 4 deactivates every outgoing webhook

`20240305000019_scope_outgoing_hooks` turns off every existing outgoing webhook. They now
require an explicit `channel_ids` allow-list, and an old hook carries no record of which
channels it was ever meant to see. Re-create them from **Integrations** with the channels
they may read; the alternative was leaving them running against every channel in the
workspace, which is the leak the change closes. Incoming webhooks are unaffected.

Two more changes ship with it and need no action:

- `20240305000018_audit_trail` drops the `audit_log` foreign keys to `workspaces` and
  `users`. The trail is append-only history and has to outlive what it describes — with
  the reference in place, a hard workspace delete either fails on it or takes the record
  of the deletion with it.
- Attachments now follow their message. Deleting a message, or editing it to drop the
  link, **hard-deletes the stored object immediately**. There is no grace period and no
  purge job to run; restoring one means restoring from backup.

## Upgrade note: Wave 5 changes the DM send contract

`POST /api/conversations/:id/messages` and `POST /api/channels/:id/messages` no longer
accept `id`. The server owns the message id; a sender that wants an idempotent retry passes
`client_message_id` instead, unique within the conversation or channel. An old client that
still sends `id` is not rejected — the field is ignored, so its retries stop being
idempotent and a double-send stores two rows. Ship the frontend and the API together.

`20240305000020_client_message_id` adds the column to both tables with a partial unique
index. Existing rows keep `client_message_id NULL`, which the index excludes; nothing is
backfilled and nothing breaks.

Scheduled messages are now re-authorized at delivery. A message whose author has since lost
access to the destination is not delivered: the row records a reason
(`not_authorized`, `channel_archived`, `workspace_unavailable`), the author gets a
notification, and the row stays visible in their scheduled list rather than disappearing.
Expect a small burst of these on the first tick after upgrading if people have been removed
from channels while messages were queued. Removal now also cancels pending messages for
that scope, so the burst is one-off.

## Upgrade note: Wave 6 backfills unread counters

`20240305000021_unread_counters` adds `unread_count`, `mention_count` and
`last_read_message_id` to `channel_members` and backfills the first from the message table.
On an instance with a large `messages` table this is the slow statement in the migration —
it is a single pass, but budget for it and do not run it during peak.

`mention_count` is **not** backfilled: reconstructing who was mentioned in historical
messages would mean re-parsing every message body. It starts at zero and is correct from the
first message after the upgrade. Expect mention badges to be missing for pre-upgrade
messages until people read those channels.

A reconciler runs in `chat-worker` every six hours over channels active in the last day and
logs `Unread reconciler corrected drifted unread counters` with a count when it finds drift.
A non-zero number there is worth reading — it means something wrote a message without going
through `create_message`, or a restore reset the read state.

## Upgrade note: Wave 6 changes the presence key layout

Presence moved from `presence:{user_id}:{node_id}` (one key per user per node, 60s TTL) to
one sorted set per workspace, `presence:ws:{workspace_id}`, scored by expiry. The old keys
are **not** migrated: they expire on their own within a minute, and during a rolling deploy
users on old nodes are simply absent from the new roster until they reconnect. Presence is
recomputed from live connections, so nothing is lost permanently.

There is no sweeper to run. Expired entries are trimmed on read, so a node that dies takes
its users offline within the TTL by itself.

`realtime_presence_query_duration_seconds` is a new histogram. If it grows with anything
other than workspace size, something is wrong.

## Upgrade note: Wave 7 adds a replay log in Redis

Durable events (messages, reactions, conversations, workspace membership) are now written
to a Redis Stream per workspace, `stream:ws:{workspace_id}`, in addition to being published
live. **Budget Redis memory for it:** each stream is capped at 10,000 entries and trimmed to
one hour by a worker task, so the steady state is roughly "an hour of a workspace's events",
a few MB for an active workspace. `stream:index` is the set of live stream keys; the worker
reads it instead of scanning.

The gateway is backwards compatible with an old client — one that sends no `last_event_id`
simply gets no replay, exactly as before — but an old **gateway** with a new client is not
useful, so deploy realtime before or with the frontend.

**Worker replicas are no longer limited to one.** The notification and hook consumers read
through `XREADGROUP` with acknowledgement, so events are distributed across replicas and an
unacknowledged event is redelivered rather than lost with the process holding it. You can
scale `chat-worker` now. The scheduled dispatcher and reminder checker still claim their
rows in the database, which was already safe for multiple replicas.

Delivery to the worker is at-least-once as a result. Outgoing webhooks claim a
`(hook_id, event_id)` row before dispatching (migration `…22`), so a redelivery does not
call the same endpoint twice. If you see `Hook consumer: already dispatched, skipping
redelivery` in the logs, that is the guard doing its job, not an error.

**Nothing to migrate.** The streams start empty; there is no backfill and no dual-run
period. Rolling back means clients stop sending positions and delivery returns to
at-most-once — the streams are then trimmed away by age on their own.

## Upgrade note: Wave 8 turns on cleanup, SSO, TOTP and SCIM

**Retention starts deleting things you never configured — but only cleanup.** Expired
refresh tokens, consumed password-reset tokens, expired invites and `hook_executions` older
than 30 days are purged unconditionally on every pass. None of that is anybody's data.
Messages, files, notifications and audit rows are only touched where a workspace owner has
set a policy; a workspace with no `retention_policies` row keeps everything forever.

Set `RETENTION_DRY_RUN=true` for the first run on a real instance. It logs and counts what
each pass *would* delete and deletes nothing. Deletion is irreversible, and a misread policy
is visible in those numbers before it is visible in a support ticket. The metric is
`retention_rows_deleted_total{table}`; a purge is also written to the audit log.

**SSO is off until `OIDC_ISSUER` is set.** Endpoints are read from the provider's
`.well-known/openid-configuration`, so the settings are the issuer, the client id and the
secret. The redirect URI to register with the provider is
`<PUBLIC_URL>/api/auth/oidc/callback`. `OIDC_PROVISIONING` defaults to `invite_only`:
existing accounts may sign in through the provider, but nobody new is created. Set it to
`domain_allowlist` with `OIDC_ALLOWED_DOMAINS` if the provider is allowed to create
accounts, or `disabled` to turn the door off without removing the configuration.

Signing in through the provider **removes the password** from a non-admin account. That is
deliberate — an SSO account with a working password has a second way in that nobody is
watching — but it means people cannot fall back to a password if the provider is down.
Instance admins keep theirs on purpose: that is the break-glass account.

**`REQUIRE_ADMIN_TOTP=true` will lock out an admin who has not enrolled.** Turn it on only
after every instance admin has a factor set up (Profile → Two-factor authentication).
Enrolment is two steps and nothing is enforced until a code confirms it, so a half-finished
setup cannot lock anybody out. Recovery codes are shown once; the count remaining is
visible in the same panel.

**SCIM needs a token minted here first.** `POST /api/admin/scim/tokens` (instance admin)
returns it once — there is nowhere to read it again. Point the provider at
`<PUBLIC_URL>/api/scim/v2` with that token as a bearer credential. Rotation is
`POST /api/admin/scim/tokens/:id/rotate`, which issues the replacement and revokes the old
one in the same call.

Be aware of what deprovisioning does, because it is not reversible from the provider's side:
`active: false` (and `DELETE`, which means the same thing) suspends the account, ends every
session, and **removes every workspace and channel membership**. Setting `active: true`
again restores the account and *no* memberships — the person needs a fresh invite. That is
the CS-007 behaviour on purpose; an identity provider must not be able to hand somebody back
their old private channels by flipping a flag.

**Exports are single-use links.** A workspace export is the whole workspace in one file, so
the download token works once and expires. If somebody says the link is dead, the answer is
a new export, not a longer expiry.

**Nothing to migrate by hand.** Migrations `…23` through `…26` add the edit-history,
retention, export, TOTP and SCIM tables and one column on `oauth_accounts`. They are
additive; rolling back the binaries leaves the tables unused.

## Upgrade note: the dependency refresh changes no data, but rebuild everything

**Nothing to migrate, but the whole backend has to be rebuilt.** The Rust dependency set
moved forward across several majors — axum 0.8, sqlx 0.9, redis 1.x among them — and sqlx 0.9
needs rustc 1.94 or newer. `docker compose build api worker realtime` handles it; a partial
rebuild that leaves one service on the old image is fine on the wire, since nothing about the
HTTP or WebSocket protocol changed.

**One behavioural fix worth knowing about.** redis 1.x gives a connection manager a 500 ms
response timeout by default, which is shorter than the two-second blocking read the worker
uses to pull from the event streams. A read that times out on the client still counts as
delivered on the server, so those events landed in the consumer's pending list and were never
offered again — webhooks and notifications would have gone quietly missing under the new
client. The stream reader now sets its own response timeout past the block. If you are
running a build from between these releases, this is the symptom to look for.

## Upgrade note: Wave 9 rebuilds the search index and adds push

**The search migration does not lock the table, but the backfill takes a while.** Migrations
27 to 32 add a `search_vector` column, a trigger, and four indexes built `CONCURRENTLY`;
none of that holds an exclusive lock, so the instance keeps serving throughout. What they do
*not* do is fill in existing rows — the worker does that in committed batches of 5,000 with
a short pause between them, starting at boot and stopping when there is nothing left.

Watch `search_backfill_rows_total` or the `search backfill: N rows in messages` log line.
Until it finishes, older messages are found through the trigram index only, which means
substring matches work and exact word ranking is incomplete. On a few hundred thousand
messages this is minutes; the pause is deliberate, so that a backfill cannot saturate the
pool of an instance that is serving traffic.

If a concurrent index build is interrupted it leaves an `INVALID` index behind, which
Postgres will not use. Find them with:

```sql
SELECT indexrelid::regclass FROM pg_index WHERE NOT indisvalid;
```

Drop and recreate any that show up; nothing else needs doing.

**Search language is a migration, not a setting.** `search_text_config()` returns `simple`,
which does no stemming and folds diacritics through `unaccent`. Both the trigger that writes
the vector and the query that reads it call that one function, so they cannot disagree. To
stem for a single-language instance, write a migration that replaces the function and
rebuilds the vectors — there is no environment variable, on purpose.

**Web Push is off until you generate VAPID keys.** One pair per instance:

```
npx web-push generate-vapid-keys
```

Set `VAPID_PUBLIC_KEY`, `VAPID_PRIVATE_KEY` and `VAPID_SUBJECT` (a `mailto:` the push
service can use to reach you). **Rotating the pair silently invalidates every existing
subscription** — every browser has to be re-registered, and nobody is told — so treat it as
permanent. Both `api` and `worker` need the values: the API stores subscriptions, the worker
sends.

Nothing is pushed to somebody who has a live socket for that workspace, is inside their
do-not-disturb window, or has muted the channel. Subscriptions are pruned when the push
service answers `410 Gone`, which is the only reliable signal one is dead.

The service worker is served from `/sw.js` with `Cache-Control: no-cache`. Keep that: a
service worker cached for a year is a bug fix nobody receives.

**Slash commands call out synchronously, with a three-second timeout and no retries.** A
command that needs longer should acknowledge and post back through an incoming webhook.
Outbound targets are still refused if they resolve to a private, loopback or link-local
address. If your CI is on the same private network and you accept that anyone who can create
a hook then has the server's network position, set
`WEBHOOK_ALLOW_PRIVATE_TARGETS=true`.

**Nothing to migrate by hand.** Migrations 27 to 35 are additive apart from dropping the old
`content_search` column, which is a catalog change rather than a rewrite.

## Audit log

`GET /api/workspaces/:ws_id/audit-log` (workspace admin) and `GET /api/admin/audit-log`
(instance admin), both keyset-paginated on `(created_at, id)` via `before`/`before_id` and
filterable by `action`, `user_id`, `since` and `until`. The UI is under the workspace menu
and as a tab in the instance admin page.

`ip_address` is resolved with the same `TRUSTED_PROXIES` rules as the rate limit. Behind a
proxy that is not listed there, the column records the proxy's address rather than the
caller's — if the trail shows one repeated address, that is the setting to check.

The table has no retention policy yet (`CS-030`). On a busy instance it is the table that
grows without bound; watch it.

## Mail transport

SMTP transport security is `SMTP_TLS_MODE` (`starttls` / `implicit` / `none`). With it
unset, a local catcher gets `none` and any other host gets `starttls`. The API **refuses
to start** when the mode is `none`, the host is remote, and SMTP credentials are set —
that combination puts the relay password on the wire in clear. An open internal relay with
no credentials still starts, with a warning.

## Rate limits and upload size

Writes are limited per user, per class of action: messages 120/min, reactions 240/min,
invites 20/hour, workspaces 5/hour, channels 30/hour, everything else 120/min. Auth paths
(`/auth/login`, `/auth/forgot-password`) **fail closed** — if Redis is unreachable they
return 503 rather than verifying a password, so a Redis outage cannot silently remove
brute-force protection. Watch `rate_limit_backend_failures_total{policy="closed"}`: a
non-zero rate means logins are failing for infrastructure reasons, not credentials.

`MAX_UPLOAD_BYTES` (default 100 MiB) caps uploads and is enforced while streaming, so an
oversized file never lands in memory. `client_max_body_size` in `docker/nginx.conf` must
move with it — nginx rejects first, and a mismatch surfaces as a bare 413.

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
