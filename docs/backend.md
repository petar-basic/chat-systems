# Backend

Three processes over one codebase, sharing a Postgres database, a Redis instance and
the crates under `shared/`:

| Process | Port | Role | Replicas |
|---|---|---|---|
| **`chat-api`** | 3000 | Stateless REST API. Owns the migrations, applied at startup. | scale freely |
| **`chat-worker`** | 3005 | Background consumers: outgoing webhooks, reminders, notifications, huddle history, call ringing, scheduled messages, email delivery, event outbox relay. Serves only `/livez`, `/readyz`, `/metrics`. | scale freely |
| **`chat-realtime`** | 3004 | WebSocket gateway. | scale freely |

`chat-api` serves an OpenAPI document at `/api/openapi.json`, generated from the handlers
themselves, and the frontend's `src/api/schema.d.ts` is generated from that. Every feature
is on it except SCIM (its own standard, its own error format) and the OIDC redirects. The
route tables below are hand-maintained and kept only as a narrative overview; where the two
disagree, the document is right.

`chat-api` and `chat-worker` are two binaries over the same library crate
(`backend/api`), so they share `AppState`, config and every repo.

**Why the worker is separate.** The consumers used to be driven by Redis pub/sub, which
delivers a copy to *every* subscriber. Running them inside the API meant a second API
replica produced a second notification row, a second outgoing webhook POST and a second
reminder — so the API tier could not actually be replicated despite being stateless.
Moving them into one process fixed that. The worker itself scales because every
consumer now reads through a Redis Streams consumer group — each event reaches exactly
one replica — or claims its row with `FOR UPDATE SKIP LOCKED` first. Delivery is
at-least-once, so the side effects are idempotent on their own: partial unique indexes
make a duplicate mention or call notification impossible, and outgoing webhooks claim
`(hook_id, event_id)` before they POST.

## Architecture & Rationale

### Why this stack

- **Rust + Axum.** A chat backend is mostly fan-out and connection handling; Rust gives
  predictable latency and memory with no GC pauses, and Axum is a thin, `tower`-based
  layer over `hyper` that composes middleware cleanly.
- **Two binaries, not one.** The REST API is request/response and **stateless**; the
  WebSocket gateway is long-lived and **connection-stateful**. Splitting them lets each
  scale and fail independently — you can run many api replicas and many realtime nodes.
- **Redis as the bus, Postgres as the ledger.** The api never talks to sockets directly.
  A durable event (message, reaction, membership, ring) is written to `event_outbox` in
  the same transaction as the row it describes, then appended to the workspace's Redis
  Stream (`stream:ws:{id}`) and published live; a relay in the worker re-sends anything
  the fast path missed. Every realtime node reads the live channel and pushes to *its*
  connected clients, and a reconnecting client replays the stream from its cursor. The
  worker's consumers read the streams through consumer groups, so each event reaches
  exactly one replica. Redis also backs presence (TTL-keyed per node, self-healing) and
  rate-limit counters.
- **PostgreSQL + sqlx.** Every query is a `sqlx::query!` macro checked against the schema
  at compile time (the committed `.sqlx/` cache makes that work without a database, which
  is how the image builds); fully parameterized, soft deletes, partial indexes, a GIN
  full-text index plus trigram fallback for search. Migrations run automatically on api
  startup (`sqlx::migrate!`).
- **Boring libraries for boring problems.** `object_store` for local disk and S3, `garde`
  for request validation, `askama` for email templates, `utoipa` for the OpenAPI document,
  `ts-rs` for the WebSocket frame types, `ipnet` for proxy trust, `backon` for retries.
  Each replaced a hand-rolled version that had accumulated its own bugs.

### Feature-modular layering

Every feature under `api/src/` (`auth`, `workspace`, `conversations`, `messaging`, `files`,
`hooks`, `notifications`, `scheduled`, `saved`, `groups`, `emoji`, `retention`, `export`,
`slack_import`, `scim`, `push`, `commands`, `huddle`, `admin`) follows the same shape, with
files added only where warranted:

| File | Responsibility |
|------|----------------|
| `routes.rs` | parse request, authorize the caller, delegate — no business logic, no SQL |
| `service.rs` | business logic / orchestration across repos |
| `repo.rs` | **all** SQL for the feature; the only place that touches the pool |
| `models.rs` | request/response and row types |
| `publisher.rs` / `consumer.rs` / `executor.rs` | Redis publish / background consumers / outbound execution |
| `storage.rs` | the `object_store`-backed file store (files) |

Rules that keep it from rotting: routes never write SQL, a feature never reaches into
another feature's repo, and `AppState` (`state.rs`) is the single composition root wired
in `main.rs`.

### Cross-cutting concerns

