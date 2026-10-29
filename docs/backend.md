# Backend

Three processes over one codebase, sharing a Postgres database, a Redis instance and
the crates under `shared/`:

| Process | Port | Role | Replicas |
|---|---|---|---|
| **`chat-api`** | 3000 | Stateless REST API. Owns the migrations, applied at startup. | scale freely |
| **`chat-worker`** | 3005 | Background consumers: outgoing webhooks, reminders, notifications, huddle history, call ringing, scheduled messages, email delivery, event outbox relay, text extraction from files. Serves only `/livez`, `/readyz`, `/metrics`. | scale freely |
| **`chat-realtime`** | 3004 | WebSocket gateway. | scale freely |

`chat-api` serves an OpenAPI document at `/api/openapi.json`, generated from the handlers
themselves, and the frontend's `src/api/schema.d.ts` is generated from that. Every feature
is on it except the SCIM provisioning API (its own standard, its own error format) and the
OIDC redirects; the SCIM *token* routes an instance admin uses are on it. The
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
`hooks`, `notifications`, `scheduled`, `later`, `groups`, `emoji`, `retention`, `export`,
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

- **Auth.** Argon2id password hashing; Ed25519-signed (EdDSA) JWTs with an `access` /
  `refresh` token-type claim and a `kid` naming the key. `shared_common::session_keys` signs
  with the first of `JWT_SIGNING_KEYS` and verifies against every listed key; the gateway
  gets only `JWT_PUBLIC_KEYS`. Any other algorithm, a missing `kid` or an unlisted one is
  refused. Refresh tokens are DB-backed, single-use, and rotated; password reset is single-use
  and revokes all sessions. Auth cookies are `HttpOnly; SameSite=Lax`, and `Secure` whenever
  `PUBLIC_URL` is https.
- **Secrets at rest.** Connected apps' client secrets and tokens and TOTP secrets are sealed
  with ChaCha20-Poly1305 by `sealed/`, under a per-purpose key derived from `SECRETS_KEYS`
  (a list of `id:base64` keys, the first one current). Each row stores the key id beside the
  nonce; on start the API re-seals any row not on the current key (`sealed::reseal`), including
  rows 3.7.0 sealed under `JWT_SECRET`, which is read for nothing else. The setup token is an HMAC under the same keyring.
- **Authorization** is re-derived per request from the verified token (`auth.user_id`),
  never trusted from the body — and re-checked against the DB on every WebSocket
  subscribe/join.
- **Rate limiting.** A shared Redis fixed-window limiter (`rate_limit.rs`) guards auth
  endpoints (per-email and per-IP on login, per-email on forgot-password) and incoming
  webhooks (per-token). A per-user `write_rate_limit` middleware caps write methods on the
  messaging and files routers (120 writes / 60s per user).
- **Error handling.** `AppError` (`shared/common`) maps to status codes; 500-class errors
  log detail but return an opaque body so internals/SQL never leak (with tests proving it).
- **Files.** `FileStorage` wraps an `object_store` backend (local disk or any S3-compatible store; the compose stack runs RustFS) and
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

An email address is stored lowercase (`users_email_lowercase` and
`workspace_invites_email_lowercase` refuse anything else), and every lookup lowercases what it
is given, so sign-in, invites and SSO match an address in any case.

Handles user identity — login, registration, JWT, and profile.

