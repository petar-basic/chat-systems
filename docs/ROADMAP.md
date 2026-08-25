# Roadmap & known limitations

What isn't done yet, why, and in what order it should be built. This is a living
document — it's here so the sharp questions have honest, specific answers instead of
hand-waving.

Every open item below is a ticket in [`docs/tickets/`](./tickets/INDEX.md), numbered in
execution order — the number is the schedule. A ticket's file is deleted when it ships;
what it changed lives in the git history and in the docs it touched. Read
[the index](./tickets/INDEX.md) for the dependency map and the conflict table; this page
is the summary and the reasoning behind the sequence.

**Waves 0 through 7 are shipped.** Next ticket:
[CS-029](./tickets/CS-029-message-edit-history.md).

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

## Wave 4 — Governance ✅ shipped

### [CS-018] Audit log coverage
The trail was written from three ad-hoc call sites and could not be read at all. There is
now one writer — `audit::record` with a typed `AuditAction` — reached from every
destructive action: message and channel deletion, channel and workspace membership and
role changes, invites, integrations (create, delete, reveal, rotate), file deletion,
workspace create/delete/restore, and suspend/activate/instance-role. `ip_address` is
populated through the same trusted-proxy rules the rate limit uses, so a caller cannot
write a chosen address into the trail. A write that fails logs and returns; the action has
already happened, and a hiccup on the trail must not turn a successful deletion into a 500
the caller will retry.

Reading it: `GET /api/workspaces/:ws_id/audit-log` for workspace admins and
`GET /api/admin/audit-log` for instance admins, both keyset-paginated on `(created_at, id)`
and filterable by action, actor and time. The UI is a panel in the workspace menu and a tab
in the instance admin page.

Login events are deliberately absent — the decision was to record actions that destroy or
grant, not every successful sign-in, which would bury them.

The `audit_log` foreign keys to `workspaces` and `users` are dropped in migration `…18`:
the trail is append-only history, and with the reference in place a hard workspace delete
would either fail on it or take the record of the deletion down with it.

### [CS-019] Scope outgoing webhooks
An outgoing webhook fired on every `message.created` in the workspace, so one hook created
by any workspace admin streamed every private channel to an external URL, invisibly to the
people in them. `config.channel_ids` is now mandatory and validated against the workspace;
delivery matches on it (`config->'channel_ids' ? $2`). Attaching one to a private channel
requires membership *and* moderator rights there — otherwise an admin outside the channel
could read it through a webhook pointed at themselves. The delivered payload is enumerated
rather than filtered: `id`, `channel_id`, `workspace_id`, `user_id`, `content`,
`created_at`, and nothing the message model grows next.

Every channel with an attached webhook shows an "Integration" badge in its header, readable
by every member, not just admins — the point is that people can see it before they type.

**Breaking:** migration `…19` deactivates every existing outgoing webhook. They carry no
record of which channels they were meant to see, and a silent no-op would look like a
working integration that quietly stopped delivering. Re-create them with a scope.

### [CS-020] File moderation and lifecycle
The uploader was the only person who could delete a file, so an admin could not take down a
leaked credential. A workspace admin and the moderators of the channel a file was posted in
can now delete it, and a non-owner deletion is audited with the uploader's id.

Attachments now follow their message: deleting a channel or conversation message deletes
its attachments, and an edit that drops the link to an attachment deletes that attachment.
Both delete the stored object immediately rather than marking a row — an attachment whose
message is gone is unreachable through the UI but still downloadable by storage key, so
leaving it behind meant "deleted" only ever meant "hidden". The rows and keys come back
from a single `DELETE … RETURNING`, because a read followed by a delete can hand the same
key to two concurrent callers.

---

## Wave 5 — Correctness ✅ shipped

### [CS-021] Re-authorize scheduled messages at delivery
The dispatcher inserted the message with no check that the author could still post there.
Authorization had happened when the message was scheduled, possibly days earlier; after
CS-007 made removal actually cut access, this was the one remaining path that wrote on
behalf of a removed user.

`deliver` now runs the same predicates the interactive handlers run —
`authz::require_channel_access` and `authz::require_conversation_participant` — plus the
two states a permission check cannot see: an archived channel and a soft-deleted
workspace. Using the same helpers is the point: a future change to visibility rules
applies to scheduled delivery for free.