- **Auth.** Argon2id password hashing; HS256 JWTs with an `access` / `refresh` token-type
  claim. Refresh tokens are DB-backed, single-use, and rotated; password reset is single-use
  and revokes all sessions. Auth cookies are `HttpOnly; SameSite=Lax`, and `Secure` whenever
  `PUBLIC_URL` is https.
- **Authorization** is re-derived per request from the verified token (`auth.user_id`),
  never trusted from the body — and re-checked against the DB on every WebSocket
  subscribe/join.
- **Rate limiting.** A shared Redis fixed-window limiter (`rate_limit.rs`) guards auth
  endpoints (per-email and per-IP on login, per-email on forgot-password) and incoming
  webhooks (per-token). A per-user `write_rate_limit` middleware caps write methods on the
  messaging and files routers (120 writes / 60s per user).
- **Error handling.** `AppError` (`shared/common`) maps to status codes; 500-class errors
  log detail but return an opaque body so internals/SQL never leak (with tests proving it).
- **Files.** `FileStorage` wraps an `object_store` backend (local disk or S3/MinIO) and
  streams multipart uploads; downloads go through the authenticated `/api/files/download`
  route, so the object store stays private and access is gated by channel membership —
  which covers direct messages, since a DM is a channel.
- **Validation.** Request DTOs derive `garde::Validate` with the rule declared on the
  field; a handler calls `req.validate()?` and a violation is a 422 with the field named.
- **Email.** Invites, resets and mention digests are queued in `outbound_emails` inside
  the request and delivered by the worker with retries and backoff; templates are `askama`.
- **The API contract is generated.** Handlers carry `#[utoipa::path]`, the document is
  served at `/api/openapi.json`, and the frontend's types and WebSocket frame union are
  generated from the backend; CI fails when either is stale.
- **Webhooks.** Outbound delivery is SSRF-hardened (scheme allow-list, DNS resolution with
  private/loopback/link-local/metadata-IP blocking, redirects disabled) and HMAC-signed.

### Testing

Integration tests live in `api/src/http_tests/` and `realtime/src/tests/`. Each
`#[test_macros::db_test]` gives its test a database nobody else is touching and drives the
full Axum router via `tower::oneshot` — real middleware, real auth, real JSON — asserting
the authorization matrix per endpoint. Realtime tests use real Redis + Postgres and assert
the right frames reach the right subscribers.

**Where the databases come from.** Running all 45 migrations per test cost about 0.4s and
there are several hundred tests. The first test in a process builds a *template* database
instead — migrations once, behind an advisory lock so parallel test binaries do not race —
and every test after it clones the template with `CREATE DATABASE ... TEMPLATE`, which costs
about 0.1s. The template is named after a fingerprint of the migration versions and
checksums, so editing or adding a migration builds a new one rather than testing yesterday's
schema. A test that panics leaves its database behind; the next run drops anything older
than an hour.

---

## chat-api (REST API)

All routes below are nested under the **`/api`** prefix (e.g. `POST /api/auth/login`);
the tables omit it for brevity. Only the health probes (`/livez`, `/readyz`) and
`/metrics` live at the root, outside `/api`.

On success, login / complete-registration / refresh all return the same `AuthSession`
shape: `{ user: UserPublic, expires_in, access_token, refresh_token }`. They also set
the `access_token` / `refresh_token` cookies (see Cross-cutting concerns).

### auth

Handles user identity — login, registration, JWT, and profile.

**Input / Output:**

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| POST | `/auth/login` | `{ email, password }` | `AuthSession` |
| GET | `/auth/invites/:token/verify` | — | `{ email, workspace_name, workspace_id }` |
| POST | `/auth/complete-registration` | `{ token, password, display_name }` | `AuthSession` |
| POST | `/auth/refresh` | `{ refresh_token }` (or refresh cookie / Bearer) | `AuthSession` |
| POST | `/auth/logout` | refresh cookie | `{ status: "logged_out" }` |
| POST | `/auth/forgot-password` | `{ email }` | `{ status: "sent" }` |
| POST | `/auth/reset-password` | `{ token, password }` | `{ status: "reset" }` |
| GET | `/instance/info` | — | `{ name, icon_url }` |
| GET | `/users/me` | JWT | `UserPublic` |
| PATCH | `/users/me` | `{ display_name?, avatar_url?, bio?, timezone? }` | `UserPublic` |
| PATCH | `/users/me/password` | `{ current_password, new_password }` | `{ status: "password_changed" }` |
| PUT | `/users/me/status` | `{ emoji?, text?, expires_at? }` | `UserPublic` — a custom status; needs an emoji or some text, and an expiry in the future. An expired status stops being returned by every read; the row itself is only overwritten the next time the person sets one |
| DELETE | `/users/me/status` | — | `UserPublic` |

