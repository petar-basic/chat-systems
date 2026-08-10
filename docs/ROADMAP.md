# Roadmap & known limitations

What isn't done yet, why, and in what order it should be built. This is a living
document — it's here so the sharp questions have honest, specific answers instead of
hand-waving.

Every open item below is a ticket in [`docs/tickets/`](./tickets/INDEX.md), numbered in
execution order — the number is the schedule. A ticket's file is deleted when it ships;
what it changed lives in the git history and in the docs it touched. Read
[the index](./tickets/INDEX.md) for the dependency map and the conflict table; this page
is the summary and the reasoning behind the sequence.

**Waves 0 through 3 are shipped.** Next ticket:
[CS-018](./tickets/CS-018-audit-log-coverage.md).

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

It also surfaced two real product bugs, both fixed. **CS-005a:** role-gated controls
disappeared when the workspace role was not populated — a channel admin could not open
channel settings, a workspace admin could not reach Integrations. The role was copied
into a Zustand store by effects, so its correctness depended on effect scheduling; it is
now derived from live query data on every render
([`useCurrentWorkspaceRole.ts`](../frontend/src/features/workspace/hooks/useCurrentWorkspaceRole.ts)),
and `wsQuerySync` computes it from the query cache instead of reading a mirror. The
second: the scheduled-message reschedule form closed on submit without awaiting the
mutation, so a failed save looked like a successful one.

The suite is green end to end and the `e2e` job blocks merges.

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

## Wave 1 — Access control ✅ shipped

### [CS-006] Invite lifecycle
Every invite ever issued was unlimited, never expired, and was not bound to the address
it was mailed to — one forwarded link was a permanent door at whatever role it carried.
Invites are now bounded by construction: an email invite is single-use, seven days, and
refuses any other account; a link invite must say how many people it is for (capped at
100) and cannot outlive a week. **The migration expires every outstanding invite** — there
is no way to tell a legitimate one from a leaked one, so admins re-send.

### [CS-007] Membership removal reaches the socket
The gateway checked channel membership only at subscribe time, so removing somebody from
a private channel did not stop delivery until their token expired. Removal now publishes
`channel.member_removed` / `workspace.member_removed`; the gateway drops the subscription
and tells the client, which closes the view and forgets what it cached. Workspace removal
also deletes the `channel_members` rows in the same transaction — returning to a workspace
grants nothing back, which is deliberate: re-entry to a private channel should be somebody's
decision, not a leftover row.

### [CS-008] Revoke on password reset and change
Both flows deleted refresh tokens and left access tokens alive, so recovering a
compromised account left the attacker an hour of access. Both now go through
`sessions::revoke`: a reset ends every session, a change ends every session **except the
one making the change**.

### [CS-009] Attachment access control for conversations
`link_attachments` only ever ran for channel messages, so every file sent in a DM kept a
null owner — and a null owner meant `require_file_access` asked for nothing but workspace
membership. Files now carry a `conversation_message_id`, linking moved into
`files::service` so both features share one implementation, and an unowned upload is
readable only by whoever uploaded it. The migration attributes existing DM attachments by
finding their storage key in the message that posted it, so participants keep the files
they already had.

### [CS-010] Guest scoping in search
`require_channel_access` holds guests to explicit channel membership; the search query
re-derived visibility in SQL and dropped that clause, so a guest could read via
`/api/search` what they were refused via `/channels/:id/messages`. The rule is now passed
down from `authz` instead of restated.

## Wave 2 — Abuse and resource limits ✅ shipped

### [CS-011] Streaming upload with an enforced cap
The whole multipart field was read into memory and *then* compared against the limit, so
the check never rejected anything the router had already accepted — and the production API
container is capped at 512 MB. Uploads now stream through an `UploadSink`: the local
backend writes straight to disk, S3 assembles the object from 8 MiB parts, and the byte
count is enforced as it goes. A refused or failed upload removes what it had already
written and leaves no row behind. The 100 MiB body limit now applies only to the upload
route; everything else is capped at 1 MiB, so a 100 MiB JSON body can no longer be posted
to `/api/auth/login`. `MAX_UPLOAD_BYTES` makes the cap configurable.

### [CS-012] Rate limit on every mutating route
The write limiter was attached to two of ten routers, so direct messages, invites,
channels and workspaces were unlimited. Authentication and the limiter are now applied
together by `crate::protected`, so a new feature cannot be wired up without one. The
budget follows the action rather than one global number: messages 120/min, reactions
240/min, invites 20/hour, workspaces 5/hour, channels 30/hour.

### [CS-013] Fail closed where it matters
The limiter swallowed Redis errors and allowed the request. That is right for sending a
message and wrong for `/auth/login`, where it was the only thing between an attacker and
unlimited guesses — a Redis outage silently removed brute-force protection. The policy is
now a property of the call site, auth paths return 503 rather than verifying the password,
and `rate_limit_backend_failures_total` makes the outage visible. `429` responses carry
`Retry-After`.

### [CS-014] Bounded WebSocket input
The socket loop had no throughput limit and most arms hit the database, so one client
could saturate the connection pool with `typing.start`. Frames are now drawn from a
per-connection token bucket, repeated typing for the same channel is coalesced instead of
republished, and frame size is capped. No membership caching — a cache in an
authorization path is what CS-005a was.

### [CS-015] Per-IP limit for incoming webhooks
The only unauthenticated mutating endpoint keyed its limit on the token from the URL, so
varying the token bought a fresh bucket per request and each one still cost a database
lookup. It is now bounded per source address before the database is touched. That address
is only taken from `X-Forwarded-For` when the request actually arrived from one of our own
proxies (`TRUSTED_PROXIES`) — previously the header was believed unconditionally, so a
caller could set it themselves and defeat the limit.

## Wave 3 — Authentication hardening ✅ shipped

### [CS-016] One answer for every login failure
An unknown address, a suspended account, an invited-but-unregistered account and a wrong
password each produced a different response — and the unknown-address path returned
without running Argon2 at all, so the instance's address book was recoverable with a
stopwatch. Every failure now returns the same status and body, and every attempt pays for
one verification against a placeholder hash when no real one exists. The real reason still
goes to the log. The login page carries a hint about unused invites that is shown to
everyone, so it tells an invited user what to do without telling anyone else whether an
account exists. Invalid and expired invite tokens are likewise indistinguishable.

### [CS-017] Mail transport defaults
SMTP gained `SMTP_TLS_MODE` (`starttls` / `implicit` / `none`), defaulting to STARTTLS for
remote hosts and plaintext only for a local catcher. Sending credentials in clear to a
remote relay now aborts startup; an open internal relay still works, with a warning.

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

- **A password policy beyond length.** `CS-017` originally proposed a 12-character
  minimum, a breach-list check and rejecting passwords containing the user's own name.
  Decided against on 2026-08-08: the project does not want to police what people choose,
  and the minimum stays at 8. The trade-off is explicit — until SSO lands (`CS-032`) the
  password is the entire authentication story, and nothing constrains it. Revisit if an
  instance is ever run for an organisation that needs to pass a security review.

- **Partitioning `messages` / `audit_log`.** Right at a scale this instance is nowhere
  near. Revisit when a metric says so, not before.
- **Server-side huddle recording.** A compliance feature with its own consent and retention
  requirements; it does not belong inside CS-037.
- **Cyrillic ↔ Latin transliteration in search.** A separate feature from CS-034's
  diacritic folding, and it should not be smuggled in with it.
- **SCIM `/Groups`.** `/Users` delivers the whole offboarding value; groups are where SCIM
  implementations go to die. Add only on request.