Failure is terminal by construction — the claim already marked the row sent, and retrying
a permission the author does not have never succeeds. The reason is a stable slug
(`not_authorized`, `channel_archived`, `workspace_unavailable`, `internal_error`) rather
than a formatted database string, because it reaches the author's client. The author gets
a notification, and the failed row stays in their scheduled list instead of vanishing —
a message that silently evaporates is worse than one that fails loudly.

Removal cancels proactively as well: leaving a channel cancels what was queued for that
channel, leaving a workspace cancels everything queued in it. Delivery-time checking stays
as the backstop; this is so the author is told now rather than at send time, when they may
have forgotten writing it.

Reminders are the same class and got the same treatment: a reminder whose target has lost
access to the channel it names is dropped, not delivered without the link. It would
otherwise leak the channel's existence and, with a message link, a route into a
conversation they can no longer read.

### [CS-022] Scope the client-supplied message id
Conversation `send_message` let the client choose the primary key and used a unique
violation as the idempotency signal — then looked the row up by primary key alone, with no
check that it belonged to this conversation or that the caller could see it. A caller who
supplied an id that already existed elsewhere got that message's full row back.

**Decision: `client_message_id`, not a scoped lookup.** The ticket allowed either. The
scoped lookup closes the hole but leaves the client owning a global primary key, which is
a collision waiting for the day ids become predictable or land in a URL. The server now
generates the id and the sender's key lives in `client_message_id`, unique per
`(conversation_id, client_message_id)` — scoped by construction, so the same client id in
two conversations is not a conflict at all rather than a conflict we have to answer
carefully. It also has to be a v4 UUID: nil and non-random values are refused.

The frontend keeps its optimistic id as the client key and swaps the row for the server's
on success, which is what stops the websocket echo arriving as a second copy.

**The ticket was wrong about the blast radius.** It stated the channel `send_message` path
does not accept a client id, so it was unaffected. It does, with the same
`find_by_id`-after-unique-violation branch — and worse consequences: the caller only has to
be a member of the channel they are posting to in order to be handed a message from a
private channel they cannot read. Both paths now carry `client_message_id`, unique per
`(channel_id, client_message_id)` and `(conversation_id, client_message_id)`.

Reactions were checked in the same pass and needed no change — `UNIQUE (message_id,
user_id, emoji)` was already correctly scoped.

### [CS-023] Close remaining input validation gaps
Reaction emoji, reminder content and channel topic/description reached the database with no
length check, so an over-long value produced a 500 where the answer is 400 — and a 500 on
user-supplied input is a monitoring false positive that trains people to ignore alerts.

The survey covered every `String` field of every request DTO, and **every one of them now
has an explicit validator** rather than relying on the column to complain:

| Field | Rule |
|---|---|
| reaction emoji (HTTP, both paths) | 1–8 chars, no control characters — the rule the WebSocket path already enforced |
| reminder content | 4000, matching messages |
| channel topic | 500 · channel/workspace description | 4000 |
| workspace `icon_url` | same rule as avatars (http(s) or site-relative) |
| user `bio` | 500 · user `timezone` | IANA-shaped, 1–50 |
| hook `name` | 1–100 · hook `description` | 4000 |
| invite `email` on `create_invite` | validated, as it already was on the provisioning path |

Deliberately unvalidated: `LoginRequest.email`/`password` and `ForgotPasswordRequest.email`
are lookups whose whole design is one indistinguishable answer, so an early 400 would be
an oracle. Tokens are verified rather than validated.

`is_unique_violation` moved from a private helper in `conversations/routes.rs` to
`shared_common::errors`, and duplicate reactions now return 409 instead of a raw database
error — the new test found that one: it was a live 500.

---

## Wave 6 — Performance ✅ shipped

### [CS-024] Static message renderer
`RichTextDisplay` called `useEditor` **per message**, so every rendered message mounted a
full TipTap instance — a ProseMirror state, view, plugin stack, schema and contenteditable
subtree — to display text nobody edits. Messages now render from a parsed tree instead;
TipTap stays where it belongs, in the composer and the inline edit form.

**Measured, not asserted** (`MessageContent.bench.tsx`, `npx vitest bench`):