---

### workspace

Manages workspaces, members, invites, and channels. (Direct messages are their own
feature — see `dm` below.)

**Role model** (`Guest` 10 < `Member` 20 < `Admin` 40 < `Owner` 50):

| Action | Required |
|--------|----------|
| Read channels/messages, post, react, thread | Member of the workspace; **Guest** additionally must be a member of that channel (Guests never get implicit access to public channels) |
| Create a channel | `Member` |
| Browse public channels and self-join one | `Member` — Guests are refused, and private/archived channels are never listed or joinable |
| Rename/archive a channel, change a channel member's role, remove someone from a channel | **Channel admin** (`channel_members.role = 'admin'`) or workspace `Admin`; a Guest never moderates, even holding the channel-admin row |
| Add someone to a channel | `Member` of the workspace, and — for a private channel — a member of that channel; the person being added must already be in the workspace |
| Manage invites and workspace settings | `Admin` |
| Change a member's role | `Admin`, and the actor must strictly outrank the target and may not grant a role above their own; the Owner cannot be demoted |
| Remove a member | `Admin` and strictly outranking the target, or the member removing themselves; the Owner cannot be removed |
| Soft/hard delete or restore a workspace | `Owner` (or instance admin) |

**Workspaces:**

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/workspaces` | — | `{ data: Workspace[] }` |
| POST | `/workspaces` | `{ name, description? }` | `Workspace` |
| GET | `/workspaces/:ws_id` | — | `Workspace` |
| PATCH | `/workspaces/:ws_id` | `{ name?, description?, icon_url? }` | `Workspace` |
| DELETE | `/workspaces/:ws_id` | Query: `hard=bool` | `{ status: "soft_deleted" \| "hard_deleted" }` |
| POST | `/workspaces/:ws_id/restore` | — | `Workspace` |
| GET | `/workspaces/deleted` | — | `{ data: Workspace[] }` |

**Members:**

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/workspaces/:ws_id/members` | — | `{ data: MemberWithUser[] }` |
| PATCH | `/workspaces/:ws_id/members/:user_id/role` | `{ role }` | `WorkspaceMember` |
| DELETE | `/workspaces/:ws_id/members/:user_id` | — | `{ status: "removed" }` |

**Invites:**

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/workspaces/:ws_id/invites` | — | `{ data: WorkspaceInvite[] }` |
| POST | `/workspaces/:ws_id/invites` | `{ email?, role? }` | `WorkspaceInvite` |
| DELETE | `/workspaces/:ws_id/invites/:invite_id` | — | `{ status: "revoked" }` |
| POST | `/invites/:token/accept` | — | `WorkspaceMember` |

**Channels:**

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/workspaces/:ws_id/channels` | — | `{ data: Channel[] }` (each augmented with `muted`) |
| GET | `/workspaces/:ws_id/channels/unread` | — | `{ channel_ids: string[] }` |
| GET | `/workspaces/:ws_id/channels/browse` | — | `{ data: BrowsableChannel[] }` — every public, unarchived channel with `member_count` and `is_member` (Members and up; Guests are refused) |
| POST | `/workspaces/:ws_id/channels` | `{ name, channel_type?, description?, is_default? }` | `Channel` |
| GET | `/channels/:ch_id` | — | `Channel` |
| PATCH | `/channels/:ch_id` | `{ name?, topic?, description? }` | `Channel` |
| DELETE | `/channels/:ch_id` | — | `{ status: "archived" }` |
| PATCH | `/channels/:ch_id/notifications` | `{ muted }` | `{ muted: bool }` |

**Channel members:**

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/channels/:ch_id/members` | — | `{ data: ChannelMember[] }` |
| POST | `/channels/:ch_id/members` | `{ user_id }` | `ChannelMember` |
| POST | `/channels/:ch_id/join` | — | `ChannelMember` — self-join, public and unarchived channels only; idempotent |
| PATCH | `/channels/:ch_id/members/:user_id/role` | `{ role: "member" \| "admin" }` | `ChannelMember` — channel moderators only |
| DELETE | `/channels/:ch_id/members/:user_id` | — | `{ status: "removed" }` |

**Channel bookmarks** (the link bar under the channel header):

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/channels/:ch_id/bookmarks` | — | `{ data: ChannelBookmark[] }` — any channel member |
| POST | `/channels/:ch_id/bookmarks` | `{ label, url, emoji? }` | `ChannelBookmark` — channel moderators only; the URL must be `http(s)` |
| DELETE | `/channels/:ch_id/bookmarks/:bookmark_id` | — | `{ status: "deleted" }` — channel moderators only |

---

### conversations

