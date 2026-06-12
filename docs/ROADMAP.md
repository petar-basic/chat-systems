# Roadmap & known limitations

What isn't done yet, why, and how it would be built. Ordered roughly by impact
per effort. This is a living document — it's here so the sharp questions about a
reference codebase have honest, specific answers instead of hand-waving.

## Reliability

### Durable real-time delivery (at-least-once)
**Today:** events fan out over Redis pub/sub. A disconnected client recovers by
refetching open views on reconnect (`frontend/src/lib/realtimeBackfill.ts`), so
nothing is permanently lost, but there's no gap replay — delivery is at-most-once
over the socket. Backpressure on a slow client drops its connection (surfaced via
the `realtime_backpressure_drops_total` metric) and it reconnects + refetches.

**Plan:** a Redis Stream per workspace (`XADD events:{ws}`). Each client tracks the
last stream id it processed and sends it on reconnect; the gateway replays the gap
with `XREAD` before resuming live tail. Trim streams by length/age. This makes
delivery at-least-once with client-side idempotency on message id (already unique).
Also send a WebSocket close frame on backpressure drop so the client reconnects
immediately instead of waiting for the next heartbeat.

## Performance

### M7 — Unread counts without per-channel subqueries
**Today:** the channel-list query runs an `EXISTS` subquery per channel; at 50+
channels and growing history this adds up.
**Plan:** denormalize an `unread_count` onto `channel_members`, maintained on
message insert / mark-read, or a short-TTL Redis cache invalidated on insert, with
badge deltas pushed over the socket.

### M11 — Message list virtualization
**Today:** `frontend/src/features/messaging/MessageList.tsx` renders all loaded
messages; channels with thousands of messages degrade scroll/render.
**Plan:** windowed rendering (`@tanstack/virtual`) over the existing paginated
query; target sub-100ms re-render on large channels.

## Auditing & compliance

### M8 — Message edit history
**Today:** `update_message` mutates in place (only `updated_at` changes).
**Plan:** a `message_edits` table (`message_id`, `prev_content`, `edited_at`)
written in the same transaction as the update; expose prior versions to admins.

### Retention policies
Per-workspace retention with a nightly purge over messages/files/audit_log;
`password_reset_tokens` already want a cleanup job. Partition `messages` /
`audit_log` only once metrics show the need (don't pre-optimize).

## Features (Slack parity gaps)

- **Group DMs (3–9).** The DM schema is hard-wired to pairs (`LEAST/GREATEST`).
  Generalize to a `conversations` + `conversation_participants` model that subsumes
  1:1; pragmatic stopgap is an unnamed private channel.
- **Slack import / export.** A CLI that ingests a Slack export ZIP (users→accounts
  by email, channels, per-day message JSON with `thread_ts`→`thread_parent_id`,
  files→MinIO) and a GDPR-style export the other way.
- **SSO (OIDC) + 2FA.** `openidconnect` for Google Workspace etc.; `totp-rs` for at
  least the admin account.
- **`@channel` / `@here` / `@everyone` + user groups.** Special mention types in the
  parser, fanned out to channel members.
- **Custom emoji.** `workspace_emojis` table + MinIO upload, registered in the picker.
- **Scheduled send.** A `scheduled_messages` table; the existing reminder checker is
  ~80% of the needed infrastructure.
- **Bots / slash commands.** `HookType::Bot` / `SlashCommand` are defined but unused;
  incoming webhooks already ship (`POST /api/hooks/incoming/:token`).

## Testing & polish

### M10 — Frontend test coverage
Component tests (RTL) for the composer / channel list / login flow, and a handful of
Playwright journeys (auth, workspace switch, thread, DM, file, mention) beyond the
current send/receive smoke test.

### Web Push notifications
The PWA shows notifications only while a window is open. Closed-app delivery needs a
service worker + Push API subscription with VAPID keys and a backend sender — the
natural next step now that the client ships as a PWA.

## Calls at scale

Huddles use a WebRTC **mesh**, which is fine to ~6–8 participants. Large calls would
need an SFU (e.g. LiveKit); until then, mesh + a documented participant ceiling.