| Loaded messages | Editor per message | Static tree | |
|---|---|---|---|
| 100 | 512 ms | 9.5 ms | **54× faster** |
| 500 | 27.4 s | 45.5 ms | **601× faster** |

The old path degraded superlinearly — five times the messages cost fifty times the mount —
which is why the list fell over rather than merely slowing down.

The parser is `markdown-it` built from the **`zero` preset** and opened up to exactly the
constructs the composer can serialise, rather than the default preset with the rest turned
off: a parser that cannot represent headings, tables, images, raw HTML or reference links
has no surface there to get wrong. The token stream maps to React elements directly — no
`dangerouslySetInnerHTML` anywhere on this path — so the XSS posture holds by construction
instead of by getting a sanitiser's configuration right. The link allowlist (`http`,
`https`, `mailto`, `rel="noopener noreferrer nofollow"`) is ported deliberately, and a
rejected protocol renders as plain text with no anchor at all rather than an anchor with a
defused href.

Two things the ticket got wrong, found by testing against the real serialiser rather than
against the ticket's list:

- **Underline never reaches storage.** tiptap-markdown drops the mark (`"underline" mark is
  only available in html mode`), so there was nothing to port. The stylesheet rule for `u`
  is now dead and left in place only because the composer still offers the button.
- `![alt](url)` degrades to a link, not an image — there is no image node in either the old
  or the new path, so the behaviour is unchanged.

`createDisplayExtensions` and the ProseMirror mention-decoration plugin are deleted. Mention
highlighting is a pure function over text now (`lib/mentionHighlight.ts`), with the same
rules: longest label first, a boundary check on the left so an email address is not a
mention, no overlapping matches, broadcast words styled as self-mentions.

### [CS-025] Virtualize the message list
Channels and conversations now share one windowed list. Both had the same structure and the
same scroll behaviours to get right, and the conversation view had grown its own fourth copy
of the grouping rule — that is now one structural helper both use.

The old list leaned on `flex-col-reverse`, which gives bottom-anchoring for free but cannot
be windowed. Anchoring is explicit instead, and the three behaviours that break in naive
implementations are tests rather than hopes: an older page must not move the viewport, a new
message must stick to the bottom only when the reader is already there, and a new message
while scrolled up must offer a jump instead of yanking the view. The "jump to latest"
affordance the ticket assumed already existed did not, so it is new.

Grouping is pre-computed into a flat row list before the virtualizer sees it: grouping
depends on the *previous* message, which is usually not mounted once the list is windowed.

**Accepted trade-off:** find-in-page and text selection no longer reach messages outside the
window. The decision was to virtualize unconditionally rather than above a threshold, so
there is one code path and one behaviour; "copy link to message" is the practical substitute
and still works.

A message after a deleted one now shows its header again in channels, matching what
conversations already did — a tombstone breaks the visual run either way.

### [CS-026] Unread counts without subqueries
The channel list ran an `EXISTS` subquery per channel, so the cost grew with message volume
rather than channel count, and it could only answer "any", never "how many".
`channel_members` now carries `unread_count`, `mention_count` and `last_read_message_id`,
maintained in the same transaction as the insert — one statement for the whole channel, not
an N+1 on the hottest path in the product.

Mentions are counted separately because a mention badge and an unread badge are different
things in the UI, and deriving one from the other needs exactly the subquery this removes.
Muting deliberately does not touch the counters: it decides whether you are notified, not
whether you have read something, and letting it change the count would make the badge
disagree with the message list. The sidebar shows a number now, capped at `99+`.

The socket carries the delta — `message.new` gained `mentioned_user_ids` as a top-level
field — so the badge moves without a round trip back to the channel list.

A denormalised counter drifts: a half-committed transaction, a manual fix, a restore. A
reconciler in `chat-worker` recomputes recently-active channels every six hours and **logs
the number it corrected**, which turns "my badge is wrong and nobody knows why" from a bug
report into a metric. One test induces drift and asserts it is corrected; another proves the
new counter equals the old subquery over the same data, so the two definitions were shown
equivalent before the subquery was deleted.