A direct message is a channel of type `dm` (two people) or `group_dm` (up to nine) that nobody
can browse or join: participants are its `channel_members`, its messages are `messages`, and
reactions, edit history, threads, pins, attachments, saved and scheduled messages are the
channel ones. These routes only find and open conversations; everything else goes through the
channel and message routes with the conversation id as `ch_id`. Every message route already
requires membership of a non-public channel, so a conversation is readable by its participants
and nobody else. `GET /workspaces/:ws_id/channels` leaves dm channels out; the unread endpoint
counts them.

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/workspaces/:ws_id/conversations` | — | `{ data: ConversationSummary[] }` — newest message first, with `participant_ids`, `last_message_at` and the caller's `last_read_at` |
| POST | `/workspaces/:ws_id/conversations` | `{ participant_ids }` | `Conversation` — one other person returns the existing `direct` thread if there is one; more create a `group` |
| POST | `/conversations/:conv_id/read` | — | `{ status: "ok" }` — clears the caller's unread and mention counters for it |

Creating one publishes `conversation.created`, which the gateway delivers to every participant
so the new thread shows up for the other side; messages in it are ordinary `message.*` and
`reaction.*` events on that channel id, delivered to the sockets that joined it.

---

### scheduled

Messages queued for later delivery, aimed at one channel — a conversation id is a channel id
here. The author must be able to post to the target at scheduling time, and the send window is
capped at 120 days out.

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/workspaces/:ws_id/scheduled-messages` | — | `{ data: ScheduledMessage[] }` — the caller's pending queue |
| POST | `/workspaces/:ws_id/scheduled-messages` | `{ channel_id, content, send_at }` | `ScheduledMessage` |
| PATCH | `/scheduled-messages/:id` | `{ send_at }` | `ScheduledMessage` — author only, pending only |
| DELETE | `/scheduled-messages/:id` | — | `{ status: "canceled" }` — author only, pending only |

A background dispatcher ticks every 15s and claims due rows with
`UPDATE … WHERE id IN (SELECT … FOR UPDATE SKIP LOCKED) RETURNING *`, so several api replicas
can run it without delivering a message twice. Delivery reuses the normal send path — mentions
are expanded and `message.created` is published — and a failure is recorded on the row instead
of retried.

---

### messaging

Sends and manages messages, threads, reactions, pins, read tracking, and search.

**Messages:**

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/channels/:ch_id/messages` | Query: `limit=50, cursor?` | `{ data: Message[] }` |
| POST | `/channels/:ch_id/messages` | `{ content, thread_parent_id?, id? }` | `Message` |
| PATCH | `/messages/:msg_id` | `{ content }` | `Message` |
| DELETE | `/messages/:msg_id` | — | `{ status: "deleted" }` |

**Threads:**

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/messages/:msg_id/thread` | Query: `limit=50, offset=0` | `{ data: Message[] }` |
| POST | `/messages/:msg_id/thread` | `{ content }` | `Message` |

**Mentions.** Sending a channel message or thread reply expands the mentions in its body
into the `mentioned_user_ids` carried by `message.created`, which the notifications
consumer turns into per-user notifications (still subject to that user's channel mute and
DND). A mention is either a picked token (`@[Label](uuid)`) or one of the broadcasts —
`@channel` / `@everyone` (every channel member) and `@here` (channel members with a live
`presence:*` key in Redis). Broadcasts count whether the author picked them from the
composer (`@[channel](channel)`) or simply typed the word; the author is never notified of
their own message, and a named user inside a broadcast is notified once, not twice.

**Reactions:**

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/messages/:msg_id/reactions` | — | `{ data: Reaction[] }` |
| POST | `/messages/:msg_id/reactions` | `{ emoji }` | `Reaction` |
| DELETE | `/messages/:msg_id/reactions/:emoji` | — | `{ status: "removed" }` |

**Pins:**

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/channels/:ch_id/pins` | — | `{ data: Message[] }` |
| POST | `/messages/:msg_id/pin` | — | `{ status: "pinned" }` |
| DELETE | `/messages/:msg_id/pin` | — | `{ status: "unpinned" }` |

**Read tracking & search:**

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| POST | `/channels/:ch_id/read` | `{ message_id }` | `{ status: "read" }` |
| GET | `/search` | Query: `q, workspace_id, channel_id?, user_id?, limit=20, offset=0` | `{ data: Message[] }` |

Search is access-scoped: results come only from public channels in the workspace
plus the private/DM channels the caller belongs to.

---

### files

File upload and download. Supports both local storage and S3/MinIO.

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| POST | `/files/upload/:ws_id` | Multipart form (files) | `FileUploadResponse[]` |
| GET | `/files/:file_id` | — | `{ file: FileRecord, url: string }` |
| GET | `/files/download/*key` | — | Binary file stream |
| DELETE | `/files/:file_id` | — | `{ status: "deleted" }` |
| GET | `/files/workspace/:ws_id` | Query: `limit=50, offset=0` | `{ data: FileRecord[] }` |

