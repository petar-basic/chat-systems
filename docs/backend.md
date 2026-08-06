# Backend

Two binaries: **`chat-api`** (port 3000) and **`chat-realtime`** (port 3004), sharing
a Postgres database, a Redis instance, and the crates under `shared/`.

## Architecture & Rationale

### Why this stack

- **Rust + Axum.** A chat backend is mostly fan-out and connection handling; Rust gives
  predictable latency and memory with no GC pauses, and Axum is a thin, `tower`-based
  layer over `hyper` that composes middleware cleanly.
- **Two binaries, not one.** The REST API is request/response and **stateless**; the
  WebSocket gateway is long-lived and **connection-stateful**. Splitting them lets each
  scale and fail independently — you can run many api replicas and many realtime nodes.
- **Redis as the bus.** The api never talks to sockets directly. It `PUBLISH`es domain
  events to Redis; every realtime node runs a consumer and pushes to *its* locally
  connected clients. Result: an event from any api replica reaches sockets on every
  realtime node, with no sticky sessions. Redis also backs presence (TTL-keyed per node,
  self-healing) and rate-limit counters.
- **PostgreSQL + sqlx.** Compile-time-checked, fully parameterized queries; soft deletes,
  partial indexes, and a GIN full-text index for search. Migrations run automatically on
  api startup (`sqlx::migrate!`).

### Feature-modular layering

Every feature under `api/src/` (`auth`, `workspace`, `messaging`, `dm`, `files`, `hooks`,
`notifications`, `admin`, `huddle`) follows the same shape, with files added only where warranted:

| File | Responsibility |
|------|----------------|
| `routes.rs` | parse request, authorize the caller, delegate — no business logic, no SQL |
| `service.rs` | business logic / orchestration across repos |
| `repo.rs` | **all** SQL for the feature; the only place that touches the pool |
| `models.rs` | request/response and row types |
| `publisher.rs` / `consumer.rs` / `executor.rs` | Redis publish / background consumers / outbound execution |
| `storage.rs` | storage backend abstraction (files) |

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
- **Files.** A `FileStorage` trait abstracts local disk vs S3/MinIO; both serve downloads
  through the authenticated `/api/files/download` route, so the object store stays private
  and access is gated by workspace **and** channel membership.
- **Webhooks.** Outbound delivery is SSRF-hardened (scheme allow-list, DNS resolution with
  private/loopback/link-local/metadata-IP blocking, redirects disabled) and HMAC-signed.

### Testing

Integration tests live in `api/src/http_tests/` and `realtime/src/tests/`. Each
`#[sqlx::test]` provisions a fresh Postgres, runs migrations, and drives the full Axum
router via `tower::oneshot` — real middleware, real auth, real JSON — asserting the
authorization matrix per endpoint. Realtime tests use real Redis + Postgres and assert
the right frames reach the right subscribers.

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

---

### conversations

Direct and group messages share one model: a `direct` conversation is the two-person case of
the same rows that back a group, so read state, reactions and fan-out have a single shape.
Every route requires the caller to be a participant; creating one requires every invitee to be
a workspace member, and a conversation holds at most nine people.

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/workspaces/:ws_id/conversations` | — | `{ data: ConversationSummary[] }` — newest first, with `participant_ids` and the caller's `last_read_at` |
| POST | `/workspaces/:ws_id/conversations` | `{ participant_ids }` | `Conversation` — one other person returns the existing `direct` thread if there is one; more create a `group` |
| GET | `/conversations/:conv_id/messages` | Query: `limit=50, before?` (message id) | `{ data: ConversationMessage[], next_cursor }` |
| POST | `/conversations/:conv_id/messages` | `{ content, id? }` | `ConversationMessage` — `id` makes the send idempotent |
| POST | `/conversations/:conv_id/read` | — | `{ status: "ok" }` |
| PATCH | `/conversations/messages/:msg_id` | `{ content }` | `ConversationMessage` — author only |
| DELETE | `/conversations/messages/:msg_id` | — | `{ status: "deleted" }` — author only |
| POST | `/conversations/messages/:msg_id/reactions` | `{ emoji }` | `ConversationReaction` |
| DELETE | `/conversations/messages/:msg_id/reactions/:emoji` | — | `{ status: "ok" }` |

Mutations publish to `events:conversation` (`conversation.created`, `conversation.message.created`
/`.updated`/`.deleted`, `conversation.reaction.added`/`.removed`); every payload carries
`participant_ids`, and the realtime gateway pushes to exactly those users.

---

### scheduled

Messages queued for later delivery, aimed at exactly one channel **or** one conversation. The
author must be able to post to the target at scheduling time, and the send window is capped at
120 days out.

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/workspaces/:ws_id/scheduled-messages` | — | `{ data: ScheduledMessage[] }` — the caller's pending queue |
| POST | `/workspaces/:ws_id/scheduled-messages` | `{ channel_id? \| conversation_id?, content, send_at }` | `ScheduledMessage` |
| PATCH | `/scheduled-messages/:id` | `{ send_at }` | `ScheduledMessage` — author only, pending only |
| DELETE | `/scheduled-messages/:id` | — | `{ status: "canceled" }` — author only, pending only |