### [CS-027] Presence without a keyspace scan
`get_online_users` walked the **entire Redis keyspace** in `COUNT 100` batches, and ran on
every `subscribe` frame — every WebSocket connection. The same Redis holds rate-limit keys,
revocation flags and huddle membership, so the cost grew with traffic that has nothing to do
with presence. After a deploy, 70 users reconnecting meant hundreds of full keyspace scans
in a few seconds.

Presence is now a sorted set per workspace scored by expiry: `ZADD presence:ws:{id}`,
read with one `ZRANGEBYSCORE` bounded by the workspace's member count. Stale entries are
trimmed with `ZREMRANGEBYSCORE` on read, so a node that dies self-heals without a sweeper.
`SCAN` is gone from the realtime crate entirely, along with `scan_keys` and
`get_online_users`, so it cannot come back by accident.

`node_id` is deleted with it. It existed to disambiguate two nodes holding the same user;
with a sorted set the score is the latest heartbeat from any node, which is the semantics
the per-node keys were approximating. The workspace list is resolved once per connection
instead of on every heartbeat, and `workspace.member_removed` drops the user from that
roster immediately rather than leaving a ghost until the TTL expires.
`realtime_presence_query_duration_seconds` is exported so the improvement is visible and a
regression is not silent.

---

## Wave 7 — Reliability ✅ shipped

### [CS-028] Durable realtime delivery
Events fanned out over pub/sub, which has no backlog: a client that was disconnected when
an event was published never received it. Nothing was permanently lost — the database is
the source of truth and reconnect refetched open views — but delivery over the socket was
at-most-once, and a view that was not open was not refetched, so unread state could sit
stale until something else triggered a fetch.

Durable events now go to a Redis Stream per workspace, `stream:ws:{id}`, capped at 10k
entries and trimmed to an hour by a worker task. Every frame carries the position it
occupies; the client remembers the newest position it has processed and sends it back on
`subscribe`, and the gateway replays the gap with `XRANGE`. Past the window — a position
older than the log, or more than 1000 events behind — it answers `sync.refetch_required`
and the client falls back to the refetch it already had.

**Which events are durable, and which deliberately are not.** Messages, reactions,
conversations and workspace membership go to the log. Typing, presence and WebRTC
signalling stay on pub/sub: replaying a typing indicator from five minutes ago is not
recovery, and a late ICE candidate is worse than none. Keeping two transports is the point
rather than an omission — each carries the traffic it suits.

**A deviation from the ticket, stated rather than buried.** It called for the gateway to
read the live tail from the stream with `XREAD BLOCK` and join it to the replay. The stream
is the log here and pub/sub remains the live tail. The seam between "replay the gap" and
"join the tail" is the part of that design most likely to drop or duplicate an event, and
it has to be right per connection; with the log written alongside the live path, the replay
simply overlaps the tail and the client discards what it has already applied — which it has
to do anyway, because delivery is at-least-once by design. The cost is one extra Redis
write per durable event.

**The client discards what it has already applied.** At-least-once delivery is only safe
if duplicates are actually dropped, so `wsQuerySync` was audited handler by handler. Most
reconcile by id and would not have noticed a repeat — but two **increment**: a thread's
`reply_count` and the unread badge. Both would have double-counted at the seam where the
replay overlaps the live tail. The suppression sits in `dispatch`, where an event at or
behind the position already processed is dropped once, rather than in every handler that
would otherwise have to defend itself.

**Replay uses the live path's visibility rules, not its own.** `handle_event_for` takes an
`Audience` — everyone, or one connection — and every predicate below it is unchanged. This
is the part where a naive implementation leaks a private channel into somebody's backlog,
so there is a test that puts a message the client cannot see in the middle of the gap and
asserts it does not come out.

**Two ordering bugs found by the end-to-end test**, both worth recording because neither
shows up in a unit test:

- A client that had never received an event had no position to resume from, so it silently
  fell back to at-most-once. The gateway now hands out the current tail at subscribe time.
- The client re-subscribed *before* re-declaring its channels on reconnect, so the replay
  ran against an empty subscription set and correctly delivered nothing. Channel joins now
  go first.

**Backpressure now says why.** A slow client is still dropped, but with a close frame
(`4003`) instead of in silence, so it reconnects immediately and replays from its position
rather than waiting up to a heartbeat and then refetching everything.