---

### hooks

Incoming/outgoing webhooks, bots, slash commands, and reminders. Background task checks reminders on a schedule and executes hooks from Redis events.

**Hooks:**

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/workspaces/:ws_id/hooks` | — | `{ data: Hook[] }` |
| POST | `/workspaces/:ws_id/hooks` | `{ hook_type, name, description?, config? }` | `Hook` |
| GET | `/hooks/:hook_id` | — | `Hook` |
| POST | `/hooks/:hook_id/reveal` | — | `{ hook_id, hook_type, config, incoming_url }` — config unredacted, written to `audit_log` |
| POST | `/hooks/:hook_id/rotate` | — | same shape, after minting a fresh `token` (incoming) or `secret` (outgoing); the previous value stops working immediately |
| DELETE | `/hooks/:hook_id` | — | `{ status: "deleted" }` |
| POST | `/hooks/incoming/:token` | `{ text }` | `{ status: "ok", message_id }` |

Hook types: `incoming_webhook`, `outgoing_webhook`, `bot`, `slash_command`, `scheduled`

Built-in commands run before any registered one and never leave the instance: `/dnd`,
`/topic`, `/invite`, `/shrug` and `/remind`. `/remind` understands `me in 30m to ship`,
`me at 15:00 to call back` and `me tomorrow at 9am standup`; clock times are resolved in the
caller's own timezone and a time already past means tomorrow. Reminding somebody else needs
workspace admin, the same rule `POST /workspaces/:ws_id/reminders` applies.

`GET`/`POST`/`DELETE` on `/hooks` and `/workspaces/:ws_id/hooks` require a workspace
admin session; secrets in `config` (`token`, `secret`, …) are redacted on read — `reveal`
is the only way back to the plaintext value, and it leaves an audit trail. Creating an
`outgoing_webhook` requires an http(s) `config.url` and mints `config.secret` when omitted
(the delivery path still runs the full SSRF check per request).
Creating an `incoming_webhook` requires `config.channel_id`, and the server mints a
`config.token`. **`POST /hooks/incoming/:token` is authenticated by that URL token,
not a session** (Slack-compatible `{ "text": ... }`); it posts to the bound channel
as the hook's creator and is rate-limited per token.

**Reminders:**

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/workspaces/:ws_id/reminders` | — | `{ data: Reminder[] }` |
| POST | `/workspaces/:ws_id/reminders` | `{ target_user_id, content, remind_at, channel_id?, message_id? }` | `Reminder` |
| DELETE | `/workspaces/:ws_id/reminders/:reminder_id` | — | `{ status: "deleted" }` — only the person the reminder is for |

---

### notifications

