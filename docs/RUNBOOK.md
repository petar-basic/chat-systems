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
| `worker` | Background consumers: outgoing webhooks, reminders, notifications, huddle history, call ringing, scheduled messages, email delivery, event outbox relay | yes |
| `realtime` | WebSocket gateway | yes |

Every consumer in `worker` either reads through a Redis Streams consumer group, which hands
each event to exactly one replica, or claims its row in the database first, so a second
replica is a second pair of hands rather than a second copy of every side effect. The
production compose runs two. If every replica is down, nothing is lost permanently —
messages still send and read — but webhooks, reminders, scheduled messages, call ringing
and notification rows wait until one returns; the streams hold what was published in the
meantime. Its `/readyz` is wired to the autoheal sidecar like the others.

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

`POST /api/channels/:id/messages` no longer accepts `id`. The server owns the message id; a
sender that wants an idempotent retry passes `client_message_id` instead, unique within the
channel. An old client that
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

**The notification and hook consumers stopped being the reason for one replica.** They read
through `XREADGROUP` with acknowledgement, so events are distributed across replicas and an
unacknowledged event is redelivered rather than lost with the process holding it: every
replica runs `XAUTOCLAIM` on each stream every 30 seconds and takes over anything that has
sat unacknowledged for 60 seconds, whoever was holding it. An event that has been delivered
five times without ever being acknowledged is assumed to be killing its consumer and is
acknowledged unprocessed — `dropping <stream> <id> after N deliveries` in the worker log is
that happening, and the event id in it is what to go looking for. The scheduled dispatcher
and reminder checker claim their rows in the database, which was already safe for multiple
replicas.

**At the time this shipped, two consumers were still on plain pub/sub** — the huddle
consumer and the call notification consumer — and that kept `chat-worker` at one replica.
Both have since moved to consumer groups; see the ring note under migration 42 below.

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
set a policy; a workspace with no `retention_policies` row keeps everything forever. A policy
covers direct and group messages too — they are messages in `dm`/`group_dm` channels, and a
retention rule that skipped them would keep exactly the history people assume is gone.

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

## Upgrade note: Wave 11 changes who guests can see and how sessions fail

**Guests see less, immediately and without a migration.** A guest now gets only the people
they share a channel with, and no email addresses at all. If you were relying on the member
list as a directory for guest accounts, that is gone on deploy — it was handing external
people your staff list. Members, admins and owners are unaffected.

A guest also cannot start a conversation with somebody they share no channel with.
Conversations that already exist keep working.

**Announcement channels.** `PATCH /api/channels/:id` accepts `post_policy`: `everyone` (the
default, and what every existing channel is) or `moderators`. Under `moderators` the
composer, thread replies, scheduled sends and `in_channel` slash commands are refused for
everybody but workspace admins and that channel's admins; reading and reacting are
unchanged, and an incoming webhook scoped to the channel still posts. Changes are audited as
`channel.post_policy_changed` with before and after.

**`ACCESS_TOKEN_EXPIRY` now defaults to 900, not 3600.** Clients refresh transparently, so
this is invisible in use — it is the window in which a revoked session would still work if
the revocation store could not be reached. Raising it back raises that window.

**A revocation lookup that fails now refuses the request.** Redis gets 250ms and one retry;
after that the API answers `503` and the realtime gateway refuses the socket, rather than
treating "cannot check" as "not revoked". Watch `auth_revocation_lookup_failures_total`: any
sustained non-zero rate means Redis is unhealthy *and* people are being turned away, and both
need looking at. This is the trade — a Redis outage now degrades sign-in instead of silently
disabling deprovisioning.

**Email leaves through the worker.** Invites, password resets and mention digests are
queued in `outbound_emails` by the api and delivered by `chat-worker`, so the worker needs
the same `SMTP_*` settings as the api — an api that can queue and a worker that cannot send
looks like "the invite never arrived". `outbound_emails` rows with `sent_at IS NULL` and a
`last_error` are the first thing to look at.

**Mention emails.** If SMTP is configured, somebody who is mentioned while offline, with no
push subscription, not muted and not in do-not-disturb, gets one digest email five minutes
later — cancelled if they come online first. It carries who and where and a link, never the
message text. Individuals can turn it off at `PATCH /api/notifications/email`; with SMTP
unconfigured the whole feature is off and logs nothing.

**Realtime events go through an outbox too.** Every durable event (messages, reactions,
membership, rings) is written to `event_outbox` inside the transaction that writes the row
it describes, published to Redis right after the commit, and marked `published_at`. If the
API crashed or Redis was down at that moment, the worker's relay publishes it within a few
seconds and the client sees it late rather than never. Rows are pruned a day after
publishing. A growing count here means Redis is unreachable from the API:

```sql
SELECT event_type, COUNT(*) FROM event_outbox WHERE published_at IS NULL GROUP BY 1;
```

**Invites and password resets go through an outbox.** Neither is sent inside the request
any more: the row lands in `outbound_emails` and the worker delivers it within a couple of
seconds, retrying with backoff (1, 4, 16, 64 minutes, then every four hours) up to eight
attempts. After the last one the row is parked with `next_attempt_at` null and the SMTP
error in `last_error`, which is what to look at when somebody says the invite never came:

```sql
SELECT to_address, subject, attempts, next_attempt_at, last_error
  FROM outbound_emails WHERE sent_at IS NULL ORDER BY created_at DESC;
```

Setting `next_attempt_at = NOW()` on a parked row puts it back in the queue once the SMTP
problem is fixed. With SMTP unconfigured nothing is queued and a warning is logged instead.

**`chat-worker` can now run more than one replica.** The huddle consumer reads through a
consumer group like the others, and the ring claims `(huddle_id, to_user_id)` before it
sends, so a second replica loses the race instead of ringing twice. The scheduled dispatcher
and reminder checker already claimed their rows. Nothing forces you to scale it — at a few
dozen people one replica is plenty — but it is no longer a correctness constraint.

**Migration 42 moves the ring itself onto the stream.** The ring was the last event still
on pub/sub, which is at-most-once: a worker that was restarting when somebody started a
call simply never rang anyone. It is now written to the workspace stream like every other
durable event and read by the notification consumer through its group, so a ring that
lands while every worker is down is delivered when one comes back — unless it is older than
sixty seconds by then, in which case it is dropped, because a call announced a minute late
is worse than one not announced. The `huddle_ring_claims` table is gone; a redelivery is
absorbed by `idx_notifications_call_dedup`, one call notification per person per call.
Reconnecting clients never see a ring in their replay.

**Migrations 39 and 40** add `pending_mention_emails`, `users.mention_emails` and
`huddle_ring_claims`. All additive.

## Upgrade note: the 2026-09 follow-ups fold direct messages into channels

Migrations 43–45 ship together. **Take a backup first** (see below): 45 is not reversible.

- **Migration 45 makes every conversation a channel** of type `dm` or `group_dm`, moves
  its messages, reactions, edit history, attachments, saved and scheduled rows onto the
  channel tables with their ids preserved, and drops the conversation tables. Old clients
  break: the `/conversations/…/messages` routes and the `conversation.message.*` frames are
  gone, so deploy api, worker, realtime and frontend in one go. Retention policies now cover
  direct messages, which they previously did not reach.
- **Migration 44 adds `event_outbox`** and migration 43 `outbound_emails`. Both are additive,
  but email is now delivered by `chat-worker`, so the worker needs the same `SMTP_*`
  settings as the api (the compose files carry them; a hand-written deployment must add
  them). The relay and the outbox prune themselves.
- **Every query is now a compile-time-checked macro.** A build from source needs either
  `DATABASE_URL` pointing at a migrated database or the committed `.sqlx/` cache with
  `SQLX_OFFLINE=true`; the Docker build uses the cache.

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

`WRITE_RATE_LIMIT_MULTIPLIER` scales all of the write budgets at once and is 1 everywhere
except the CI stack, where three Playwright workers act as a single admin account and would
otherwise trip the 120/min default class mid-suite — the symptom was a slash command or an
import that silently did nothing, with a 429 in the trace.

`MAX_UPLOAD_BYTES` (default 100 MiB) caps uploads and is enforced while streaming, so an
oversized file never lands in memory. `client_max_body_size` in `docker/nginx.conf` must
move with it — nginx rejects first, and a mismatch surfaces as a bare 413.

## Huddle relay (TURN)

Huddles are mesh WebRTC. STUN alone only finds a path when neither side sits behind a
symmetric NAT, which excludes most mobile carriers and anything doing CGNAT: two people on
different connections then join a room that looks healthy and carries no audio. The
`coturn` service is the relay that removes the dependency on what either side is behind.

It runs from `docker-compose.turn.yml`, on the host, next to the stack rather than inside
it. TURN needs the host network namespace — the browser reaches its relay ports directly —
and a Coolify compose resource cannot express that, because Coolify attaches its own
network to every service it manages. The relay shares nothing with the application anyway:
two configuration values are the entire contract between them.

The API serves STUN-only until **both** `TURN_SECRET` and `TURN_URLS` are set, so the
sequence matters — an advertised TURN server that is not answering is worse than none,
because the browser waits on it before giving up.

