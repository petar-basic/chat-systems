# Roadmap & known limitations

What isn't done yet, why, and in what order it should be built. This is a living
document — it's here so the sharp questions have honest, specific answers instead of
hand-waving.

Every open item below is a ticket in [`docs/tickets/`](./tickets/INDEX.md), numbered in
execution order — the number is the schedule. A ticket's file is deleted when it ships;
what it changed lives in the git history and in the docs it touched. Read
[the index](./tickets/INDEX.md) for the dependency map and the conflict table; this page
is the summary and the reasoning behind the sequence.

**Wave 0 is shipped**, with one open tail:
[CS-005a](./tickets/CS-005a-workspace-role-not-populated.md), a product bug the new E2E
job found on its first run. Next planned ticket:
[CS-006](./tickets/CS-006-invite-lifecycle.md).

## How the order was chosen

1. **Wave 0 before anything else.** It installs the regression net (E2E in CI) and the
   three structures the rest depends on: one authorization module, one session-revocation
   path, and background workers in their own process. Half the fixes below are one-line
   changes *given* those, and duplicated plumbing without them.
2. **Access control before rate limits.** A limit on a leaking endpoint is a slower leak.
3. **Governance after access control.** An audit log is only worth having once the thing
   it audits is enforced.
4. **Correctness before performance.** And within performance, the renderer before
   virtualization — windowing a list of editor instances optimizes the wrong layer.
5. **Durable delivery after the worker split.** Both rework the same transport.
6. **Compliance before parity features.** Retention, export and SSO are what a security
   review asks for; custom emoji is not.

---

## Wave 0 — Safety net and structural groundwork ✅ shipped

Wave 0 is done. It is kept here because the rest of the roadmap is written against
the structures it introduced.

### [CS-001] Playwright suite runs in CI
The 12 E2E specs existed and CI never ran them. They now run in an `e2e` job that boots
the real stack with `docker compose --profile frontend`, seeds it, and uploads the
report, screenshots and container logs on failure. Two specs were unrunnable on a
runner — one hard-coded an absolute laptop path, another a MailHog URL — and were fixed
in the same change.

Turning it on immediately paid for itself. Three specs had rotted against the group-DM
rework: the DM picker became multi-select and nothing clicked "Start chat", the
`dm-message` hook was renamed to `conversation-message`, and a constant run stamp made
every message collide with the previous run's — which is also why the suite could not be
re-run against the same database. All three are fixed.

It also surfaced a real product bug, [CS-005a](./tickets/CS-005a-workspace-role-not-populated.md):
role-gated controls disappear when the workspace role is not populated. The specs that
catch it are correct as written, so the `e2e` job runs but **does not block merges** until
that ticket closes.

### [CS-002] Central authorization module
`require_workspace_member` had three separate implementations, `require_channel_access`
two, and the role gate two more. All of it now lives in
[`backend/api/src/authz.rs`](../backend/api/src/authz.rs), and `ChannelAccess` carries
the decision so a caller can ask about visibility instead of restating the rule in SQL.

Consolidating surfaced a real drift: the huddle copy of `require_channel_access` had no
guest clause, so a guest could start a huddle in a public channel they were not a member
of. The merged predicate is the strict one.

### [CS-003] Session revocation primitive
`sessions::revoke` is now the only way a session ends: refresh tokens deleted, access
tokens marked invalid, live sockets closed with a reason. A revocation stores the moment
it happened rather than a boolean — a flat flag would have blocked the user from signing
back in with their new password, which is what `CS-008` is about to need. `SessionScope`
supports sparing one named session so "log out my other devices" keeps the device you
are typing on.

### [CS-004] Background workers split out
Every `chat-api` process used to spawn four Redis pub/sub consumers, so a second API
replica meant duplicate notifications, duplicate webhook deliveries and duplicate
reminders — the API tier could not be replicated despite being stateless. They now run
in `chat-worker`, one replica by contract. Two safety nets underneath: a partial unique
index makes a duplicate mention notification impossible, and reminders are claimed with
`FOR UPDATE SKIP LOCKED` the way scheduled messages already were.