In-app notifications for mentions, DMs, replies, reactions, calls, reminders, and system events. Created by a background consumer listening on Redis.

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/workspaces/:ws_id/notifications` | Query: `limit=50, offset=0` | `{ data: Notification[] }` |
| POST | `/notifications/read` | `{ notification_ids: string[] }` | `{ updated: number }` |
| POST | `/workspaces/:ws_id/notifications/read-all` | — | `{ updated: number }` |
| POST | `/workspaces/:ws_id/channels/:ch_id/notifications/read` | — | `{ updated: number }` |
| GET | `/workspaces/:ws_id/notifications/unread-count` | — | `{ unread_count: number }` |
| GET | `/notifications/dnd` | — | `{ dnd_until: timestamp \| null }` |
| PATCH | `/notifications/dnd` | `{ dnd_until: timestamp \| null }` | `{ dnd_until: timestamp \| null }` |

All workspace-scoped routes require workspace membership.

---

### slack_import

Reads a Slack export into a workspace, from the app or from a shell.

**From the app** — a workspace admin uploads the zip and watches the run. The upload is one
request; the import is not. It is queued, claimed by the worker the way an export job is, and
writes its counters as it goes so a run that takes an hour has something to show.

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| POST | `/workspaces/{ws_id}/slack-imports` | multipart: `archive` (the zip), `dry_run` | `ImportRun` — status `pending`; workspace admins only |
| POST | `/slack-imports` | multipart: `archive`, `workspace_name`, `dry_run` | Creates the workspace, then queues — an export *is* a workspace, and there is nothing to import into yet. The name is the caller's: Slack writes it into the file name and nowhere inside the archive, not even the manifest |
| GET | `/workspaces/{ws_id}/slack-imports` | — | `{ data: ImportRun[] }` — the last 20 runs |
| GET | `/slack-imports/{import_id}` | — | `ImportRun` — status, live counts, and what was skipped |

The archive is checked for a zip's magic bytes before anything is stored, capped at 512 MB
(past that the answer is the CLI, and the error says so), kept in the same storage uploads
use, and **deleted once the worker has read it** — it is a copy of somebody's entire Slack
history and there is no reason to keep it.

**From a shell** — for an export too large to travel through a browser, or a migration
somebody wants to watch over ssh:

```
chat-import --workspace <uuid|slug> --export <zip-or-directory> [--dry-run] [--no-files] [--slack-token <token>]
```

**All four listings, and it says which ones were there.** `channels.json` becomes public
channels, `groups.json` private ones, and `dms.json` / `mpims.json` become `dm` and
`group_dm` channels, which the channel list never shows, so a two-person history stays out
of everybody's sidebar. A listing the export does not carry is named in the report rather
than assumed empty: an export without private channels and an export whose private channels
were quietly dropped should not look the same.

**Two passes, because threads are what break single-pass importers.** The first creates
users, channels, conversations and memberships; the second writes messages, so a `thread_ts`
resolves against a row that already exists. A reply whose parent is missing from the export
stays in the channel as an ordinary message rather than being dropped, and says so in the
report.

**Users are matched by email and by nothing else.** A Slack handle is not an identity.
Somebody with no matching account is created with no password, so their history is
attributed to an account only they can claim through the ordinary invite flow; a bot, or an
account the export carries no address for, is reported rather than guessed at.

**Idempotent by construction.** `messages.slack_ts` carries the Slack timestamp the row came
from, unique per channel, and `slack_users` / `slack_channels` record what a previous run
mapped. A re-run finds them and moves on, which is also what makes an interrupted import
resumable — a 200k-message import will be interrupted.

**`--dry-run` writes nothing**, not even the run record, and reports the same counts the real
run would produce, including which items would not convert.

**An attachment becomes its own message.** This product's attachment form is a message whose
whole body is `[file: name](url)`, so a Slack message carrying both text and a file arrives as
two: the text, then the attachment. That is what makes it render and what puts it under the
same access rules as a native upload, at the cost of one extra row.

**Files are fetched, not linked.** Slack's URLs expire, so the bytes are downloaded during
the import and stored through `FileStorage`. Every fetch goes through the same SSRF guard the
webhook path uses, the token is attached only for Slack's own hosts — an export names those
URLs, and it is a file that arrives from outside — and the response is read with the size cap
applied as it streams rather than after it has all arrived; the message that carries one is written in the
composer's own attachment form, which is what makes CS-009's access rules apply to it. A file
that cannot be fetched is named in the report with the reason — deleted in Slack, hosted
outside it, no token, or a URL the guard refused — and the message it belonged to is still
imported. Running the import again with a token picks up the files a tokenless run left
behind, without duplicating the messages.

**The archive is checked against its own manifest.** `.slack-manifest.json` states a file
count and a total size; both are verified before the import starts, which catches a truncated
download before an hour of work goes into it. Its checksum is an undocumented aggregate
(`sha256-agg-v2`), so that is reported rather than pretended to be verified. The report also
carries what the manifest says the export *is* — a `MANUAL_NON_COMPLIANCE` export has public
channels only, which is worth saying before somebody concludes the import lost their DMs.

**Notes, not just failures.** The report separates what could not be imported from what is
merely worth knowing: a listing that is present but empty (different from absent), the kind of
export it was, and an account created under Slack's placeholder address for a deactivated
member — which needs to exist so their history has an owner, but which nobody will ever be
able to claim.

**Custom emoji are not in the export.** Slack keeps them behind `emoji.list`, so the import
reads them with the same token it uses for files (`emoji:read`) — or from an `emoji.json` in
the export, which some tools write, and which wins when it is there. Aliases point at the
image they alias rather than downloading it twice, and a name our own rules reject (uppercase,
too long, or one that shadows a standard emoji) is reported instead of being mangled into
something else. Without a token the run says so, rather than leaving somebody to discover it
from a message full of `:shortcodes:`.

**What it does not carry over.** Message `attachments` and Block Kit `blocks` (the `text`
fallback is imported instead), per-person starred items, edit history,
and anything an integration posted — a bot has no account here, and inventing one would put a
stranger in the member list. Each of those is counted in the report rather than passed over
in silence.

| Table | What it holds |
|---|---|
| `slack_imports` | One row per run: source, dry-run flag, status, the report as JSON, and the error if it failed |
| `slack_users` | `(workspace, slack user id)` → our user |
| `slack_channels` | `(workspace, slack channel id)` → our channel, DMs included |
| `messages.slack_ts` | The Slack timestamp a message came from; unique per channel |

---

### saved

One person's own list of kept messages. A row points at one message, and saving is
idempotent: saving something twice returns the row that already exists rather than a second
one. Reading a message is what entitles you to save it — the same channel access check the
message routes use, which covers direct messages too.

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/workspaces/:ws_id/saved` | — | `{ data: SavedMessageDetail[] }` — newest first, joined with the message so the panel renders in one round trip |
| POST | `/workspaces/:ws_id/saved` | `{ message_id, note? }` | `SavedMessage` |
| DELETE | `/saved/:id` | — | `{ status: "removed" }` — owner only |

