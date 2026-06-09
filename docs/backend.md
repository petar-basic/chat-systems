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
`notifications`, `admin`) follows the same shape, with files added only where warranted:

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
  endpoints and, via a per-user middleware, all authenticated write paths.
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

### auth

Handles user identity — login, registration, JWT, and profile.

**Input / Output:**

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| POST | `/auth/login` | `{ email, password }` | `{ access_token, refresh_token, expires_in }` |
| POST | `/auth/complete-registration` | `{ token, password, display_name }` | `{ access_token, refresh_token, expires_in }` |
| POST | `/auth/refresh` | `{ refresh_token }` | `{ access_token, refresh_token, expires_in }` |
| POST | `/auth/forgot-password` | `{ email }` | `{ status: "sent" }` |
| POST | `/auth/reset-password` | `{ token, password }` | `{ status: "reset" }` |
| GET | `/instance/info` | — | `{ name, icon_url, version }` |
| GET | `/users/me` | JWT header | `UserPublic` |
| PATCH | `/users/me` | `{ display_name?, avatar_url?, bio?, timezone? }` | `UserPublic` |

---

### workspace

Manages workspaces, members, invites, channels, and DMs.

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
| POST | `/workspaces/:ws_id/invites` | `{ email?, role }` | `WorkspaceInvite` |
| POST | `/invites/:token/accept` | — | `WorkspaceMember` |

**Channels:**

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/workspaces/:ws_id/channels` | — | `{ data: Channel[] }` |
| POST | `/workspaces/:ws_id/channels` | `{ name, channel_type?, description?, is_default? }` | `Channel` |
| GET | `/channels/:ch_id` | — | `Channel` |
| PATCH | `/channels/:ch_id` | `{ name?, topic?, description? }` | `Channel` |
| DELETE | `/channels/:ch_id` | — | `{ status: "archived" }` |

**Channel members:**

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| GET | `/channels/:ch_id/members` | — | `{ data: ChannelMember[] }` |
| POST | `/channels/:ch_id/members` | `{ user_id }` | `ChannelMember` |
| DELETE | `/channels/:ch_id/members/:user_id` | — | `{ status: "removed" }` |

**Direct messages:**

| Method | Route | Input | Output |
|--------|-------|-------|--------|
| POST | `/workspaces/:ws_id/dm` | `{ user_id }` | `Channel` (gets or creates DM) |

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
| GET | `/search` | Query: `q, channel_id?, user_id?, limit=20, offset=0` | `{ data: Message[] }` |

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
| DELETE | `/hooks/:hook_id` | — | `{ status: "deleted" }` |

Hook types: `incoming_webhook`, `outgoing_webhook`, `bot`, `slash_command`, `scheduled`

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
| GET | `/workspaces/:ws_id/notifications/unread-count` | — | `{ unread_count: number }` |

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

**Connection:** `wss://<host>/ws` — the browser sends the `access_token` `HttpOnly` cookie on the upgrade automatically (no token in the URL).

**Incoming client messages (subscribe/unsubscribe):**

```json
{ "type": "subscribe_workspace", "workspace_id": "..." }
{ "type": "subscribe_channel", "channel_id": "..." }
{ "type": "unsubscribe_channel", "channel_id": "..." }
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
| `events:message` | `message.created`, `message.updated`, `message.deleted` |
| `events:reaction` | `reaction.added`, `reaction.removed` |
| `events:notification` | `notification.created` |
| `events:workspace` | `workspace.updated`, `member.joined`, `member.left` |
| `events:huddle` | `huddle.started`, `huddle.ended`, `huddle.ring`, `huddle.member_joined`, `huddle.member_left` — lifecycle; also consumed by the API for history + call notifications |
| `events:huddle-signal` | `huddle.offer`, `huddle.answer`, `huddle.ice`, `huddle.mute`, `huddle.camera`, `huddle.screenshare`, `huddle.hand`, `huddle.reaction` — high-frequency relay, realtime-only (kept off `events:huddle` so the API consumers don't parse every ICE candidate) |

All events use the envelope: `{ id, event_type, payload, timestamp }`.

---

## Shared

### shared-common
- `AppError` — unified error type mapped to HTTP status codes (400, 401, 403, 404, 409, 500)
- CORS layer configuration
- Input validation helpers

### shared-events
- `Event<T>` envelope with `id`, `event_type`, `payload`, `timestamp`
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