**Input / Output:**

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| POST | `/auth/login` | `{ email, password }` | `AuthSession` |
| GET | `/auth/invites/:token/verify` | — | `{ email, workspace_name, workspace_id, already_registered }` — 401 once the invite the link names is revoked, expired or used up |
| POST | `/auth/invites/:token/accept` | JWT | `{ workspace_id, workspace_name }` — an email invite for an account that already exists; same checks, claims a use |
| POST | `/auth/complete-registration` | `{ token, password, display_name }` | `AuthSession` — joins the invite's workspace with the invite's role and claims a use; 401 for a revoked, expired or used-up invite |
| POST | `/auth/refresh` | `{ refresh_token }` (or refresh cookie / Bearer) | `AuthSession` |
| POST | `/auth/logout` | refresh cookie | `{ status: "logged_out" }` |
| POST | `/auth/forgot-password` | `{ email }` | `{ status: "sent" }` whoever asks; a password account gets a reset link, an account still waiting on its invite gets that invite again, and an SSO account is told to sign in with SSO |
| POST | `/auth/reset-password` | `{ token, password }` | `{ status: "reset" }` — the password is checked before the link is spent, so a too-short one leaves it usable |
| GET | `/auth/oidc/start` | Query: `invite?` | Redirect to the provider. With `invite` (an email invite's link token), the callback redeems it for the account that signed in, activates it if it was waiting on the invite, and lands on `/app/:ws_id`; a link for another address lands back on `/invite/:token` |
| GET | `/instance/info` | — | `{ name, icon_url, sso_enabled, max_upload_bytes, setup_required }` |
| POST | `/setup` | `{ token, display_name, email, password, workspace_name }` | `{ session: AuthSession, workspace_id }` and the auth cookies; 403 for a wrong token, 409 once an instance admin exists |
| GET | `/users/me` | JWT | `UserPublic` |
| PATCH | `/users/me` | `{ display_name?, avatar_url?, bio?, timezone? }` | `UserPublic` — replacing or clearing a photo stored under `avatars/` deletes the old object |
| POST | `/users/me/avatar` | multipart `file`: PNG, JPEG, GIF or WebP, at most 10 MB | `UserPublic` with the new `avatar_url` — the photo is sniffed, decoded and stripped of metadata (one wider than 720 px is stored at 720), saved as `avatars/{user_id}/{uuid}.{ext}` with no `files` row, so retention never sees it, and set on the account; the previous one is deleted |
| PATCH | `/users/me/password` | `{ current_password, new_password }` | `{ status: "password_changed" }` |
| PUT | `/users/me/presence` | `{ away }` | `{ away }` — sets `users.manual_away`, which shows the person away in every workspace and on every device until they set it back; publishes `presence.recompute` for the gateway |
| GET | `/workspaces/{ws_id}/profile` | — | `WorkspaceProfile` — the caller's own name, photo, bio and live status in that workspace; `null` where the account's is shown |
| PATCH | `/workspaces/{ws_id}/profile` | `{ display_name?, avatar_url?, bio? }` | `WorkspaceProfile` — a field left out stays; `""` goes back to the account's; replacing or clearing a workspace photo deletes its object |
| POST | `/workspaces/{ws_id}/profile/avatar` | multipart `file`, as `/users/me/avatar` | `WorkspaceProfile` with the new `avatar_url` — processed like an account photo and stored as `avatars/{user_id}/{ws_id}/{uuid}.{ext}`, outside retention; the previous workspace photo is deleted |
| PUT | `/workspaces/{ws_id}/profile/status` | `{ emoji?, text?, expires_at? }` | `WorkspaceProfile` — a status in that workspace only; needs an emoji or some text, the emoji being a character or a `:shortcode:` that workspace has (422 otherwise), and an expiry in the future. An expired status stops being returned by every read; the row itself is only overwritten the next time the person sets one |
| DELETE | `/workspaces/{ws_id}/profile/status` | — | `WorkspaceProfile` |

---

### workspace

Manages workspaces, members, invites, and channels. (Direct messages are their own
feature — see `dm` below.)

**Role model** (`Guest` 10 < `Member` 20 < `Admin` 40 < `Owner` 50):

| Action | Required |
|--------|----------|
| Read channels/messages, post, react, thread | Member of the workspace; **Guest** additionally must be a member of that channel (Guests never get implicit access to public channels) |
| Create a channel | `Member` |
| Browse public channels and self-join one | `Member` — Guests are refused; an open private channel is never listed, an archived one only to its members, and archived ones are listed to read but cannot be joined |
| Rename/archive/unarchive a channel, change a channel member's role, remove someone from a channel | **Channel admin** (`channel_members.role = 'admin'`) or workspace `Admin`; a Guest never moderates, even holding the channel-admin row |
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
| PATCH | `/workspaces/:ws_id` | `{ name?, description?, icon_url?, link_previews?, workflow_builders?, broadcast_mentions?, public_channel_creators?, private_channel_creators? }` | `Workspace` — admins; each is merged into `settings`. `link_previews` defaults to on; the four rules are `members` (the default) or `admins`. `icon_url: ""` removes the icon; a replaced or removed stored icon is deleted |
| POST | `/workspaces/:ws_id/icon` | multipart `file`, as `/users/me/avatar` | `Workspace` with the new `icon_url` — admins; processed like a photo and stored as `icons/{ws_id}/{uuid}.{ext}`, outside `files` and retention, readable by the workspace's members; the previous icon is deleted, and so is the icon on a hard delete |
| DELETE | `/workspaces/:ws_id` | Query: `hard=bool` | `{ status: "soft_deleted" \| "hard_deleted" }` |
| POST | `/workspaces/:ws_id/restore` | — | `Workspace` |
| GET | `/workspaces/deleted` | — | `{ data: Workspace[] }` |

**Members:**

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/workspaces/:ws_id/members` | — | `{ data: MemberWithUser[] }` — name, photo and bio as shown in that workspace (its own, else the account's), its status, and `notifications_paused_until` (`null` unless their notifications are paused now) |
| PATCH | `/workspaces/:ws_id/members/:user_id/role` | `{ role }` | `WorkspaceMember` |
| DELETE | `/workspaces/:ws_id/members/:user_id` | — | `{ status: "removed" }` |

**Invites:**

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/workspaces/:ws_id/invites` | — | `{ data: WorkspaceInvite[] }` |
| POST | `/workspaces/:ws_id/invites` | `{ email?, role? }` | `CreatedInvite` — the invite plus `invite_url` (an email invite's registration link) and `email_status`: `sent`, `not_configured` (`SMTP_HOST` is empty) or `failed` (it could not be queued); `null` for a link invite |
| GET | `/workspaces/:ws_id/invites/:invite_id/link` | — | `{ url }` — a fresh registration link for a pending email invite; 400 once it is used up or expired |
| DELETE | `/workspaces/:ws_id/invites/:invite_id` | — | `{ status: "revoked" }` |
| POST | `/invites/:token/accept` | — | `WorkspaceMember` |

An email invite's link carries a registration token that names the invite row
(`invite_id`) and expires when the invite does. Every use of it loads the row, so revoking
the invite, its expiry and its use limit hold for the emailed link as they do for a shared
one, and the role is the row's, not the token's.

**Channels:**

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/workspaces/:ws_id/channels` | — | `{ data: ChannelListing[] }` — each channel with the caller's `muted`, `notify_level` (`null` when it follows the default) and `effective_notify_level` |
| GET | `/workspaces/:ws_id/channels/unread` | — | `{ channel_ids: string[], counts: { channel_id, unread_count, mention_count, last_read_msg, last_read_at }[] }` — `last_read_msg` is where the *New* line goes; a list that has paged back past `last_read_at` without finding it stops paging and draws no line |
| GET | `/workspaces/:ws_id/channels/browse` | — | `{ data: BrowsableChannel[] }` — every public channel, and every archived private channel the caller is a member of, with `member_count`, `is_member` and `is_archived`; archived ones last, the private ones under the public ones, so an archived private channel, which the sidebar leaves out, can still be reached and unarchived (Members and up; Guests are refused) |
| POST | `/workspaces/:ws_id/channels` | `{ name, channel_type?, description?, is_default?, post_policy? }` | `Channel` — Members and up; only admins when `settings.public_channel_creators` (or `private_channel_creators` for a private one) is `admins` |
| GET | `/channels/:ch_id` | — | `Channel` |
| PATCH | `/channels/:ch_id` | `{ name?, topic?, description?, post_policy?, allow_broadcast_mentions? }` | `Channel` — channel moderators; `post_policy` and `allow_broadcast_mentions` are merged into `settings` and each change is audited |
| DELETE | `/channels/:ch_id` | — | `{ status: "archived" }` — channel moderators; posts a `channel_archived` system line and publishes `channel.updated`. An archived channel refuses every write with 403 until it is unarchived: posts, replies, reactions, scheduled messages, incoming webhooks, editing, deleting and pinning a message, its link previews, poll votes and closing, files, renaming and settings, adding, removing and re-roling members, bookmarks and starting a huddle |
| POST | `/channels/:ch_id/unarchive` | — | `{ status: "unarchived" }` — channel moderators; posts `channel_unarchived` and publishes `channel.updated` |
| PATCH | `/channels/:ch_id/notifications` | `{ muted }` | `{ muted: bool }` — members only (403 otherwise) |

**Channel members:**

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/channels/:ch_id/members` | — | `{ data: ChannelMember[] }` — anyone who can read the channel; a private channel's to its members only, and any channel a guest is not in is refused |
| POST | `/channels/:ch_id/members` | `{ user_id }` | `ChannelMember` |
| POST | `/channels/:ch_id/join` | — | `ChannelMember` — self-join, public and unarchived channels only; idempotent |
| PATCH | `/channels/:ch_id/members/:user_id/role` | `{ role: "member" \| "admin" }` | `ChannelMember` — channel moderators only |
| DELETE | `/channels/:ch_id/members/:user_id` | — | `{ status: "removed" }` |

**Channel bookmarks** (the link bar under the channel header):

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/channels/:ch_id/bookmarks` | — | `{ data: ChannelBookmark[] }` — any channel member |
| GET | `/channels/:ch_id/context` | — | `{ workspace_id, members, pins, bookmarks, lists, workflow_shortcuts }` — what the members, pins, bookmarks, `lists?channel_id=` and workflow-shortcuts routes return, in one request; refused where pins and bookmarks are, and `workflow_shortcuts` is empty rather than 403 in a channel the caller cannot post in |
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
| POST | `/workspaces/:ws_id/scheduled-messages` | `{ channel_id, content, send_at, attachment_ids?, thread_parent_id?, also_to_channel? }` | `ScheduledMessage` — the drafts are held until a day after `send_at` and attached at delivery; a `thread_parent_id` delivers it as a reply, and one whose thread is gone by then fails with `thread_gone` |
| PATCH | `/scheduled-messages/:id` | `{ send_at }` | `ScheduledMessage` — author only, pending or failed; a failed one is queued again |
| DELETE | `/scheduled-messages/:id` | — | `{ status: "canceled" }` — author only, pending or failed |
| POST | `/scheduled-messages/:id/send` | — | `{ status: "sent" }` — author only, pending or failed: delivers it now; 409 with the reason when delivery is refused, which is also recorded as its `failure` |

A background dispatcher ticks every 15s and claims due rows with
`UPDATE … WHERE id IN (SELECT … FOR UPDATE SKIP LOCKED) RETURNING *`, so several api replicas
can run it without delivering a message twice. Delivery reuses the normal send path — mentions
are expanded and `message.created` is published — and a failure is recorded on the row instead
of retried. The author is checked again when it fires: a suspended account fails with
`author_inactive`, a lost channel or workspace with `not_authorized`.

---

### messaging

Sends and manages messages, threads, reactions, pins, read tracking, and search.

**Messages:**

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/channels/:ch_id/messages` | Query: `limit=50, cursor?, mark_read?` | `{ data: Message[], unread_notifications?, last_read_before?, last_read_before_at? }` — with `mark_read=true` and no cursor, the newest message becomes the caller's read position (as `POST /channels/:ch_id/read`), `unread_notifications` is the workspace's count afterwards, and `last_read_before` is the message the caller had read up to when anything after it was unread (where the *New* line goes), read at `last_read_before_at` |
| POST | `/channels/:ch_id/messages` | `{ content, thread_parent_id?, also_to_channel?, client_message_id?, attachment_ids?, forwarded_from? }` | `Message` — the one send path; a `thread_parent_id` makes it a reply (one level deep, same channel; 404 once the root is deleted and has no replies left), `also_to_channel` shows that reply in the channel too (`metadata.also_in_channel`), and `client_message_id` makes a retry idempotent. `attachment_ids` (at most 10, the sender's own drafts) are linked in the message's transaction and listed in `metadata.attachments`; `content` may then be empty. `forwarded_from` (a forward) copies that message's files, if the sender can read it, and records `metadata.forwarded_from`; the quoted lines (`>`) of a forward neither mention nor broadcast, only the sender's own words do |
| PATCH | `/messages/:msg_id` | `{ content, attachment_ids? }` | `Message` — `attachment_ids` is the set of files to keep; the rest are deleted |
| DELETE | `/messages/:msg_id` | — | `{ status: "deleted" }` |
| DELETE | `/messages/:msg_id/unfurls` | — | `Message` — the author removes its link previews; they stay removed across edits |

**Link previews.** `chat-worker`'s `unfurl` consumer reads every `message.created` and
`message.updated`, takes the first three `http(s)` links outside code spans, and fetches
each through `hooks::ssrf::resolve_outbound` in strict mode — whatever
`WEBHOOK_ALLOW_PRIVATE_TARGETS` says, since any member can post a link — connecting to the
addresses it checked, re-checking every redirect hop, with a 3 s timeout, a 512 KB cap and
`text/html` only. OpenGraph tags, falling back to `<title>` and `<meta name=description>`,
become `metadata.unfurls`, written only while the content is still the content that was
read and announced with `message.updated`. `metadata.unfurls_source` (a hash of the link
list) is what makes a redelivered event and the consumer's own update a no-op, and an edit
that keeps the links free. Each URL's result is cached in Redis for 24 hours. A workspace
turns previews off with `PATCH /workspaces/:ws_id { link_previews: false }`.

YouTube, Vimeo and Loom links (`unfurl::video`) skip the page and ask the service's oEmbed
endpoint — a fixed host, so no SSRF resolution is involved — about the canonical form of the
link, which also sidesteps YouTube's consent page. The result is `kind: "video"` with
`embed: { provider, id, hash?, start? }`; the client builds the player URL from those and
never renders oEmbed's `html`. The CSP's `frame-src` lists exactly the three player hosts.
A video oEmbed refuses (private, removed, not embeddable) falls back to the OpenGraph
fetch.

**Threads:**

A thread is one unit. Deleting a root that has live replies keeps it as a tombstone:
`deleted_at` set, `content` and `metadata` served empty, still listed in the channel so its
replies stay reachable and still repliable. A root with no live replies goes, as any deleted
message does. Deleting a reply takes it off the root's `reply_count` and announces the root
with `message.updated`. A reply never appears in the channel list unless it was sent with
`also_to_channel`. Retention removes a thread once its newest message is past the period,
root and replies together, and erasing a person tombstones their roots that others answered.

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/messages/:msg_id/thread` | Query: `limit=50 (≤200), cursor?` (the last reply of the previous page) | `{ data: MessageWithReactions[] }` — newest reply first, with reactions; the root may be a tombstone |
| GET | `/messages/:msg_id/location` | — | `{ workspace_id, channel_id, kind: channel \| dm \| group_dm, thread_parent_id?, joined }` — where a message lives, for opening it from a link, a notification or a search hit; `joined` is false for a public channel the caller can read but is not in. A message the caller cannot read is a 404, as is a deleted one, unless it is a thread root that still has replies |

**Mentions.** Sending a channel message or thread reply expands the mentions in its body
into the `mentioned_user_ids` carried by `message.created`, which the notifications
consumer turns into per-user notifications for the channel's members (still subject to
their notification level, channel mute and DND; a mentioned non-member gets nothing). A mention is either a picked token (`@[Label](uuid)`) or one of the broadcasts —
`@channel` / `@everyone` (every channel member) and `@here` (channel members with a live
`presence:*` key in Redis). Broadcasts count whether the author picked them from the
composer (`@[channel](channel)`) or simply typed the word; the author is never notified of
their own message, and a named user inside a broadcast is notified once, not twice.
`authz::can_broadcast` decides who may: nobody in a channel whose
`settings.allow_broadcast_mentions` is `false`, and only workspace admins and the channel's
own admins when the workspace's `settings.broadcast_mentions` is `admins`. Sending a
message, creating a poll and scheduling a message answer 403 with the reason for a
broadcast the author may not make; saving a workflow whose `send_message` step would make
one answers 422. A scheduled message or a workflow run whose author lost the right still
posts, but `expand_mentions` leaves the broadcast out and nobody is notified by it.

**Reactions:**

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/messages/:msg_id/reactions` | — | `{ data: Reaction[] }` |
| POST | `/messages/:msg_id/reactions` | `{ emoji }` | `Reaction` — a character of at most eight code points, or a `:shortcode:` the workspace has a custom emoji for (`422` otherwise) |
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
| POST | `/channels/:ch_id/read` | `{ message_id }` | `{ status: "read" }` — for channels and conversations alike; the read position becomes the newest message the channel shows (top level, or a reply also sent to it) at or before `message_id`. Also marks the channel's own notifications read, a reply's that was also sent to the channel among them; its threads' other notifications stay until each thread is read |
| POST | `/workspaces/:ws_id/read` | — | `{ status: "read" }` — moves the caller to the newest message shown in every unread channel and conversation of the workspace, by the same rule, one `channel.read_state` per channel, and marks their notifications read (not threads') |
| GET | `/workspaces/:ws_id/unreads` | Query: `limit=20 (≤50), offset=0` | `{ data: UnreadGroup[], next_offset }` — every channel and DM the caller has unread messages in, muted last and otherwise newest activity first, each with its newest 20 unread messages (oldest first, the caller's own left out) |
| POST | `/channels/:ch_id/unread` | `{ message_id }` | `ReadState` — moves the caller's read position to just before that message and recomputes their unread and mention counts for the channel |
| GET | `/search` | Query: `q?, workspace_id, channel_id?, user_id?, has? (file \| image \| link), after?, before?, scope?, limit=20, offset=0` | `{ data: SearchHit[] }` — each a `Message` plus `channel_name`, the channel's name (also for a public channel the caller is not in), `null` in a conversation; `q` may be empty when a filter is set (newest first); `channel_id` takes a channel or a conversation; `after` is inclusive and `before` exclusive (RFC 3339 instants); `has=file` counts images too |
| GET | `/search/files` | Query: `q?, workspace_id, channel_id?, user_id?, kind?, after?, before?, limit=20, offset=0` | `{ data: FileSearchHit[] }` — attachment fields plus `kind`, `message_id`, `channel_id`, `channel_name` (as on `/search`), `user_id`, `created_at` and `snippet`. Any fragment of the name matches, ignoring case and accents (a near miss also matches, ranked after), and so does a file whose extracted text holds every word of `q`, ranked after every name match by `ts_rank`; then newest first. A content match carries `snippet: { text, hit }[]`, up to two fragments of the text around the words, with `hit` on the words themselves — parts, never markup, so the client cannot render the file's text as HTML. `q` may be empty when a filter is set. `kind` is one of `pdf, image, video, audio, spreadsheet, presentation, document, archive, text, other`. Only files of live messages in channels the caller can search: the same `searchable_channels()` set as `/search` |

Search is access-scoped: results come only from public channels in the workspace
plus the private/DM channels the caller belongs to.

---

### files

File upload and download. Supports both local storage and S3 (RustFS in the compose stack).
A message's files belong to it through `files.message_id` and are copied into
`messages.metadata.attachments` when they are linked, so every path that returns a message —
a page, a thread, search, pins, a realtime frame — carries them without a join. A composer
uploads with `?draft=true`: the file is the uploader's alone until a send claims it, and
`chat-worker`'s draft sweeper deletes drafts nobody sent within a day. Avatars and custom
emoji upload without it and are never swept. Deleting a file that is attached removes it from
its message and announces the update.

**Images** are recognised by their first bytes (PNG, JPEG, GIF, WebP), not by the client's
`Content-Type`, and stored with the sniffed type. Up to 30 MB they are buffered and processed
on a blocking thread (`files::imaging`): a canvas over 50 megapixels or 20,000 px a side is
refused before decoding; EXIF orientation is applied and the upright `width`/`height` stored;
an image wider than 720 px gets a thumbnail (JPEG, or PNG when it has transparency) under
`thumbnail_key`; and the original's EXIF, XMP, comments and PNG text chunks are removed without
re-encoding its pixels — a JPEG that was not upright keeps an EXIF block holding only its
orientation. GIFs keep their animation. An image that does not decode, or is larger than
30 MB, is stored as it came. Thumbnails are served through the download route under the same
access rule as their file, and every download is `Cache-Control: private, max-age=31536000,
immutable` because a key never changes content.

**Names.** `files.filename` is the name as uploaded, normalised to NFC, with any path, control
characters and leading dots removed and at most 200 characters (a long name keeps its
extension). The storage key uses an ASCII form of it (`files::names::storage_name`), so a key
is always one safe path segment and keeps its extension. Downloads send both
`filename="ascii fallback"` and `filename*=UTF-8''…` (RFC 6266 / 5987).

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| POST | `/files/upload/:ws_id` | Query: `draft?`; multipart form (files) | `FileUploadResponse[]` |
| GET | `/files/:file_id` | — | `{ file: FileRecord, url: string }` |
| GET | `/files/download/*key` | Header: `Range?` | The file, streamed from storage — a single `bytes=` range answers `206` with `Content-Range`, an unsatisfiable one `416`, several ranges the whole file; always `Accept-Ranges: bytes`. A custom emoji's image is served here too, to anyone in its workspace, an account photo (`avatars/{user_id}/…`) to its owner and anyone who shares a workspace with them, and a workspace photo (`avatars/{user_id}/{ws_id}/…`) to its owner and that workspace's members; anyone else gets `403` |
| DELETE | `/files/:file_id` | — | `{ status: "deleted" }` |
| GET | `/files/workspace/:ws_id` | Query: `limit=50, offset=0` | `{ data: FileRecord[] }` |
| GET | `/channels/:ch_id/files` | Query: `kind?` (`image`, `video`, `audio`, `document`), `cursor?` (a file id), `limit=50` | `{ data: ChannelFile[] }` — the files of the channel's live messages, newest first, to whoever can read the channel |

**Text inside files.** `chat-worker`'s `file_text_extractor` (`files::extractor`) reads every
file attached to a message once and writes `file_texts` (`file_id` → `status`, `content`,
generated `search_vector`), a table of its own so the hot `files` row stays small. It claims
up to 20 files with no row by inserting `pending` rows (`FOR UPDATE SKIP LOCKED`, `ON CONFLICT
DO NOTHING`, newest first), so two workers never read the same file, and the same loop is the
backfill for files that existed before migration 61. A claim older than ten minutes is a
worker that died mid-file: it is taken again, and after three attempts it is `failed`.
Every other outcome is final — `done`, `empty`, `unsupported`, `too_large` or `failed` — so
a file that cannot be read is never retried. Deleting the file, by hand, by retention or with
its channel, cascades to its text; the export leaves it out, being derived data.

`files::text::extract` picks the format from `file_kind_of()` and the extension: PDF with
`pdf-extract`; DOCX (body, headers, footers, notes), XLSX (shared and inline strings) and PPTX
(slides in order, then speaker notes) by reading their XML parts with `zip` and `quick-xml`;
plain text, Markdown, CSV and source code as UTF-8. Images, media, archives and the old
binary Office formats are `unsupported` and never downloaded. The text is NFKC-normalised
(ligatures such as `ﬁ` become `fi`), control characters become spaces and whitespace is
collapsed. Because these are untrusted files: an object over 25 MB is `too_large` before it
is downloaded; a zip with more than 10,000 entries, or whose XML parts would inflate past
64 MB, is `too_large`, checked against the declared size and again while reading; text stops
at 1 MB on a character boundary; each extraction runs on a blocking thread inside
`catch_unwind` under a 30 s timeout, so a parser that panics or loops costs one `failed` row,
not the worker. A password-protected PDF is `unsupported`. If Postgres refuses the
`tsvector` as too long, the text is halved until it fits.

**Why `pdf-extract`.** Both candidates were run over thirteen real PDFs (licences, manuals,
papers, a Serbian contract, an encrypted file, truncated and garbage input). `lopdf`'s own
text extraction is three to nine times faster (197 ms against 966 ms for a 2.8 MB licence)
but drops the space between words wherever the PDF positions them instead of writing one —
14 % of the word breaks in one paper were lost, joining words into tokens nobody searches
for. `pdf-extract` (built on `lopdf`) keeps them, reports an encrypted file as a decryption
error where `lopdf` returned empty text, and rejected every malformed file without panicking.
Search is what the text is for, so correct word breaks won over speed on a background path.

The snippet is `ts_headline` with `public.search_headline`, a copy of `simple` that passes
words through `unaccent` first: the query is matched without accents while the fragment keeps
the original spelling (`Čačak` is marked for `cacak`). It runs only on the page of results.

---

### hooks

Incoming/outgoing webhooks, bots and slash commands. A background task executes hooks from Redis events; reminders live in `later`.

**Hooks:**

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/workspaces/:ws_id/hooks` | — | `{ data: Hook[] }` |
| POST | `/workspaces/:ws_id/hooks` | `{ hook_type, name, description?, config?, app_id? }` | `Hook` — `app_id` hangs it off an app of the workspace (admin; a slash command needs the app's `commands` scope), and it is removed with the app |
| GET | `/hooks/:hook_id` | — | `Hook` |
| PATCH | `/hooks/:hook_id` | `{ is_active: true }` | `Hook` — turns a stopped hook back on for someone allowed to make it (its type's role and every channel it is bound to), who becomes its creator; `disabled_reason` clears and `hook.enabled` is audited. `false` is a 422: a hook is turned off by deleting it |
| POST | `/hooks/:hook_id/reveal` | — | `{ hook_id, hook_type, config, incoming_url }` — config unredacted, written to `audit_log` |
| POST | `/hooks/:hook_id/rotate` | — | same shape, after minting a fresh `token` (incoming) or `secret` (outgoing); the previous value stops working immediately |
| DELETE | `/hooks/:hook_id` | — | `{ status: "deleted" }` |
| POST | `/hooks/incoming/:token` | `{ text }` | `{ status: "ok", message_id }` |
| GET | `/channels/:ch_id/integrations` | — | `{ incoming_webhooks, outgoing_webhooks, slash_commands, workflows, can_add_webhook }` — what touches the channel, for anyone with access to it: incoming webhooks bound to it, outgoing webhooks and commands whose `channel_ids` include it, workflows whose trigger listens to it or whose `send_message` step posts into it; names and authors only, never a URL, token or secret |

Hook types: `incoming_webhook`, `outgoing_webhook`, `slash_command`

Built-in commands run before any registered one and never leave the instance: `/away`,
`/dnd`, `/topic`, `/invite`, `/shrug` and `/remind`. `/away` switches `manual_away` on or off. `/remind` understands `me in 30m to ship`,
`me at 15:00 to call back` and `me tomorrow at 9am standup`; clock times are resolved in the
caller's own timezone and a time already past means tomorrow. Reminding somebody else needs
workspace admin, the same rule `POST /workspaces/:ws_id/later` applies. A reminder lands in the person's *Later* list.

`GET`/`POST`/`DELETE` on `/hooks` and `/workspaces/:ws_id/hooks` require a workspace
admin session, with one exception: a channel admin creates, reveals, rotates and deletes the
`incoming_webhook`s of their own channel (`authz::can_manage_channel_hook`). Outgoing
webhooks and slash commands send what people type to a third-party URL, so they stay with
workspace admins, and attaching any hook to a private channel needs its membership. Secrets in `config` (`token`, `secret`, …) are redacted on read — `reveal`
is the only way back to the plaintext value, and it leaves an audit trail. Creating an
`outgoing_webhook` requires an http(s) `config.url` and mints `config.secret` when omitted
(the delivery path still runs the full SSRF check per request).

**A hook acts for the person who made it.** Every time an outgoing webhook would fire and every
time an incoming webhook is called, `authz::hook_still_acting` asks whether its creator's
account is active, they are still in the workspace, they can still read the channel, and they
still hold the role it takes to make that hook (an admin for outgoing webhooks and slash
commands, a channel moderator for incoming ones). If not, the hook is
turned off with the reason in `Hook.disabled_reason` and nothing is sent or posted. Suspending
or erasing an account turns off all of that person's hooks and workflows at once
(`hook.disabled` / `workflow.disabled`, by the admin who did it); removing someone from a
workspace turns off theirs in it. Activating the account again turns nothing back on.
Creating an `incoming_webhook` requires `config.channel_id`, and the server mints a
`config.token`. **`POST /hooks/incoming/:token` is authenticated by that URL token,
not a session** (Slack-compatible `{ "text": ... }`); it posts to the bound channel
under the hook's creator's id with `metadata.bot = { hook_id, name, icon_url }`, and is
rate-limited per token. The creator is notified of it like any other member, and a
notification about it is titled with the bot's name.

**Slash commands:**

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/workspaces/:ws_id/commands` | — | `{ data: CommandInfo[] }` — built-in and registered commands the caller can run |
| POST | `/channels/:ch_id/commands` | `{ command, text }` | `CommandResponse` — `{ response_type: "ephemeral" \| "in_channel" \| "pending", text }` |
| POST | `/commands/responses/:token` | `{ text, response_type?, replace_original? }` | `{ status: "ok" }` — authenticated by the token in the URL, not a session |

A registered command is a `slash_command` hook whose `config.url` receives a signed POST
(`X-ChatSystems-Signature`, the outgoing-webhook HMAC) with `{ command, text, channel_id,
workspace_id, user_id, request_id, response_url }` and the header `X-Chat-Request-Id` equal to
`request_id`. The app has three seconds. A body of `{ text, response_type }` is the answer
there and then; an empty 2xx body acknowledges, and the caller sees */deploy is working on it…*
until the app answers through `response_url`. That URL takes up to five answers within 30
minutes (`command_invocations.uses_left`, `expires_at`); after that, or with a forged token, it
is a 404. It is `{PUBLIC_URL}/api/commands/responses/{id}.{mac}`, the mac an HMAC of the
invocation id under the `command-responses` key of `SECRETS_KEYS`, so the token itself is never
stored. `in_channel` posts a bot message as the caller, if they may post there, and
`replace_original` edits the one the invocation posted before; `ephemeral` (the default) goes
as a `command.response` frame to the caller's sockets only. An answer still needs the caller to
read the channel and the hook to be on (403 otherwise).

A connection error, a timeout or a 502–504 is retried by the worker's `command_retrier` after 2
and then 8 seconds, with ten seconds each and the same `X-Chat-Request-Id`, so an app can drop
a repeat; meanwhile the caller sees */deploy is taking a moment…*. After the third failure the
caller is told */deploy could not be reached*. Any other 4xx or 5xx is the app's answer and is
never retried: on the first attempt it is a 400 with the reason, on a retry an `error` frame.
Rows past `expires_at` are purged every five minutes.

**Reminders:**

| Method | Route | Input | Output |
--------|-------|-------|--------|
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
| GET | `/workspaces/:ws_id/notifications/dnd` | — | `{ dnd_until, schedule_paused_until }` — the running timer in that workspace and, if its schedule is pausing notifications now, when it resumes |
| PATCH | `/workspaces/:ws_id/notifications/dnd` | `{ dnd_until: timestamp \| null }` | `{ dnd_until, schedule_paused_until }` — `null` during a scheduled pause also resumes until the next window opens |
| GET | `/workspaces/:ws_id/notifications/schedule` | — | `{ enabled, days, start_minute, end_minute, timezone }` (weekdays 9:00–18:00, off, when never set) |
| PUT | `/workspaces/:ws_id/notifications/schedule` | `{ enabled, days: 1–7 (ISO, Monday = 1), start_minute, end_minute }` | the schedule — minutes 0–1439, start ≠ end (422 otherwise); an end before the start crosses midnight |
| GET | `/workspaces/:ws_id/notifications/email` | — | `{ mention_emails, available }` — `available` is false without SMTP |
| PATCH | `/workspaces/:ws_id/notifications/email` | `{ mention_emails }` | `{ mention_emails, available }` |
| PUT | `/channels/:ch_id/notify-level` | `{ level: "all" \| "mentions" \| "nothing" \| null }` | `{ notify_level, effective_notify_level }` — members only; `null` returns the channel to the default |
| GET | `/workspaces/:ws_id/notify-default` | — | `{ notify_default }` |
| PUT | `/workspaces/:ws_id/notify-default` | `{ level: "all" \| "mentions" \| "nothing" }` | `{ notify_default }` |
| GET | `/workspaces/:ws_id/notification-keywords` | — | `{ keywords: string[] }` |
| PUT | `/workspaces/:ws_id/notification-keywords` | `{ keywords: string[] }` | `{ keywords }` — replaces the list; each 2–64 characters, not starting with `@` or `#`, at most 50 (422 otherwise); a keyword equal to an earlier one after normalization is dropped |

All workspace-scoped routes require workspace membership.

**Notification levels.** Every channel member has a `notify_level` on `channel_members`
(`all`, `mentions`, `nothing`, or `NULL` to follow the default), and every workspace member a
`notify_default` on `workspace_members` (`mentions` unless changed). The SQL function
`effective_notify_level(stored, default, channel_type)` resolves them: a stored level wins, a
DM or group DM otherwise means `all`, and a channel otherwise takes the workspace default. The
consumer decides each recipient's reason for one `message.created`:

| Reason | When | Activity row | Frame `kind` | Push | Email |
|--------|------|--------------|--------------|------|-------|
| Mention | named or broadcast, level is not `nothing` | yes | `mention` | yes | yes |
| Reply | follows the thread, level is not `nothing` | yes | `reply` | yes | no |
| Direct | DM on `all`, not a thread reply (one also sent to the channel counts) | yes (`dm`) | `dm` | yes | yes |
| Keyword | one of the person's keywords is in the text | yes (`keyword`) | `keyword` | yes | no |
| Message | channel on `all`, not a thread reply (one also sent to the channel counts) | no | `message` | yes | no |

The first row that applies wins, so a message is one notification per person. Keywords live
in `notification_keywords`, stored with `normalized = lower(search_normalize(keyword))`, the
same folding search uses. One query per message finds the channel members whose keyword
appears in `lower(search_normalize(content))` as a whole word, the quoted lines of a forward
left out as they are for mentions: `keyword_pattern()` escapes the
regex characters and requires a non-word character or the text's edge on both sides, so
`deploy` matches "Deploy!" and "deploý" but not "redeployed", and `c++` matches literally.
A keyword does not bump the unread or mention counters, which are written with the message —
and only for a message shown in the channel: a reply kept in its thread counts in that thread
(*Threads*), not in the channel, unless it was also sent to the channel. A push carries
`badge_count`, the person's unread Activity across every workspace on the instance, the same
total the app shows on its icon.

Muted and `nothing` mean no reason at all. DND — `notifications_paused_until(workspace, user, at)` in
SQL, the one definition of "paused" — keeps the Activity row and drops the rest; a
frame goes to every session and push to every device unless a session is *active* (below).
A mention or DM with email on is queued in `pending_mention_emails`, once per person and
message; five minutes later the digest sweep drops what has been read since, and emails one
digest per person and workspace for what is still unread, linking to the message when there
is one (`/app/{ws}/{channel}/{message}`) and to the workspace otherwise. The rows are claimed
and the email queued in one transaction, so two workers cannot send it twice. The frame's `kind` also carries `reaction`, `call`, `reminder` and
`system` from the other producers; the client rings and counts on it rather than deciding
for itself. Channel listings and conversation summaries carry the caller's `notify_level` and
`effective_notify_level`.

**Active, not just connected.** An open socket is not someone looking: a tab left in the
background, or a phone that kept its socket for a while, used to hold back every push. The
client sends `{ "type": "activity", "active": bool }` — active while its page is visible and
has had input in the last two minutes, idle the moment it is hidden or unloaded — and the
gateway keeps `active:user:{user}`, a hash of connection → expiry refreshed on each heartbeat
and removed on idle or disconnect. A heartbeat renews only a connection that answered within
the last interval and removes one that did not, so a laptop shut mid-use stops holding push
back within about 30 seconds. Push is held only while some entry has not expired. Its badge is
the person's unread notifications in the workspaces they are still a member of, the number
the app shows.

**Paused.** A pause belongs to one workspace: `notifications_paused_until(workspace, user,
at)` is the later of the `/dnd` timer (`workspace_members.dnd_until`) and
`schedule_paused_until(workspace, user, at)` over that member's row of
`notification_schedules`, which is `NULL` inside the
person's window and otherwise the next time a window opens. The window is one start and
end, the same on every selected day, in `users.timezone`: the instant is turned into local
wall time, so 9:00 stays 9:00 across a DST change, and a window whose end is before its
start runs past midnight into the next day. Resuming during a scheduled pause stores the
next window's start in `workspace_members.dnd_override_until`; until then the schedule is ignored. The
push path, the email digest (pending emails wait until the person is unpaused) and the
member listing all call the same function, with the instant passed in by the caller.

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
| GET | `/slack-imports/{import_id}` | — | `ImportRun` — status, live counts, what was skipped, and `retryable` |
| POST | `/workspaces/{ws_id}/slack-imports/{import_id}/retry` | — | `ImportRun` back to `pending`, its report cleared; workspace admins only. Only a `failed` run whose archive is still stored (`retryable`) can be retried, otherwise `409` |

**A run the worker lost fails instead of running for ever.** While a run is being imported the
worker writes `slack_imports.heartbeat_at` every 30 seconds; each poll first marks any
`running` row whose heartbeat is older than five minutes `failed` with *The import stopped
when the worker did*. A failed run keeps its archive (only a finished one deletes it), so
*Start again* re-queues it, and the import's idempotency makes the second attempt pick up
where the first stopped.

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
were quietly dropped should not look the same. A public Slack channel merges into an existing
public one of the same name. Any other one, and every private one, lands in `<name>-slack`
(then `-slack-2`, …) and the report says so, so a private history never ends up in a public
channel and nobody from Slack joins a private channel that was already here.

**Two passes, because threads are what break single-pass importers.** The first creates
users, channels, conversations and memberships; the second writes messages, so a `thread_ts`
resolves against a row that already exists. A reply whose parent is missing from the export
stays in the channel as an ordinary message rather than being dropped, and says so in the
report.

**Users are matched by email and by nothing else.** A Slack handle is not an identity.
Somebody with no matching account is created with no password, so their history is
attributed to an account only they can claim through the ordinary invite flow; a bot, or a
current account the export carries no address for, is reported rather than guessed at. A
Slack guest (`is_restricted` or `is_ultra_restricted`) joins as a guest, so it sees only the
channels it was in. An account deleted in Slack is created **suspended** — with
`slack-<id>@deleted.invalid` when the export kept no address — so its messages keep its name
and nobody can sign in as it or be invited into it; an existing account matched by email is
left as it is. Reactivating such an account sets it to `pending`, so an invite lets the person
choose a password.

**Archived stays archived.** A channel archived in Slack is created, filled and then archived,
and a re-run archives it again. One that merged into an existing channel here is left open
and the report says so, on the first run and on every later one: `slack_channels.created_by_import`
records which channels the import made, and only those are archived.

**Reactions map the way the composer does.** `slack_import/emoji_shortcodes.tsv` is the full
emoji-mart shortcode set with its aliases and skin tones (`+1::skin-tone-3` is 👍🏼), generated
from the frontend's `@emoji-mart/data` by `node scripts/emoji-shortcodes.mjs` in `frontend/`.
A reaction that is not in it stays `:name:`, which renders as the workspace's custom emoji when
one of that name exists; a name that is neither is listed in the report once.

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
| `slack_imports` | One row per run: source, dry-run flag, status, the report as JSON, the error if it failed, and the worker's `heartbeat_at` while it runs |
| `slack_users` | `(workspace, slack user id)` → our user |
| `slack_channels` | `(workspace, slack channel id)` → our channel, DMs included |
| `messages.slack_ts` | The Slack timestamp a message came from; unique per channel |

---

### later

Slack's *Later*: one person's list of saved messages and reminders, each *in progress*,
*completed* or *archived*. An item is a saved message (`message_id`), a reminder (`text` and
`remind_at`), or both — a saved message given a due time. Saving a message is idempotent and
brings a completed or archived one back; reading the message is what entitles you to save it,
the same channel access check the message routes use. A reminder for somebody else needs
workspace admin, as `/remind` does. `chat-worker`'s reminder checker claims due items, sends
the push (unless the person lost the channel it names) and stamps `reminded_at`; the item
stays *in progress* until it is ticked, and a new `remind_at` re-arms it. Every change sends a
`later.updated` frame to the owner's sessions. Items go with the workspace membership.

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/workspaces/:ws_id/later` | Query: `status?` (`in_progress`, default; `completed`; `archived`) | `{ data: LaterItem[] }` — with the message (author, channel, content, attachments) in one round trip; in progress sorted by due time, the rest by last change |
| POST | `/workspaces/:ws_id/later` | `{ message_id }` or `{ text, remind_at, channel_id?, for_user_id? }`, `remind_at?` on either | `{ id }` |
| PATCH | `/later/:id` | `{ status?, remind_at?, clear_reminder?, note? }` | `{ status: "updated" }` — owner only |
| DELETE | `/later/:id` | — | `{ status: "deleted" }` — owner only |

---

### threads

The threads a person follows, as Slack's *Threads* view lists them. `thread_follows` holds one
row per person and thread root: whether they follow it and how far they have read. Replying
follows the thread and reads it up to the reply; the first reply follows it for the root's
author, read up to just before it; a mention in a reply follows it for the person mentioned.
*Unfollow* is a row with `following = false`, so a later reply does not bring it back — only a
reply of their own or a mention does. A reply from someone else after `last_read_at` is
unread. A thread drops out of every count and list once its follower can no longer read the
channel, the same check the message routes use. Every change sends a `thread.read_state`
frame to the follower's sessions, and `chat-worker` sends followers the `reply` notification
(in-app and push, never email; mute and DND apply), except the reply's author and anyone it
mentions, who already have the mention. Rows go with the workspace membership and the root.

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/workspaces/:ws_id/threads` | Query: `cursor?` (the previous page's `next_cursor`), `limit=20 (≤50)` | `{ data: ThreadSummary[], next_cursor }` — followed threads with replies, newest reply first, each with its root, the latest three replies (oldest first), `unread_count`, `last_read_at` and `last_reply_at` |
| GET | `/workspaces/:ws_id/threads/unread` | — | `{ count }` — followed threads with an unread reply |
| POST | `/threads/:root_id/read` | — | `ThreadState` (`{ root_message_id, following, unread_count }`) — a reply's id reads its root; marks the thread's notifications read and announces `thread.read_state` |
| PUT | `/threads/:root_id/follow` | — | `ThreadState` |
| DELETE | `/threads/:root_id/follow` | — | `ThreadState` |

---

### polls

A poll is a message: its content is the question, so search finds it, and `metadata.poll`
carries the question, the options with their counts, `voters`, `closes_at`, `closed_at` and
`created_by`, never who voted. `polls`, `poll_options` (2–10, in order) and `poll_votes` hold
it; `poll_votes` has one row per option a person picked, keyed `(option_id, user_id)`, and a
partial unique index keeps a single-choice poll at one row per person. A vote locks the poll
row, replaces the caller's choice, recounts and rewrites `metadata.poll` in one transaction,
then stages `message.updated` through the outbox, so every client sees the new counts. A
closed poll refuses votes; `chat-worker`'s reminder checker closes a poll at its `closes_at`.
Creating needs the right to post in the channel (`require_channel_post`), voting needs only
read access, so everybody votes in an announcement channel; closing is for the creator or a
channel moderator. Voters of an anonymous poll are refused to everybody, its creator
included. A poll message cannot be edited. Workspace export writes `polls.jsonl` with counts
and `poll_votes.jsonl` for polls that are not anonymous.

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| POST | `/channels/:ch_id/polls` | `{ question, options[], multiple_choice?, anonymous?, closes_at?, thread_parent_id? }` | `Message` — the poll message, in a thread when `thread_parent_id` is set |
| GET | `/polls/:poll_id/vote` | — | `{ option_ids }` — the caller's own choice |
| PUT | `/polls/:poll_id/vote` | `{ option_ids }` — an empty list withdraws the vote | `{ option_ids, poll }` — 409 when closed, 422 for an unknown option or two on a single-choice poll |
| GET | `/polls/:poll_id/votes` | — | `{ data: [{ option_id, user_ids }] }` — 403 on an anonymous poll |
| POST | `/polls/:poll_id/close` | — | `PollSummary` — creator or channel moderator; 409 when already closed |

---

### lists

A list is a table kept in one conversation (a channel, DM or group DM) or, with no
`channel_id`, a personal list only its creator sees. Reading it is the conversation's read
access (`require_channel_access`), editing it is the right to post there
(`require_channel_post`), so an announcement channel's list is read-only to everyone but its
posters; there is no permission system of its own. Deleting a list is for its creator or a
channel moderator. `list_columns` (at most 30) have a kind — `text`, `number`, `select`,
`multi_select`, `person`, `date`, `checkbox`, `link` — and `options`: a select's `choices`
(`{ id, label, color }`) and a number's `number` format (`plain`, `percent`, `currency` with a
three-letter code, 0–6 decimals), and `hidden`, which hides the column from the table for
everyone who sees the list. `list_items` (at most 10,000) keep `values` keyed by column
id and a `rank`, a fractional index, so moving an item rewrites that one row.

Every cell is checked against its column on the server: a select takes one of its choice ids,
a person must be a member of the list's conversation (of the workspace, for a personal list),
a date is `YYYY-MM-DD`, a link starts with `http://` or `https://`. `null` clears a cell.
`PATCH /list-items/:id` merges (`values || $1`), so two people editing different cells of one
row both keep their edit, and the same cell is last write wins. Changing a column's kind or
options converts the cells it can (number ↔ text, select ↔ text by label, a renamed choice
keeps its cells) and clears the rest, but only when `confirm_cleared` names how many: without
it the change is 409 "This change clears N cells". `POST /list-columns/:id/preview` gives the
numbers first. A change to a list's structure locks the list row; a cell edit takes a share
lock, so edits run side by side but never against a column that is changing.

`list_views` (at most 20 a list, never fewer than one; a new list starts with a "Table") are
saved on the list and shared by everyone who sees it, as in Slack; changing one takes the
right to edit the list. A view is a `table` or a `board` with a `config`: `group_by` (a board's
single-select column), `sort` (at most 3 `{ column_id, direction }`) and `filters` (at most 10,
all of which must hold, `{ column_id, op, value? }`). The server checks each rule against its
column's kind: `contains` for text and links, `is`/`is_not` with choice ids or people (`"me"`
is whoever looks), `lt`/`gt` for numbers and dates, `this_week`/`overdue` for dates,
`empty`/`not_empty`, `checked`/`unchecked`. Filtering and sorting run in the client over the
items it already holds. A column change or deletion prunes every view in the same
transaction — a deleted column's rules, a removed choice, a filter the new kind cannot take,
a board's grouping by a column that is no longer a single-select — so a saved view never
refers to what is gone.

Each change is staged in the outbox in its transaction and sent to the list's audience: the
channel for a channel list, the owner's sessions for a personal one. Frames: `list.upserted`
(created, renamed), `list.deleted`, `list.columns_changed` (the columns as they now are;
`items_rewritten` when cells were converted or dropped, so the client refetches items),
`list.views_changed` (the views as they now are), `list.item_upserted` (the row as written),
`list.item_deleted`, `list.comment_upserted` and `list.comment_deleted`. A deleted channel takes its
lists with it; retention's message window also removes list items not edited within it,
comments older than it, and deleted lists; a workspace export writes `lists.jsonl` (each list
with its columns and comments) and every list as `lists/<id>.csv`, a DM's list and personal
lists only when DMs are included.

Every item has a comment thread (`list_item_comments`, at most 4,000 characters a comment,
oldest first). Commenting takes the right to edit the list; a comment is edited by its author
and deleted by its author or a channel moderator; `list_items.comment_count` moves in the same
transaction, so the row's `list.item_upserted` carries the new count. A comment's `@[Name](id)`
mentions and a `person` cell set to someone new (not to whoever set it) stage
`list.comment_upserted` (with `mentioned_user_ids`) and `list.assigned`; the notifications
consumer turns them into a `list` notification — "Ana mentioned you on *Fix login*", "Ana
assigned you to *Fix login*" — for whoever named can read the list, with `list_id` and
`item_id` in its data and in the `notification` frame. One `date` column a list can be the
due date (`PATCH /list-columns/:id { due }`, 422 for any other kind). `list_due_assignments()`
gives every open item (its first checkbox column unchecked) with a due date by tomorrow in the
assignee's own time zone, for every person cell, if that person can still read the list; an
item due today or tomorrow becomes a Later reminder (`later_items.list_item_id`, 09:00 on the
day in their zone), kept in step when an item of that list is written and every ten minutes by
the reminder checker, which also removes one that is no longer due, assigned or open.
Completing it in Later checks the item's first checkbox column, when the person can still edit
the list.

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/workspaces/:ws_id/lists` | Query: `channel_id?` | `{ data: List[] }` — the caller's personal lists and those in conversations they can read; with `channel_id`, that conversation's |
| POST | `/workspaces/:ws_id/lists` | `{ title, description?, channel_id?, columns?: [{ name, kind, options? }] }` | `ListDetail` (`{ list, columns, views }`) |
| GET | `/lists/:list_id` | — | `ListDetail` |
| PATCH | `/lists/:list_id` | `{ title?, description? }` | `List` |
| DELETE | `/lists/:list_id` | — | `{ status: "deleted" }` — creator or channel moderator |
| GET | `/lists/:list_id/items` | — | `{ data: ListItem[] }` in rank order |
| POST | `/lists/:list_id/items` | `{ values?, after_id? }` — at the end unless `after_id` | `ListItem` |
| POST | `/lists/:list_id/columns` | `{ name, kind, options? }` | `ListColumn` |
| PUT | `/lists/:list_id/column-order` | `{ column_ids }` — every column once | `{ data: ListColumn[] }` |
| PATCH | `/list-columns/:column_id` | `{ name?, kind?, options?, hidden?, due?, confirm_cleared? }` | `ListColumn` — 409 when cells would be cleared unconfirmed |
| POST | `/list-columns/:column_id/preview` | `{ kind, options? }` | `{ converted, cleared }` |
| DELETE | `/list-columns/:column_id` | — | `{ status: "deleted" }` — its cells go too |
| PATCH | `/list-items/:item_id` | `{ values }` — only the cells named | `ListItem` |
| POST | `/list-items/:item_id/move` | `{ after_id }` — `null` moves it to the top | `ListItem` |
| DELETE | `/list-items/:item_id` | — | `{ status: "deleted" }` |
| POST | `/lists/:list_id/views` | `{ name, kind, config? }` | `ListView` — 422 for a rule its column cannot take |
| PUT | `/lists/:list_id/view-order` | `{ view_ids }` — every view once | `{ data: ListView[] }` |
| PATCH | `/list-views/:view_id` | `{ name?, kind?, config? }` — without `config`, a kind change keeps the rules that still fit | `ListView` |
| DELETE | `/list-views/:view_id` | — | `{ status: "deleted" }` — 422 for a list's last view |
| GET | `/list-items/:item_id/comments` | — | `{ data: ListComment[] }`, oldest first |
| POST | `/list-items/:item_id/comments` | `{ content }` | `ListComment` |
| PATCH | `/list-comments/:comment_id` | `{ content }` — the author only | `ListComment` |
| DELETE | `/list-comments/:comment_id` | — | `{ status: "deleted" }` — the author or a channel moderator |

---

### workflows

A workflow is a trigger and up to 20 steps, stored as a typed `Definition` in
`workflows.definition` and checked on every save: `models.rs` for the shape and the variables
each trigger offers, `service::check_authority` for what the builder may do (see the channel
a trigger listens to, post in the channel, message the person, a real timezone). `runner.rs`
is the `workflow_runner` loop in `chat-worker`: every 5 seconds it queues the schedules that
are due and runs queued runs. `listener.rs` is `workflow_listener`, consumer group `workflows`
on the workspace streams: a `reaction.added` or `channel.member_joined` event queues a run for
every enabled workflow listening to that channel (and emoji), with the person, channel and
message in the run's `context`. `workspace::membership::add_to_channel` and the default-channel
joins stage `channel.member_joined` with the membership row.

A `shortcut` trigger is started by a person who can post in its channel, from
`POST /workflows/:id/start` or the built-in `/workflow <name>`. A `form` step posts a card
(`metadata.form`) in the DM between the builder and its recipient, or in the trigger channel
when the recipient is the builder, writes a `workflow_forms` row and leaves the run `waiting`,
which the runner does not claim. `service::submit_form` checks the answers against the stored
fields, merges them into `context.form`, records the step done and sets the run `pending`
again, in one transaction with the card's `submitted_at` and its `message.updated`. A later
step inserts `{{form.<name>}}`; a person answer is a mention and notifies, a text answer does
not. The tick also expires forms past `expires_at` (7 days): the run becomes `expired` and the
card `expired`. A run resumed after its workflow was edited stops.

`add_list_item` writes an item into a list its builder can write to (`lists::service::writable`)
and `update_list_item` sets person, select or checkbox columns on the item an earlier step
added (`from_step`), found through `workflow_run_steps.list_item_id`. A cell is a template:
`cells::check` validates it against its column when saving (a static value parsed as the
column would, a typed variable — `{{trigger.user}}`, a person, date or choice answer — only
where it fits, any text in a text or link column) and `cells::value` turns the rendered text
into the stored value at run time, failing the step with the column's reason. The write is
`lists::service::create_item_in` / `update_item_in` on the step's transaction, followed by the
returned `ItemWrite::announce`, so list frames, assignment notifications and the due-date sync
are the ones a hand edit gives.

A run is a row in `workflow_runs`, unique on `(workflow_id, trigger_key)` — the schedule
instant, the webhook request or the stream event id — so a trigger delivered twice is one run.
Every trigger shares one limit of 30 runs a minute per workflow (`RUNS_PER_MINUTE`); an event
past it is stored as a failed run saying it was skipped. Each step's message
commits with its `workflow_run_steps` row, and a step already recorded is skipped when a run
is picked up again. A step authorizes as the builder at that moment; the first that fails
stops the run with `Step N (…): why`. A builder who is no longer a member turns their
workflows off (`workspace::membership::detach` does it on removal, a run does it if it finds
them gone) and every admin gets a `system` notification.

Messages are bot messages (`metadata.bot = { workflow_id, name }`) whose `user_id` is the
builder. A step with *summary* appends the form answers to its text as Markdown, which stays
the content for search, notifications and other clients, and stores them as `metadata.fields`
(`[{ label, value }]`, a person as a mention) for the web client to lay out as a grid. Only mentions in the builder's text and `{{trigger.user}}` notify, not ones inside
webhook values or the reacted message.

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/workspaces/:ws_id/workflows` | — | `{ data: Workflow[] }` — yours; an admin's has everyone's |
| POST | `/workspaces/:ws_id/workflows` | `{ name, description?, definition }` | `Workflow`, on at once (`next_run_at` set for a schedule) — a builder (no guests; admins only when `settings.workflow_builders` is `admins`) |
| GET | `/workflows/:workflow_id` | — | `Workflow` — its builder or an admin |
| PUT | `/workflows/:workflow_id` | as POST | `Workflow`, `version` + 1, on or off as it was — its builder only |
| POST | `/workflows/:workflow_id/enable` | — | `Workflow`, `enabled`, `next_run_at` for a schedule — its builder only, re-checked |
| POST | `/workflows/:workflow_id/disable` | — | `Workflow` — its builder or an admin |
| POST | `/workflows/:workflow_id/owner` | — | `Workflow`, now the caller's and off, with a new `webhook_url` — a workspace admin, only when `creator_gone` (its builder left the workspace or is suspended; 409 otherwise) |
| POST | `/workflows/:workflow_id/rotate` | — | `Workflow` with a new `webhook_url`; the old one answers 401 — its builder or an admin; 409 for a workflow not started by a webhook |
| DELETE | `/workflows/:workflow_id` | — | `{ status: "deleted" }` — its builder or an admin |
| GET | `/workflows/:workflow_id/runs` | — | `{ data: WorkflowRun[] }`, the last 50, newest first, each with its steps |
| POST | `/workflows/:workflow_id/start` | header `Idempotency-Key` optional | 202 `{ status: "accepted", run_id }` — a shortcut, by someone who can post in its channel; 404 otherwise, 409 turned off, 429 above 30 a minute |
| GET | `/channels/:ch_id/workflow-shortcuts` | — | `{ data: Shortcut[] }` — the enabled shortcuts of a channel you can post in |
| POST | `/workflow-runs/:run_id/forms/:position` | `{ answers: { [name]: string } }` | `{ status: "submitted" }` — its recipient only (403); 422 a bad answer, 409 answered, expired or no longer waiting |
| POST | `/workflows/webhook/:token` | any JSON object; header `Idempotency-Key` optional | 202 `{ status: "accepted", run_id }`; 401 unknown token, 409 turned off, 429 above 30 a minute |

The webhook route is public, authenticated by the token in `Workflow.webhook_url`. Audit
actions: `workflow.created`, `.updated`, `.enabled`, `.disabled`, `.deleted`, `.taken_over`.
A run checks its builder first (`authz::still_acting`): one who is suspended or gone turns the
workflow off and tells the workspace admins, and one who can no longer read the trigger's
channel fails the run.

---

### integrations (apps)

Per-workspace connected apps — Jira Cloud, Confluence, GitHub, Google Drive and Figma — and each person's own connection to them.
An admin stores the app's OAuth client (`workspace_apps`, secret sealed); a member connects
through OAuth 2.0 with PKCE, the pending `state` and verifier held in Redis for ten minutes
and readable once (`GETDEL`), bound to the member who started it. Tokens live sealed in
`app_connections`, keyed to the workspace membership so leaving the workspace removes them,
and are refreshed when they are within a minute of expiry. The unfurl consumer asks
`integrations::service::unfurl` first: a Jira issue link yields an app card fetched with the
**poster's** connection, a prompt when the poster has none, or nothing when the app is off. A
GitHub pull request, issue, commit or repository link uses the poster's connection when there
is one, otherwise GitHub's anonymous API for public repositories (cached ten minutes, shared
by everyone because it is public), and only then prompts. `/pull/N` that is really an issue,
and `/issues/N` that is really a pull request, get the right card. A Google Docs, Sheets,
Slides, Forms, Drawings or Drive link needs the poster's connection, scoped to
`drive.metadata.readonly`. A Figma file needs the poster's Figma connection
(`file_metadata:read`, `current_user:read`; client credentials go as HTTP Basic). A
Confluence page or blog post needs the poster's **Confluence** connection — its own
provider on the same Atlassian OAuth client, so a Jira connection never reads Confluence —
and its card joins the page with its space and the owner's and last editor's names
(`users-bulk`), fetched in parallel. A poster who chose *Never ask me again* for an app
(`app_prompt_optouts`) gets no prompt and the OpenGraph preview instead.

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/workspaces/:ws_id/apps` | — | `{ data: AppStatus[] }` — setup values (callback URL, scopes, steps), the caller's own connection and whether they turned the app's prompts off |
| PUT | `/workspaces/:ws_id/apps/:provider` | `{ client_id, client_secret?, enabled, reuse_from? }` | `{ status: "saved" }` — admin; the secret is required the first time. `reuse_from: "jira"` (Confluence only) copies Jira's client ID and sealed secret server-side |
| DELETE | `/workspaces/:ws_id/apps/:provider` | — | `{ status: "removed" }` — admin; disconnects everyone |
| POST | `/workspaces/:ws_id/apps/:provider/connect` | `{ return_to? }` | `{ authorize_url }` |
| POST | `/apps/connections` | `{ code, state }` | `{ workspace_id, provider, external_account, return_to }` |
| DELETE | `/workspaces/:ws_id/apps/:provider/connection` | — | `{ status: "disconnected" }` |
| PUT | `/workspaces/:ws_id/apps/:provider/prompts` | — | `{ status: "off" }` — *Never ask me again*: the caller's links to that app fall back to the OpenGraph preview; connecting turns prompts back on |
| DELETE | `/workspaces/:ws_id/apps/:provider/prompts` | — | `{ status: "on" }` |
| POST | `/messages/:msg_id/unfurls/refresh` | — | `{ status: "refreshing" }` — author; re-runs the previews |

### bots

Apps a workspace builds for itself, each with a bot user and a token; the guide for whoever
writes one is [bot-api.md](./bot-api.md).

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/workspaces/:ws_id/bot-apps` | — | `{ data: BotApp[] }` — any member |
| POST | `/workspaces/:ws_id/bot-apps` | `{ name, description?, icon_url?, scopes }` | `BotApp & { token }` — admin; 409 for a name the workspace already has |
| GET | `/bot-apps/me` | bot token | `BotApp` — the app the token belongs to |
| GET | `/bot-apps/:app_id` | — | `BotApp` — any member of its workspace |
| PATCH | `/bot-apps/:app_id` | `{ name?, description?, icon_url?, scopes? }` | `BotApp` — admin; the bot user's name and photo follow |
| POST | `/bot-apps/:app_id/rotate` | — | `BotApp & { token }` — admin; the old token stops at once |
| DELETE | `/bot-apps/:app_id` | — | `{ status: "deleted" }` — admin |

Creating an app inserts a `users` row with `is_bot = true`, an address under `bots.invalid`,
no password and status `active`, makes it a `member` of the workspace, and stores the app in
`bot_apps` with the SHA-256 of a `cbot-` token. `find_by_email` and the instance's user list
leave bots out, so nothing that starts from an address (sign-in, password reset, SSO, SCIM,
invites) can reach one, and the mention digest skips them. Removing the app deletes the row
(and with it, by `ON DELETE CASCADE`, every hook whose `app_id` is the app), suspends the bot
user and takes it out of the workspace and its channels.

**A bot token is refused everywhere by default.** `auth_middleware` resolves a `cbot-` bearer
token through `BotRepo::authenticate` (the workspace must not be deleted) and puts an
`AuthUser` with `bot: Some(BotToken { app_id, workspace_id, scopes })` in the request. The
`AuthUser` extractor then answers 403 for it, so a handler accepts a bot only by taking
`bots::Caller` instead and calling `caller.scoped(Scope::…)`, which is a no-op for a person
and a 403 naming the scope for a bot without it. Those handlers carry `tags = [.., "bot"]` and
a `bot` security requirement listing the scope, and `every_route_a_bot_may_call_names_its_scope`
keeps the two together. The history routes pick the scope after loading the channel:
`channels:history` for a public one, `groups:history` for anything else.

`WorkspaceMember.is_bot` makes `ChannelAccess::requires_explicit_membership` true for a bot, so
it reads and writes only where it is a channel member. `authz::require_channel_post_as` is the
one exception: with `chat:write.public` a bot posts in a public channel it has not joined.
`bots::audit_bot_writes` (inside `protected`, after the rate limit) writes `bot_app.called`
with the method and route template for every successful write a bot token makes. Writes count
against the person budgets keyed by the bot user; reads by a bot token add a `bot-read` budget
of 300 a minute.

### sidebar

One person's sidebar layout in one workspace: their own sections, which channel sits in
which, and which of the built-in sections (`channels`, `dms`, `people`) they keep collapsed.
Nobody else ever sees it, and every query is scoped to the caller, so another member's
section id is simply not found. A channel's section lives on its `channel_members` row, so
leaving a channel forgets it and deleting a section returns its channels to *Channels*.

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/workspaces/:ws_id/sidebar` | — | `{ sections, placements, collapsed_builtins }` |
| POST | `/workspaces/:ws_id/sidebar/sections` | `{ name }` | `SidebarSection` — 1–80 characters, unique per person ignoring case (409), at most 50 |
| PATCH | `/workspaces/:ws_id/sidebar/sections/:section_id` | `{ name?, collapsed? }` | `SidebarSection` |
| DELETE | `/workspaces/:ws_id/sidebar/sections/:section_id` | — | `{ status: "deleted" }` |
| PUT | `/workspaces/:ws_id/sidebar/order` | `{ section_ids }` | `{ status: "reordered" }` |
| PUT | `/workspaces/:ws_id/sidebar/builtins` | `{ section, collapsed }` | `{ status: "saved" }` |
| PUT | `/channels/:ch_id/section` | `{ section_id \| null }` | `{ status: "moved" }` — the caller must be a channel member and own the section |

---

### admin

Instance-level administration. Requires `is_instance_admin = true`.

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/admin/health` | — | `{ status, service, version }` |
| GET | `/admin/stats` | — | `{ users, workspaces, messages, files }` |
| POST | `/admin/email/test` | — | `{ sent_to, error }`: sent now to the caller's address; `error` says in words why it did not go |
| GET | `/admin/users` | Query: `limit?, offset?` | `{ data: User[] }` |
| POST | `/admin/users/:user_id/suspend` | — | `{ status: "suspended" }` — also revokes sessions and app connections and turns off the person's hooks and workflows |
| POST | `/admin/users/:user_id/activate` | — | `{ status: "activated" }` — an account with no password and no SSO link goes back to `pending`, waiting for its invite; SCIM `active: true` does the same |
| PATCH | `/admin/users/:user_id/instance-role` | `{ is_instance_admin: bool }` | `{ is_instance_admin: bool }` |
| GET | `/admin/workspaces` | Query: `limit?, offset?` | `{ data: Workspace[] }` |
| DELETE | `/admin/workspaces/:ws_id` | — | `{ status: "deleted" }` |
| DELETE | `/admin/users/:user_id/data` | `{ hard_delete?: bool }` | `{ status: "erased", hard_delete, removed }` — either way the account is tombstoned, signed out and unlinked from SSO. `hard_delete` also removes what they wrote: their messages (a thread root someone else answered stays as an empty tombstone), every earlier version of them, the notifications that quote them, their reactions, poll votes, list comments, scheduled messages and *Later* items, and their files with the stored objects; `removed` counts `messages`, `edits`, `reactions`, `votes`, `comments`, `scheduled` and `files` |
| GET | `/admin/scim/tokens` | — | `{ data: ScimToken[] }` |
| POST | `/admin/scim/tokens` | `{ description? }` | `{ id, description, token }`, the token only here |
| POST | `/admin/scim/tokens/:id/rotate` | — | `{ id, description, token }`; the old one is revoked |
| DELETE | `/admin/scim/tokens/:id` | — | `{ status: "revoked" }` |

`suspend` revokes the user immediately: it deletes their refresh tokens, sets a
Redis revocation flag the auth middleware checks (so an unexpired access token is
rejected), and closes their live WebSockets; `activate` clears the flag. Workspace
delete is a soft-delete (reversible via restore), audited in the same transaction.

---

### retention and export

| Method | Route | Who | Output |
|--------|-------|-----|--------|
| GET | `/workspaces/:ws_id/retention` | admin | `{ policy }`, `null` for keep-everything |
| PATCH | `/workspaces/:ws_id/retention` | owner | `{ policy }`; the whole policy, a missing period is *forever* |
| GET | `/workspaces/:ws_id/retention/preview` | owner | Query `message_days?, file_days?, channel_overrides?` → `{ messages, files }` past those periods now |
| GET | `/channels/:ch_id/retention` | channel access | `{ overrides_allowed, workspace_message_days, channel }` |
| PUT | `/channels/:ch_id/retention` | channel moderator | `{ message_days }`, `null` for forever; 409 while the owner has overrides off |
| DELETE | `/channels/:ch_id/retention` | channel moderator | back to the workspace's period |
| GET | `/workspaces/:ws_id/exports` | admin | `{ data: { export, download_url }[] }`, newest 50 |
| POST | `/workspaces/:ws_id/exports` | owner | `{ export }`, queued |
| POST | `/users/:user_id/exports` | self or instance admin | `{ export }`, queued |
| GET | `/exports/:id` | requester or instance admin | `{ export, download_url }` |

The list holds the workspace's exports and the person exports the caller started for its
members, which belong to no workspace. `download_url` is there only for whoever asked for the
export (or an instance admin) and only while the single-use link is unspent and unexpired.
`files.jsonl` follows the same choices as `messages.jsonl`: without `include_dms` it leaves out
files posted in DMs, and `since`/`until` bound files by their upload time. Edits, reactions, polls and votes follow their message's date. Each workspace's
purge is audited as one `retention.purge` row with that workspace's counts; the run's total
goes only to the log and the metrics.
A channel's period change posts a message in the channel and an audit row; direct messages
keep the workspace's period.

---

### huddle

Live voice/video rooms (Slack-style huddles) over mesh WebRTC. Live membership and media signaling run over `chat-realtime`; this REST surface covers what the browser needs from the API.

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/workspaces/:ws_id/ice-servers` | — | `{ ice_servers: IceServer[], ttl: number }` |
| GET | `/workspaces/:ws_id/active-huddles` | — | `{ data: { huddle_id, channel_id, initiator_id }[] }` — channels the caller is in only |
| POST | `/workspaces/:ws_id/huddles` | `{ channel_id }` XOR `{ dm_partner_id }` | `{ huddle_id }` — a DM partner must be someone the caller could message (a guest: someone they share a channel with), else 403 |
| POST | `/workspaces/:ws_id/huddles/:huddle_id/invite` | `{ user_ids: string[] }` | `{ status: "ok" }` — 404 unless the huddle is open in this workspace, 403 unless the caller is in it; an invitee the caller could not message is skipped |

`IceServer` is the WebRTC `RTCIceServer` shape: `{ urls: string[], username?, credential? }`. STUN entries are always returned; a TURN entry with time-limited credentials (TURN REST API, `username = "<expiry-unix>:<user-id>"`, `credential = base64(hmac_sha1(TURN_SECRET, username))`) is added only when `TURN_SECRET` and `TURN_URLS` are configured. See the coturn service in `docker-compose.yml` and the TURN section of `.env.example`.

**Start** generates a `huddle_id`, persists a `huddle_sessions` row, publishes `huddle.started`, and (for channels) posts a `metadata.kind="huddle_started"` system message; DM huddles also publish `huddle.ring` to the partner. **Invite** records a `huddle_invites` row for each workspace-member invitee, which is what lets them join, and publishes `huddle.ring`. **Active-huddles** returns currently-live huddles in channels the caller is a member of — open DB sessions (`ended_at IS NULL`) intersected with live Redis room membership (`SCARD huddle:{id}:members > 0`), so abrupt-drop sessions that never emitted `huddle.ended` are excluded. The frontend fetches it on workspace load and on WS reconnect to backfill the channel huddle banner (so late-joiners see "Join" and stale banners self-heal). Live membership/media is ephemeral — see the `events:huddle` WS surface below. Session/participant history is persisted by the API's huddle consumer, which also emits `huddle.ended` when the last participant leaves; ring/invite also raise a `Call` notification (DND-respecting). A ring to somebody who has paused the workspace carries `quiet: true`: the gateway does not deliver it, and the `Call` row still lands in Activity.

---

## chat-realtime (WebSocket Gateway)

Single WebSocket endpoint. Validates the JWT on the upgrade handshake against `JWT_PUBLIC_KEYS`, so it holds nothing that can sign one, re-checks channel/workspace membership against the DB on every subscribe/join, then relays events from Redis to connected clients: the live pub/sub channels for latency, and the per-workspace stream for replay after a reconnect. Every frame it sends is a `ServerFrame` variant (`shared/events`), and the frontend's frame union is generated from that enum. The socket is also closed when the access token's `exp` passes, so a long-lived connection can't outlive its token.

The upgrade additionally checks the `Origin` against the configured CORS origins and rejects a token covered by a revocation record. A revocation stores the moment it happened rather than a boolean, so it invalidates every token issued up to that point while leaving a later sign-in working, and it can spare one named session (`except_jti`) — which is what lets "log out my other devices" keep the device you are typing on. `session.revoked` closes the matching live sockets with close code `4001` and a reason; suspending a user closes all of theirs. Alongside `/ws`, the gateway serves `/livez`, `/readyz` (DB + Redis + event-consumer liveness — it reports unhealthy if the consumer has stalled), and `/metrics` (Prometheus: connection count, consumer heartbeat age, events, backpressure drops). Presence is workspace-scoped: a client only sees the online roster and status changes for workspaces it shares. See [RUNBOOK.md](RUNBOOK.md) for ops.

**Connection:** `wss://<host>/ws` — the browser sends the `access_token` `HttpOnly` cookie on the upgrade automatically (no token in the URL).

**Incoming client messages (subscribe / join / typing):**

```json
{ "type": "subscribe",     "workspace_id": "..." }
{ "type": "channel.join",  "channel_ids": ["...", "..."] }
{ "type": "channel.leave", "channel_id": "..." }
{ "type": "typing.start",  "channel_id": "..." }
{ "type": "typing.stop",   "channel_id": "..." }
{ "type": "activity",      "active": true }
{ "type": "ping" }
```

**`channel.join` takes the whole set at once.** A client joins every channel it can see the
moment it connects, and one frame per channel is what the inbound flood guard (20/s, burst 40)
exists to stop — a workspace with more channels than that used to have its socket closed by
the act of arriving. One frame, one membership query for the set, capped at 1 000 ids so the
batch cannot become the flood it replaced.

**Presence follows what a connection subscribes to.** A connection is online in every
workspace its person belonged to when it opened, and `subscribe` adds the workspace it names:
it is marked online there before the `presence.batch` is read, the heartbeat keeps it fresh,
and a clean disconnect clears it. A workspace joined after the socket opened (accepting an
invite, creating one) shows the person online as soon as they switch to it, without a
reconnect.

**A connected person is `online` or `away`.** Away is either chosen (`users.manual_away`, set
with `PUT /users/me/presence` or `/away`, and kept until the person sets it back) or idle:
`last_active:user:{id}` holds the last moment any of their connections reported input
(`activity` frames, CS-095, renewed by the heartbeat while active), and eight minutes without
it — about ten after the last keystroke, since a client reports idle after two — shows them
away. Arriving counts as activity when there is none on record. Every connection works the
status out on connect, on each `activity` frame and on each heartbeat; `presence:status:{id}`
holds the last one announced, and `SET … GET` on it means a change is published once however
many connections or nodes notice it. `presence.recompute` (from the API) is worked out only by
a node holding one of the person's sockets. `presence.batch` carries each member's status and
`presence.changed` reaches the person's own connections too, so their other devices follow.

**Replay is bounded twice.** Each workspace stream keeps 10 000 entries and an hour
(`STREAM_MAXLEN`, the API's stream trimmer); an entry is about 0.8 KB, so a busy workspace
holds about 8 MB, which is the headroom the worker's consumer group needs when it falls
behind. A reconnecting client is replayed at most 1 000 events (`REPLAY_MAX_EVENTS`, under a
megabyte through its socket). A longer gap, or a position the stream has already trimmed,
answers `sync.refetch_required`, and the client refetches messages, threads, unread counts,
pins, the sidebar, Activity and *Later*.

**Huddle signaling (incoming client messages).** Mesh WebRTC uses this socket purely as the signaling channel — no media flows through the server. After membership is verified, the server relays via `events:huddle`:

```json
{ "type": "huddle.join", "huddle_id": "..." }
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

A join is authorised against the huddle's own `huddle_sessions` row, never against anything the client sends: it must be open, and the caller must be in its workspace and either in its channel, one of the two people of a DM huddle, or invited (`huddle_invites`). `offer`/`answer`/`ice` are relayed only to `to_user_id` (and only when both users are current room members); the rest broadcast to the room. On join the caller gets a `huddle.members` snapshot. A huddle holds at most eight people (`HUDDLE_MAX_PARTICIPANTS`), the mesh's ceiling: the check and the insert are one Lua script on the room's Redis set, so two people racing for the last place cannot both get it, and a ninth joiner gets `huddle.full { limit }` on their own socket and is not added. Someone already in the room rejoining after a reconnect always keeps their place. `huddle.member_left` is published only for a user the room actually held. Disconnect removes the user and, when a room empties, the API consumer emits `huddle.ended`.

**Outgoing events pushed to client** (live copies arrive on these pub/sub channels; the durable ones are also on `stream:ws:{id}` for replay):

| Redis channel | Event types |
|---------------|-------------|
| `events:message` | `message.created`, `message.updated`, `message.deleted`, `message.pinned` |
| `events:reaction` | `reaction.added`, `reaction.removed` |
| `events:notification` | `notification.push` |
| `events:conversation` | `conversation.created` — fanned out to the participants; a conversation's messages are ordinary `message.*` / `reaction.*` events on its channel id |
| `events:presence` | `presence.changed` (fanned out only to workspaces shared with the subject, the subject included), `presence.recompute` (worked out by the node holding the person's socket) |
| `events:typing` | `typing.indicator` |
| `events:workspace` | `workspace.deleted`, `workspace.restored` |
| `events:user` | `user.suspended` — consumed by the gateway to close the user's sockets (not pushed to clients) |
| `events:huddle` | `huddle.started`, `huddle.ended`, `huddle.ring`, `huddle.member_joined`, `huddle.member_left` — lifecycle; also consumed by the API for history + call notifications |
| `events:list` | `list.upserted`, `list.deleted`, `list.columns_changed`, `list.item_upserted`, `list.item_deleted` — to the list's channel, or to the owner of a personal list |
| `events:thread` | `thread.read_state` — sent only to the user whose thread state changed (a reply, `/read`, follow or unfollow), with its `following` and `unread_count` |
| `events:channel` | `channel.read_state` — sent only to the user whose read position moved (by `/read` or `/unread`), so their other tabs and devices update their badges; `channel.member_removed` becomes `channel.access_revoked` for the removed user; `channel.updated` (`is_archived`) goes to everyone in the channel when it is archived or unarchived |
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