### [CS-005] Compile-time-checked queries — decided
Adopt `sqlx::query_as!` for new code, convert existing runtime queries opportunistically,
no big-bang rewrite. `SQLX_OFFLINE=true` in the image build and
`cargo sqlx prepare --check` in CI are the guardrails. Rationale and the contributor
rule are in [CONTRIBUTING.md](./CONTRIBUTING.md#backend-rust).

## Wave 1 — Access control

### [CS-006](./tickets/CS-006-invite-lifecycle.md) — Invite lifecycle
**Today:** `create_invite` sets neither `max_uses` nor `expires_at`, `claim_invite_use_tx`
treats `NULL` as unlimited, and `accept_invite` never compares the invite's email to the
accepting account. Every invite ever issued is an unlimited, non-expiring, transferable
key to the workspace at whatever role it carries.

### [CS-007](./tickets/CS-007-membership-revocation.md) — Membership removal reaches the socket
**Today:** the gateway checks channel membership only at `channel.join`; removal publishes
no event. Somebody removed from a private channel keeps reading it until their socket
closes — bounded by the access-token deadline, one hour by default. Separately,
`remove_member` deletes only the `workspace_members` row, so `channel_members` survives and
re-adding a person silently restores every private channel they were ever in.

### [CS-008](./tickets/CS-008-revoke-on-password-change.md) — Revoke on password change
**Today:** reset and change delete refresh tokens but never revoke access tokens or close
sockets. After a user resets a compromised password, the attacker keeps read and write
access for up to an hour.

### [CS-009](./tickets/CS-009-conversation-attachment-access.md) — DM attachment access control
**Today:** `link_attachments` is called only from the messaging feature, so files posted in
DMs keep `message_id = NULL` forever — and `require_file_access` with a null owner requires
only workspace membership. The unguessable storage key is the only thing protecting every
DM attachment in the instance.

### [CS-010](./tickets/CS-010-guest-search-scoping.md) — Guest scoping in search
**Today:** `require_channel_access` restricts guests to channels they explicitly belong to;
the search query re-derives visibility in SQL and drops that clause. A guest reads via
`/api/search` what they are refused via `/channels/:id/messages`.

---

## Wave 2 — Abuse and resource limits

### [CS-011](./tickets/CS-011-streaming-upload.md) — Streaming upload with an enforced cap
**Today:** the whole multipart field is read into memory before the size check, so the check
never rejects anything the 100 MiB router limit accepted. The production API container is
capped at 512 MB.

### [CS-012](./tickets/CS-012-write-rate-limit-coverage.md) — Rate limit every mutating router
**Today:** the write limiter is attached to `messaging` and `files` only. Sending direct
messages, creating invites, channels and workspaces are unlimited. Per-router opt-in is the
bug.

### [CS-013](./tickets/CS-013-fail-closed-rate-limit.md) — Fail closed on auth paths
**Today:** the limiter allows the request when Redis errors. Correct for message sending,
wrong for login — a Redis outage silently removes brute-force protection.

### [CS-014](./tickets/CS-014-ws-inbound-rate-limit.md) — WebSocket inbound limits
**Today:** the socket message loop has no throughput limit and most arms hit the database.
One client can saturate the connection pool with `typing.start`.

### [CS-015](./tickets/CS-015-incoming-hook-ip-limit.md) — Per-IP limit for incoming webhooks
**Today:** the only unauthenticated mutating endpoint keys its limiter on the URL token, so
varying the token gives a fresh bucket per request — each of which still queries Postgres.

---

## Wave 3 — Authentication hardening

### [CS-016](./tickets/CS-016-uniform-login-failure.md) — Uniform login failure
**Today:** a pending account gets a distinct error message, and an unknown address returns
without running Argon2. Account enumeration by message and by timing.

### [CS-017](./tickets/CS-017-auth-transport-defaults.md) — Password policy and mail transport
**Today:** password validation is length only (8–128); `SMTP_USE_TLS` defaults to false and
the false branch uses `builder_dangerous`, sending credentials in cleartext to whatever
host is configured.

---

## Wave 4 — Governance

### [CS-018](./tickets/CS-018-audit-log-coverage.md) — Audit log coverage
**Today:** the table has every column an audit trail needs and is written from three call
sites — suspend, activate, hook reveal. Nothing records message or channel deletion, member
removal, role changes, invites, file access or login. `ip_address` is never populated, and
there is no way to read the log.

### [CS-019](./tickets/CS-019-scope-outgoing-webhooks.md) — Scope outgoing webhooks
**Today:** an outgoing webhook fires on every `message.created` in the workspace with no
channel filter, so one hook created by any workspace admin streams every private channel to
an external URL, invisibly to the people in them. The transport itself — SSRF validation,
no redirects, HMAC, bounded retries — is sound; the scope is not.

### [CS-020](./tickets/CS-020-file-moderation.md) — File moderation and lifecycle
**Today:** only the uploader can delete a file, so an admin cannot remove a leaked
credential or an inappropriate image. Deleting a message leaves its attachment readable.

---

## Wave 5 — Correctness

### [CS-021](./tickets/CS-021-scheduled-reauthorize.md) — Re-authorize at delivery
**Today:** the scheduled dispatcher posts without checking the author still has access.
After CS-007, this is the one path that still writes on behalf of a removed user.

### [CS-022](./tickets/CS-022-scoped-idempotency-id.md) — Scope the client-supplied message id
**Today:** the DM send path accepts a client-chosen id and, on a unique violation, returns
whatever message holds that id — without checking it belongs to the conversation.

### [CS-023](./tickets/CS-023-validation-gaps.md) — Remaining validation gaps
**Today:** reaction emoji, reminder content, channel topic and description reach the
database unvalidated. Over-length input surfaces as 500 rather than 400.

---

## Wave 6 — Performance

### [CS-024](./tickets/CS-024-static-message-renderer.md) — Static message renderer
**Today:** `RichTextDisplay` calls `useEditor` **per message**, so every rendered message
mounts a full TipTap/ProseMirror instance to display text that is never edited. This is the
dominant cost in the message list, and the reason virtualization alone is not the fix.

### [CS-025](./tickets/CS-025-virtualize-message-list.md) — Virtualize the message list
**Today:** `MessageList` renders every loaded message. Worth doing once each row is cheap.

### [CS-026](./tickets/CS-026-unread-counts.md) — Unread counts without subqueries
**Today:** the channel-list query runs an `EXISTS` subquery per channel, and cannot show a
count at all.

### [CS-027](./tickets/CS-027-presence-without-scan.md) — Presence without a keyspace scan
**Today:** `get_online_users` runs `SCAN` over the entire Redis keyspace and is called on
every WebSocket subscribe. Cost grows with unrelated traffic.

---

## Wave 7 — Reliability

### [CS-028](./tickets/CS-028-durable-delivery.md) — Durable realtime delivery
**Today:** events fan out over Redis pub/sub with no backlog. A disconnected client recovers
by refetching open views (`frontend/src/lib/realtimeBackfill.ts`), so nothing is
permanently lost, but delivery over the socket is at-most-once and there is no gap replay.
Backpressure drops a slow client without a close frame, so it waits for the next heartbeat.

**Plan:** a Redis Stream per workspace, client-tracked stream ids, `XRANGE` replay on
reconnect before joining the live tail, consumer groups for the worker. Replayed events
must go through the same subscription filter as live ones.

---

## Wave 8 — Compliance

### [CS-029](./tickets/CS-029-message-edit-history.md) — Message edit history
**Today:** `update_message` mutates in place; only `updated_at` changes. The UI asserts that
an edit happened and cannot say what changed.

### [CS-030](./tickets/CS-030-retention-policies.md) — Retention and cleanup
**Today:** nothing is ever deleted. Consumed `password_reset_tokens`, expired refresh
tokens, hook execution payloads and soft-deleted rows accumulate for the life of the
instance. Partition `messages` / `audit_log` only once metrics show the need.

### [CS-031](./tickets/CS-031-data-export.md) — Workspace and user export
**Today:** no export path exists. `pg_dump` is a backup, not something you hand to a
regulator or a departing employee.

### [CS-032](./tickets/CS-032-sso-and-2fa.md) — SSO (OIDC) and 2FA
**Today:** email and password only, with no second factor even for the instance admin. The
`user_identities` table exists and is unused.

### [CS-033](./tickets/CS-033-scim-deprovisioning.md) — SCIM deprovisioning
**Today:** removing someone from the identity provider does nothing here. Small ticket, but
only because CS-003 and CS-007 build the primitives it composes.

---

## Wave 9 — Product parity

### [CS-034](./tickets/CS-034-search-language.md) — Search language and DM search
**Today:** `content_search` is a `GENERATED ALWAYS ... STORED` column pinned to
`to_tsvector('english', …)`, so the language is a schema decision, not a setting. Wrong
stemming and no diacritic folding for non-English teams. DMs are not searchable at all.

### [CS-035](./tickets/CS-035-web-push.md) — Web Push
**Today:** notifications arrive only while a window is open. Closed-app delivery needs a
service worker, VAPID keys and a sender in the worker.

### [CS-036](./tickets/CS-036-slack-import-export.md) — Slack import
**Today:** no import. A migrating company either abandons its history or keeps paying Slack
as an archive — and in practice that means the migration fails.

### [CS-037](./tickets/CS-037-huddle-sfu.md) — SFU for large huddles
**Today:** huddles use a WebRTC mesh, fine to six or eight participants. The `livekit_room`
column exists and is unused. Keep the mesh for small calls; switch above a threshold.

### [CS-038](./tickets/CS-038-mobile-client.md) — Mobile client
**Today:** desktop-first, mobile layout explicitly not a goal. The largest adoption risk on
this page: a chat tool that cannot reach people on a phone gets replaced in practice by
whatever can.

### [CS-039](./tickets/CS-039-remaining-parity.md) — Custom emoji, user groups, bots, slash commands
**Today:** `HookType::Bot` and `HookType::SlashCommand` are defined and unused; incoming
webhook messages are attributed to the admin who created the hook rather than to a bot
identity.

---

## What is deliberately not on this list

- **Partitioning `messages` / `audit_log`.** Right at a scale this instance is nowhere
  near. Revisit when a metric says so, not before.
- **Server-side huddle recording.** A compliance feature with its own consent and retention
  requirements; it does not belong inside CS-037.
- **Cyrillic ↔ Latin transliteration in search.** A separate feature from CS-034's
  diacritic folding, and it should not be smuggled in with it.
- **SCIM `/Groups`.** `/Users` delivers the whole offboarding value; groups are where SCIM
  implementations go to die. Add only on request.