**Consumer groups for the worker.** The notification and hook consumers read the same
streams through `XREADGROUP` with acknowledgement, which removes the single-replica
constraint CS-004 accepted as a temporary answer — that is the payoff that makes the
transport change worth doing. Delivery becomes at-least-once, so a redelivery after a
worker dies mid-dispatch could call somebody's webhook twice; the (hook, event) pair is
claimed before the request goes out and migration `…22` is the unique index that makes the
claim mean something. Acknowledgement is structured so it cannot be skipped: the per-event
work sits in an async block, where `continue` does not compile.

Groups are created from the start of a stream rather than its tail. Creating at the tail
looks tidier but silently skips everything published between a stream appearing and the
worker noticing it.

---

## Wave 8 — Compliance ✅ shipped

### [CS-029] Message edit history
`update_message` mutated in place, so an edited message could only say *that* it changed.
Editing now writes the pre-image to `message_edits` (channels) or
`conversation_message_edits` (DMs) inside the same transaction as the update, capped at the
50 most recent versions per message.

Two tables of the same shape rather than one with two nullable foreign keys: a nullable
pair is a constraint you have to remember to write in every query, and the DM side has
different visibility rules anyway. The author sees their own history; a workspace admin sees
anyone's and **that read is itself audited** — history is the record of what somebody tried
to take back, and reading it silently is the thing the audit trail exists to prevent.

### [CS-030] Retention and cleanup
`retention_policies` is per workspace, and `NULL` means keep forever for messages and files
— an empty policy row is not a deletion order. Only an owner may change one, and the change
is audited with before and after.

**What runs unconditionally**, because it is cleanup rather than policy: expired refresh
tokens, consumed reset tokens, expired invites and `hook_executions` older than 30 days.
Nothing there is anybody's data. `audit_days` defaults to 730 against `notification_days`'s
90 — the trail has to outlive the thing it describes.

Purging is batched at 500 rows so a first run on a large instance does not hold a
transaction open for minutes, and files delete the object before the row: the reverse
orphans bytes in the bucket with nothing left pointing at them. `RETENTION_DRY_RUN` reports
what would go without deleting it — deletion is irreversible and the first run is where a
misread policy shows up.

### [CS-031] Export and erasure
Workspace export is owner-only, user export is self-or-instance-admin, and both produce a
tar of JSONL plus a manifest. Written by hand rather than by pulling in a tar crate: the
format is a header and 512-byte blocks, and the dependency would have been more surface than
code.

A workspace export **declares every file it references** even when the download is not
included, so "0 conversations" is a statement about the data rather than an artifact of the
exporter. The download link is a single-use token with an expiry, not a path — an export is
the entire workspace in one file, and a URL that keeps working is a permanent leak.

Erasure anonymizes by default and hard-deletes only when asked. Deleting a user's rows
outright takes their messages out of everybody else's threads, which is a different decision
from "this person is gone" and not one an API should make silently.

### [CS-032] SSO (OIDC) and TOTP
**The ticket named a `user_identities` table that does not exist** — the schema has
`oauth_accounts`, which is what the linkage uses; it gained an `email` column. Endpoints come
from the provider's discovery document, so Google, Entra and Okta work from the same three
settings rather than a per-provider branch. PKCE throughout; the verifier lives in Redis
under an opaque handle and the cookie only names it.