1. On the server, from a checkout of this repository:

   ```bash
   TURN_SECRET=$(openssl rand -hex 32) TURN_REALM=stage.example.com \
     docker compose -f docker-compose.turn.yml up -d
   ```

   Keep that secret: the api needs the same value. Leave `TURN_URLS` **unset** for now.
2. Open `3478/udp`, `3478/tcp` and `49160-49200/udp` on the host firewall. The relay range
   is bounded in `docker/coturn/turnserver.conf` so it can be opened explicitly.
3. Confirm the relay answers before anything depends on it. On
   [Trickle ICE](https://webrtc.github.io/samples/src/content/peerconnection/trickle-ice/),
   enter `turn:<host>:3478` with a username of `<unix-expiry>:<any-user-id>` and a
   credential of `base64(hmac_sha1(TURN_SECRET, username))`. A candidate of type `relay`
   means it works; only `srflx` means the port is closed or the secret does not match.
4. Now set `TURN_URLS=turn:<host>:3478` on the api and restart it. `GET
   /api/workspaces/{id}/ice-servers` should return two entries — the STUN one and a TURN
   one with credentials that expire after `TURN_TTL_SECS` (default 12h).

Credentials are minted per user and time-limited (TURN REST API); coturn validates them
against the same secret, which is why the two must match exactly. The relay refuses to
forward to private, loopback and link-local ranges, so it cannot be turned into an SSRF
primitive against the host it runs on.

## Importing a Slack export

Slack gives you a ZIP. **The ordinary way is the app**: as a workspace admin, workspace menu
→ *Slack import*, choose the zip, leave *Dry run* ticked, and read the report. Untick it and
upload again to do it for real. The upload is one request; the import itself runs in the
worker, and the panel shows the counts as they move.

The CLI below is for the two cases the browser cannot serve: an export larger than 512 MB, and
a migration you want to watch from a shell.

```bash
docker compose run --rm -e SERVICE=chat-import \
  -v /path/to/slack-export.zip:/import.zip api \
  --workspace <slug-or-uuid> --export /import.zip --dry-run
```

**A whole Slack workspace becomes a workspace here.** Tick *Import into a new workspace* and
the app suggests the name from the file — `Acme Slack export Jul 27 2026 - Aug 26 2026.zip`
becomes `Acme` — because that is the only place Slack writes it. Nothing inside the archive
names the workspace, manifest included.

**Read the notes, not only the counts.** Slack's ordinary export (`MANUAL_NON_COMPLIANCE`)
carries public channels and nothing else: no DMs, no private channels. An export whose
`channels.json` is present and empty is a workspace with no public channels, which looks
identical to a broken import until the report says which it was. Both are stated in the run.

**What comes across.** Public channels from `channels.json`, private ones from `groups.json`,
and direct and group messages from `dms.json` / `mpims.json` — the last two become `dm` and
`group_dm` channels, which the channel list never shows, so a two-person history does not land
in everybody's sidebar. Only some Slack plans export DMs at all; the report names every listing the
export did not carry.

**Always dry-run first.** It writes nothing and prints the counts the real run would produce,
plus every item that will not convert and why. Read that list before the real run: it is where
you find the accounts with no email address, the bots, and the replies whose parent was
deleted in Slack years ago.

Then the same command without `--dry-run`. It is safe to run twice — a second run finds what
the first wrote and skips it — which is also what makes it safe to interrupt. A large import
will take hours; running it under `tmux` or `nohup` costs nothing and saves the afternoon.

**Attachments and custom emoji need a token — put it in the environment, not the command.**
`SLACK_TOKEN=xoxb-…` rather than `--slack-token xoxb-…`: an argument is visible in `ps` to
every user on the host, and this one is a live credential with `files:read` and `emoji:read`.

Slack's file URLs are private and expire, and custom emoji are not in the export at all — the
import reads them from `emoji.list`. Without a token the messages still import; every file
and the emoji are named in the report as not fetched, and imported messages keep their
`:shortcodes:` as text. **Running the import again with a token picks the files up** without
duplicating anything. `--no-files` skips both deliberately, which is the faster way to
rehearse an import.

The token only ever goes to Slack's own hosts, and every URL from the export is checked
against the same guard outgoing webhooks use before anything is fetched.

**What people will see afterwards.** Imported messages keep their original timestamps, so
they appear in the channel's history where they belong rather than as a wall of new activity —
no unread counters move, and nothing is pushed to anybody's phone. Accounts created by the
import have no password: those people go through the normal invite flow, and until they do,
their history is attributed to an account only they can claim.

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