---

### admin

Instance-level administration. Requires `is_instance_admin = true`.

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/admin/health` | — | `{ status, service, version }` |
| GET | `/admin/stats` | — | `{ users, workspaces, messages, files }` |
| GET | `/admin/users` | Query: `limit?, offset?` | `{ data: User[] }` |
| POST | `/admin/users/:user_id/suspend` | — | `{ status: "suspended" }` |
| POST | `/admin/users/:user_id/activate` | — | `{ status: "activated" }` |
| PATCH | `/admin/users/:user_id/instance-role` | `{ is_instance_admin: bool }` | `{ is_instance_admin: bool }` |
| GET | `/admin/workspaces` | Query: `limit?, offset?` | `{ data: Workspace[] }` |
| DELETE | `/admin/workspaces/:ws_id` | — | `{ status: "deleted" }` |

`suspend` revokes the user immediately: it deletes their refresh tokens, sets a
Redis revocation flag the auth middleware checks (so an unexpired access token is
rejected), and closes their live WebSockets; `activate` clears the flag. Workspace
delete is a soft-delete (reversible via restore), audited in the same transaction.

---

### huddle

Live voice/video rooms (Slack-style huddles) over mesh WebRTC. Live membership and media signaling run over `chat-realtime`; this REST surface covers what the browser needs from the API.

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/workspaces/:ws_id/ice-servers` | — | `{ ice_servers: IceServer[], ttl: number }` |
| GET | `/workspaces/:ws_id/active-huddles` | — | `{ data: { huddle_id, channel_id, initiator_id }[] }` |
| POST | `/workspaces/:ws_id/huddles` | `{ channel_id }` XOR `{ dm_partner_id }` | `{ huddle_id }` |
| POST | `/workspaces/:ws_id/huddles/:huddle_id/invite` | `{ user_ids: string[] }` | `{ status: "ok" }` |

`IceServer` is the WebRTC `RTCIceServer` shape: `{ urls: string[], username?, credential? }`. STUN entries are always returned; a TURN entry with time-limited credentials (TURN REST API, `username = "<expiry-unix>:<user-id>"`, `credential = base64(hmac_sha1(TURN_SECRET, username))`) is added only when `TURN_SECRET` and `TURN_URLS` are configured. See the coturn service in `docker-compose.yml` and the TURN section of `.env.example`.

**Start** generates a `huddle_id`, persists a `huddle_sessions` row, publishes `huddle.started`, and (for channels) posts a `metadata.kind="huddle_started"` system message; DM huddles also publish `huddle.ring` to the partner. **Invite** publishes `huddle.ring` to each workspace-member invitee. **Active-huddles** returns currently-live channel huddles — open DB sessions (`ended_at IS NULL`) intersected with live Redis room membership (`SCARD huddle:{id}:members > 0`), so abrupt-drop sessions that never emitted `huddle.ended` are excluded. The frontend fetches it on workspace load and on WS reconnect to backfill the channel huddle banner (so late-joiners see "Join" and stale banners self-heal). Live membership/media is ephemeral — see the `events:huddle` WS surface below. Session/participant history is persisted by the API's huddle consumer, which also emits `huddle.ended` when the last participant leaves; ring/invite also raise a `Call` notification (DND-respecting).

---

## chat-realtime (WebSocket Gateway)

Single WebSocket endpoint. Validates the JWT on the upgrade handshake, re-checks channel/workspace membership against the DB on every subscribe/join, then relays events from Redis to connected clients: the live pub/sub channels for latency, and the per-workspace stream for replay after a reconnect. Every frame it sends is a `ServerFrame` variant (`shared/events`), and the frontend's frame union is generated from that enum. The socket is also closed when the access token's `exp` passes, so a long-lived connection can't outlive its token.

The upgrade additionally checks the `Origin` against the configured CORS origins and rejects a token covered by a revocation record. A revocation stores the moment it happened rather than a boolean, so it invalidates every token issued up to that point while leaving a later sign-in working, and it can spare one named session (`except_jti`) — which is what lets "log out my other devices" keep the device you are typing on. `session.revoked` closes the matching live sockets with close code `4001` and a reason; suspending a user closes all of theirs. Alongside `/ws`, the gateway serves `/livez`, `/readyz` (DB + Redis + event-consumer liveness — it reports unhealthy if the consumer has stalled), and `/metrics` (Prometheus: connection count, consumer heartbeat age, events, backpressure drops). Presence is workspace-scoped: a client only sees the online roster and status changes for workspaces it shares. See [RUNBOOK.md](RUNBOOK.md) for ops.