Three refusals worth stating: an **unverified** email is never linked (anyone who can make
their provider assert somebody else's address would otherwise inherit that account);
provisioning defaults to **invite-only**, because SSO changes how you prove who you are, not
who is allowed in; and an SSO account's password is nulled, with the instance admin as the
deliberate exception — a break-glass local admin has to survive a provider outage.

**TOTP replay was a real gap in the ticket.** Checking the six digits alone accepts the same
code for its full validity, which is exactly long enough for somebody reading over a
shoulder. `user_totp.last_used_step` is claimed atomically, so a code works once — and the
claimed step is the one the *code* belongs to rather than the one the request arrived in,
because a step either side is allowed for clock drift and claiming the request's step would
hand the same digits back to a replay as soon as the clock ticked over. Secrets are
encrypted with a key derived from the instance's JWT secret — a database dump full of TOTP
secrets is the same failure as one full of passwords. Recovery codes are hashed like
passwords and shown once. `REQUIRE_ADMIN_TOTP` makes the factor mandatory for instance
admins and defaults to **off**: turning it on locks out any admin who has not enrolled.

The flow is tested end to end against a real provider — `mock-oauth2-server` in
docker-compose. The redirect, the cookie and the code exchange are exactly where SSO
integrations break, and none of that is exercised by stubbing the token endpoint in-process.

### [CS-033] SCIM deprovisioning
`/api/scim/v2/Users` behind its own bearer token, outside `auth_middleware` entirely: the
caller is a machine, not a session. Tokens are revealed once, rotatable, and stored as a
digest.

Deactivation is the composed operation the ticket asked for and nothing new: suspend,
`sessions::revoke(All)`, then membership removal through the CS-007 path so `channel_members`
cascades and live subscriptions drop. Doing only the first leaves somebody connected; doing
only the first two leaves their private-channel access waiting for the next reactivation.

`DELETE` is deactivation. An identity provider retrying a delete must not be able to take a
workspace's history with it — erasure is CS-031's job and stays an administrator's choice.
**Reactivation restores the account and no memberships**, which is not an omission: CS-007
made removal destructive precisely so that coming back needs a fresh invite.

Both PATCH shapes are read — `{path: "active", value: false}` and Entra's pathless
`{value: {"active": false}}` — because supporting one of them means deprovisioning silently
does nothing for half the market. Audit entries for machine callers carry no `user_id`;
borrowing one would name somebody who did not do it.

---

## Wave 9 — Product parity (partly shipped)

Three of six shipped. `CS-036`, `CS-037` and `CS-038` are still ahead and keep their
tickets.

### [CS-034] Search language and DM search ✅ shipped
`content_search` was `GENERATED ALWAYS AS (to_tsvector('english', content)) STORED`, so the
language was a schema decision. For a team writing Serbian that is wrong in both directions:
English stemming mangles the words, and `četvrtak` and `cetvrtak` were different tokens.

The stored vector is now `simple` over `unaccent`, which does no stemming and no stop-word
removal — never actively wrong for a mixed-language instance, unlike a language guess. A
trigram index over the same normalized expression answers the half `to_tsvector` cannot:
substrings and near misses, which is what people actually type into a chat search. Both
signals rank in one expression, so a fuzzy hit always sits below every exact one.

**`SEARCH_TEXT_CONFIG` from the ticket is deliberately absent.** A setting that has to agree
with the contents of a stored column is a setting that will eventually disagree with it.
Instead there is a SQL function, `search_text_config()`, called both by the trigger that
writes the vector and by the query that reads it — they cannot drift. Changing the language
is a migration that replaces the function and rebuilds, which is exactly what the ticket
asked the behaviour to be.

**The migration never rewrites the table.** Replacing a generated column takes ACCESS
EXCLUSIVE for the whole rewrite, which on a large instance is minutes of downtime. Instead:
a nullable column (a catalog change), a trigger, and `CREATE INDEX CONCURRENTLY`. Each
concurrent build is alone in its own migration file because Postgres wraps a multi-statement
string in an implicit transaction and refuses to build concurrently inside one — `--
no-transaction` alone is not enough.

The backfill is not in a migration either: a migration runs as one transaction, so a loop
inside it cannot commit between batches, and a single `UPDATE` over every row would hold its
locks and its snapshot for the whole run. The worker does it in committed batches. While it
runs, old messages are still found through the trigram index, so search degrades to
substring matching rather than going blank.

DMs are searchable for the first time, scoped by participation. Channels and conversations
come back as two lists rather than one merged list — different resources, different
visibility rules, and the guest rule from CS-010 applies to only one of them.

### [CS-035] Web Push ✅ shipped
**The ticket said to add handlers to the existing service worker. There was no service
worker** — the manifest and icons were there, nothing else. `public/sw.js` is new, and it
does push and `notificationclick` and nothing else: no fetch handler, no caching, because an
app shell served from a cache is an app running a version the server has already replaced.

nginx was serving every `.js` with `Cache-Control: immutable` for a year, which would have
made a fix to the worker something nobody receives. It now has its own `no-cache` location.

Sending happens in the worker, on the same path that writes the notification row, and is
suppressed three ways: a muted channel, do-not-disturb, and — the one that matters for
whether people keep notifications on — a live socket for that workspace. Somebody looking at
the message does not need it on their phone as well. Payloads carry a truncated preview
rather than the message: encrypted or not, it passes through a third-party push service.
`410 Gone` is the only reliable signal a subscription is dead, so that is what prunes it.

Delivery is tested against a real HTTP server that records what reached it, rather than
against a mock of our own sender.

### [CS-039] Custom emoji, user groups, bots and slash commands ✅ shipped
Four independent gaps, shipped in the order the ticket set out.

**Bot identity.** Incoming webhook messages appeared as sent by the admin who created the
hook. They now carry the hook's own name and icon. **The ticket implies a bot `users` row;
this uses `messages.metadata`** — a bot account would either have to join every workspace
member list to render, or fail to resolve and show as nobody, and the column exists for
exactly this. `user_id` still points at the creator, which keeps the foreign key and the
audit trail honest. Slack-compatible `username` and `icon_url` overrides come along free.

**Custom emoji.** Per workspace, because a shortcode is shared vocabulary: two workspaces on
one instance can each have their own `:shipit:`. Names that would shadow a standard shortcode
are refused at upload — the renderer resolves the standard set first, so such an emoji would
upload cleanly and then never appear. Size and type limits are enforced because an emoji is
not a file share.

**User groups.** `@backend` resolves to its members, and the fan-out is the intersection of
the group and the channel. Notifying a member who is not in the channel would tell them a
private channel exists and hand them a preview of a message they cannot open. A group mention
carries `group:<uuid>` inside the existing `@[label](target)` form, so the user-mention parser
needed no changes at all. The handle is not editable after creation: messages carry the id
but people read the handle, and renaming would silently rewrite the record of who was asked.

**Slash commands.** Synchronous, through the same SSRF-validated, HMAC-signed transport the
outgoing hooks use, with a three-second timeout and **no retries** — unlike an event, a
command has somebody waiting for it, and a second attempt would do whatever it does twice.
Registered commands are scoped to channels exactly as outgoing hooks are, for the same
reason: invoking one sends what somebody typed to a third-party URL.

**The new realtime delivery mode the ticket called for was not needed.** Synchronous dispatch
means the ephemeral answer comes back in the same HTTP response; nothing is persisted and
nothing new had to be added to the gateway. An unknown command is a 404 and the client sends
what was typed as an ordinary message, so a typo is visible instead of disappearing into an
error.

Built-ins are `/dnd`, `/topic`, `/invite`, `/shrug` and `/remind`, through the same registry.
**`/away` is not shipped**: presence here is derived from whether the gateway holds a socket
(CS-027), so there is no flag for a command to set. What people want from `/away` — saying
what they are doing — is a custom status, which is set on the profile and is not presence.

**A new operator setting, `WEBHOOK_ALLOW_PRIVATE_TARGETS`, default off.** Outbound calls
refuse private, loopback and link-local addresses so that a workspace admin cannot point one
at a cloud metadata endpoint. A self-hosted instance whose CI is on the same private network
has nowhere else to point it, so the operator — not the admin — can allow it, with the cost
stated where it is documented.

### Still ahead in this wave

### [CS-036](./tickets/CS-036-slack-import-export.md) — Slack import
**Today:** no import. A migrating company either abandons its history or keeps paying Slack
as an archive — and in practice that means the migration fails.

### [CS-037](./tickets/CS-037-huddle-sfu.md) — SFU for large huddles
**Today:** huddles use a WebRTC mesh, fine to six or eight participants. The `livekit_room`
column exists and is unused. Keep the mesh for small calls; switch above a threshold.

### [CS-038](./tickets/CS-038-mobile-client.md) — Mobile client
**Today:** desktop-first, mobile layout explicitly not a goal. The largest adoption risk on
this page: a chat tool that cannot reach people on a phone gets replaced in practice by
whatever can. **Decided 2026-08-13: Option A, the responsive PWA.** Web Push shipped with
CS-035, which was the half that made a PWA worth installing; React Native stays a follow-up
for if push reliability or call ringing turns out to be the actual blocker.

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