A background dispatcher ticks every 15s and claims due rows with
`UPDATE … WHERE id IN (SELECT … FOR UPDATE SKIP LOCKED) RETURNING *`, so several api replicas
can run it without delivering a message twice. Delivery reuses the normal send path — channel
messages expand mentions and publish `message.created`, conversation messages publish
`conversation.message.created` — and a failure is recorded on the row instead of retried.

---

----|-------|-------|--------|
| GET | `/workspaces/:ws_id/dm` | — | `{ data: DmConversation[] }` |
| GET | `/workspaces/:ws_id/dm/:user_id` | Query: `limit=50, before?` | `{ data: DirectMessage[], next_cursor }` |
| POST | `/workspaces/:ws_id/dm/:user_id` | `{ content, id? }` | `DirectMessage` |
| POST | `/workspaces/:ws_id/dm/:user_id/read` | — | `{ status: "ok" }` |
| PATCH | `/workspaces/:ws_id/dm/:user_id/:msg_id` | `{ content }` | `DirectMessage` |
| DELETE | `/workspaces/:ws_id/dm/:user_id/:msg_id` | — | `{ status: "deleted" }` |
| POST | `/workspaces/:ws_id/dm/:user_id/:msg_id/reactions` | `{ emoji }` | `DmReaction` |
| DELETE | `/workspaces/:ws_id/dm/:user_id/:msg_id/reactions/:emoji` | — | `{ status: "ok" }` |

Edit/delete are author-only. Mutations publish to `events:dm` (`dm.created`, `dm.updated`,
`dm.deleted`, `dm.reaction.added`, `dm.reaction.removed`), fanned out to both participants.

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

Single WebSocket endpoint. Validates the JWT on the upgrade handshake, re-checks channel/workspace membership against the DB on every subscribe/join, then relays Redis pub/sub events to connected clients. The socket is also closed when the access token's `exp` passes, so a long-lived connection can't outlive its token.

The upgrade additionally checks the `Origin` against the configured CORS origins and rejects users carrying a revocation flag (e.g. just-suspended); suspending a user also closes their live sockets. Alongside `/ws`, the gateway serves `/livez`, `/readyz` (DB + Redis + event-consumer liveness — it reports unhealthy if the consumer has stalled), and `/metrics` (Prometheus: connection count, consumer heartbeat age, events, backpressure drops). Presence is workspace-scoped: a client only sees the online roster and status changes for workspaces it shares. See [RUNBOOK.md](RUNBOOK.md) for ops.

**Connection:** `wss://<host>/ws` — the browser sends the `access_token` `HttpOnly` cookie on the upgrade automatically (no token in the URL).

**Incoming client messages (subscribe / join / typing):**

```json
{ "type": "subscribe",     "workspace_id": "..." }
{ "type": "channel.join",  "channel_id": "..." }
{ "type": "channel.leave", "channel_id": "..." }
{ "type": "typing.start",  "channel_id": "..." }
{ "type": "typing.stop",   "channel_id": "..." }
{ "type": "ping" }
```

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

**Outgoing events pushed to client** (sourced from Redis pub/sub):

| Redis channel | Event types |
|---------------|-------------|
| `events:message` | `message.created`, `message.updated`, `message.deleted`, `message.pinned` |
| `events:reaction` | `reaction.added`, `reaction.removed` |
| `events:notification` | `notification.push` |
| `events:dm` | `dm.created`, `dm.updated`, `dm.deleted`, `dm.reaction.added`, `dm.reaction.removed` |
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
  → PostgreSQL write
  → Redis PUBLISH
  → chat-realtime event consumer
  → WebSocket PUSH to subscribed clients
```