**Connection:** `wss://<host>/ws` — the browser sends the `access_token` `HttpOnly` cookie on the upgrade automatically (no token in the URL).

**Incoming client messages (subscribe / join / typing):**

```json
{ "type": "subscribe",     "workspace_id": "..." }
{ "type": "channel.join",  "channel_ids": ["...", "..."] }
{ "type": "channel.leave", "channel_id": "..." }
{ "type": "typing.start",  "channel_id": "..." }
{ "type": "typing.stop",   "channel_id": "..." }
{ "type": "ping" }
```

**`channel.join` takes the whole set at once.** A client joins every channel it can see the
moment it connects, and one frame per channel is what the inbound flood guard (20/s, burst 40)
exists to stop — a workspace with more channels than that used to have its socket closed by
the act of arriving. One frame, one membership query for the set, capped at 1 000 ids so the
batch cannot become the flood it replaced.

**Huddle signaling (incoming client messages).** Mesh WebRTC uses this socket purely as the signaling channel — no media flows through the server. After membership is verified, the server relays via `events:huddle`:

```json
{ "type": "huddle.join", "huddle_id": "...", "channel_id": "..." }   // or workspace_id + dm_partner_id
{ "type": "huddle.leave", "huddle_id": "..." }
{ "type": "huddle.offer",  "huddle_id": "...", "to_user_id": "...", "sdp": { ... } }
{ "type": "huddle.answer", "huddle_id": "...", "to_user_id": "...", "sdp": { ... } }
{ "type": "huddle.ice",    "huddle_id": "...", "to_user_id": "...", "candidate": { ... } }
{ "type": "huddle.mute",   "huddle_id": "...", "audio_muted": true }
{ "type": "huddle.camera", "huddle_id": "...", "camera_on": true }
{ "type": "huddle.screenshare", "huddle_id": "...", "sharing": true }
{ "type": "huddle.hand",   "huddle_id": "...", "raised": true }
{ "type": "huddle.reaction", "huddle_id": "...", "emoji": "👍" }
```

`offer`/`answer`/`ice` are relayed only to `to_user_id` (and only when both users are current room members); the rest broadcast to the room. On join the caller gets a `huddle.members` snapshot. Disconnect removes the user and, when a room empties, the API consumer emits `huddle.ended`.

**Outgoing events pushed to client** (live copies arrive on these pub/sub channels; the durable ones are also on `stream:ws:{id}` for replay):

| Redis channel | Event types |
|---------------|-------------|
| `events:message` | `message.created`, `message.updated`, `message.deleted`, `message.pinned` |
| `events:reaction` | `reaction.added`, `reaction.removed` |
| `events:notification` | `notification.push` |
| `events:conversation` | `conversation.created` — fanned out to the participants; a conversation's messages are ordinary `message.*` / `reaction.*` events on its channel id |
| `events:presence` | `presence.changed` (fanned out only to workspaces shared with the subject) |
| `events:typing` | `typing.indicator` |
| `events:workspace` | `workspace.deleted`, `workspace.restored` |
| `events:user` | `user.suspended` — consumed by the gateway to close the user's sockets (not pushed to clients) |
| `events:huddle` | `huddle.started`, `huddle.ended`, `huddle.ring`, `huddle.member_joined`, `huddle.member_left` — lifecycle; also consumed by the API for history + call notifications |
| `events:huddle-signal` | `huddle.offer`, `huddle.answer`, `huddle.ice`, `huddle.mute`, `huddle.camera`, `huddle.screenshare`, `huddle.hand`, `huddle.reaction` — high-frequency relay, realtime-only (kept off `events:huddle` so the API consumers don't parse every ICE candidate) |

All events use the envelope: `{ id, event_type, payload, timestamp }`.

---

## Shared

### shared-common
- `AppError` — unified error type mapped to HTTP status codes (400, 401, 403, 404, 409, 422, 429, 500)
- CORS layer configuration
- Input validation helpers

### shared-events
- `Event` envelope with `id`, `event_type`, `payload` (`serde_json::Value`), `timestamp`
- Typed event payloads for auth, messaging, workspace, and huddle domains

---

## Event Flow

```
HTTP Request
  → API handler
  → PostgreSQL write + event_outbox row (one transaction)
  → XADD stream:ws:{id} + PUBLISH events:*      (outbox relay covers a crash in between)
  → chat-realtime event consumer → WebSocket PUSH to subscribed clients
  → chat-worker consumer groups  → notifications, webhooks, history (each event once)
```
