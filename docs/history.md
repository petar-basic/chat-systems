# Engineering history

What each shipped ticket changed and why, in the state it shipped. This is the record the
roadmap points at: [ROADMAP.md](./ROADMAP.md) keeps only what is open, the order, and the
decisions not to build something. [CHANGELOG.md](../CHANGELOG.md) is written from this
material, one release at a time.

A shipped section describes the code as it was on the day; where the architecture has moved
since (the conversation tables of Wave 1 and 5 no longer exist, for example), the current
shape is in [backend.md](./backend.md) and [frontend.md](./frontend.md).

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

### [CS-005] Compile-time-checked queries — done
Decided 2026-08-07 as "new code uses `sqlx::query_as!`, convert the rest opportunistically";
completed as a single pass on 2026-09-02 instead, once the architecture review made the
runtime queries the last place a schema change could hide. Every query is a macro now,
`.sqlx/` is committed, `SQLX_OFFLINE=true` in the image build and
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

Six of seven shipped. `CS-037` is still ahead and keeps its ticket; `CS-038` moved to Wave
11 and shipped there, with Web Push behind it.

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

### [CS-040] The small Slack gaps ✅ shipped
Six things a team notices in the first week, closed together because each is small and they
share the same surfaces: **threads inside DMs**, **saved items**, **channel bookmarks**,
**message forwarding**, **a status you set yourself**, and **reminders with a panel and
`/remind`**. They came out of [#33](https://github.com/petar-basic/chat-systems/issues/33),
where answering honestly meant writing down what was missing — and the list was short enough
to build instead of document.

**Threads in DMs reuse the channel rule, not a new one.** `thread_parent_id` on
`conversation_messages`, one level deep, and the feed index became partial on
`thread_parent_id IS NULL` so a reply cannot be paged into the conversation. The realtime
handler had to learn the same distinction: without it a reply arrived in the feed *and* the
thread, which is the one place it would have been visibly wrong.

**Forwarding needed no server at all.** A forward is an ordinary message quoting another one.
Adding a table and an id would have bought a "forwarded from" chip and a second delete path
to get wrong.

**`/remind` finally uses the `timezone` column**, which had been stored since the first
migration and read by nothing. `at 15:00` means 15:00 where the person is; Postgres already
ships the IANA database, so no new dependency. A time already past means tomorrow.

**A custom status is not presence.** Presence stays derived from whether the gateway holds a
socket, so `/away` is still not shipped; the status is the manual thing, and an expired one
stops being returned by every read path rather than waiting for a cleanup job.

### [CS-036] Slack import ✅ shipped
A `chat-import` binary rather than an endpoint: an import is long, restartable and
supervised, and nobody is waiting on the response.

**Two passes, because threads are what break single-pass importers.** Users, channels and
memberships first; messages second, so `thread_ts` resolves against a row that exists. A
reply whose parent is missing from the export stays in the channel and says so in the report,
rather than disappearing along with the deleted parent.

**Matching is by email and nothing else.** A Slack handle is not an identity — the same one
belongs to different people in different workspaces, and matching on it would attribute
somebody's history to a stranger. An unmatched address becomes an account with no password,
so the history has an owner who can claim it through the invite flow and nobody else can.

**Idempotency is written down, not remembered.** `messages.slack_ts` and the two mapping
tables are what make a re-run a no-op and an interrupted run resumable. A 200k-message import
*will* be interrupted; that has to be survivable rather than a reason to start over.

**`--dry-run` writes nothing at all**, including the run record, and reports what a real run
would do — counts and the list of what will not convert cleanly.

---

## Wave 11 — Guest containment, operational readiness and mobile ✅ shipped

Written out of the adoption review of 2026-08-14 — could a thirty-five person company with
guest users, guest channels and locked channels move off Slack onto this? — and shipped in
the order that review put them. `CS-047` was deliberately left out: it measures what this
wave changes, so it runs after it.

### [CS-041] Scope the member directory
`list_members` required only workspace membership and returned every member's email, so a
guest invited into one channel received the company's staff directory with addresses. Every
other guest rule was already tight; this was the door still open.

A guest now sees the people they share a channel with, and no addresses at all — not even
for those people. A display name and an avatar are what the UI needs to render a message;
an address is what somebody outside the company needs to phish it. The redaction lives in
the repo query rather than in the handler, because a projection a caller has to remember to
apply is one the next caller forgets.

### [CS-042] Restrict who a guest may DM
`create_conversation` checked workspace membership and nothing else, so a guest who already
knew a user id could open a private channel to anyone. The same predicate CS-041
introduced — "do these two share a channel" — now guards creating one, asked once for
reading and once for writing.

An unknown id and an unreachable member return the same refusal, byte for byte. Three
different answers would have rebuilt the directory CS-041 had just closed. Conversations
that already exist keep working: the rule is on creating one.

### [CS-043] Announcement channels
There was no way to make a channel read-only. `channels.settings` had been carried by the
schema since the first migration and **no route had ever read or written it**.

`post_policy` is `everyone` or `moderators`, enforced by `require_channel_post` in `authz`
rather than in each handler — the writers are the part that gets missed, and there are four
of them in four different files: the composer, thread replies, scheduled sends and slash
commands that answer `in_channel`. A scheduled message written before the channel was locked
is refused at delivery, because CS-021 already re-authorizes at the moment of the effect.

Reading and reacting are untouched: a reaction is not a post. An incoming webhook scoped to
the channel still posts, because a release feed is exactly what such a channel is for, and
the hook's own scoping from CS-019 is what decides it. A command whose answer cannot be
posted degrades to an ephemeral answer instead of an error for work it already did.

### [CS-044] Revocation no longer fails open
A Redis error during the revocation lookup was treated as "not revoked". With the access
token defaulting to an hour, that meant suspending somebody or deprovisioning them through
SCIM quietly did nothing for up to an hour whenever Redis was unavailable — and nothing
alerted on it, which was the worse half. CS-033's whole promise runs through this call.

The lookup now has three answers rather than two. A store that will not respond within
250ms gets one retry; after that the request is refused with a `503` rather than a `401`,
because a client should retry in a second, not throw the session away and send somebody to a
login form over a cache blink. The same fix landed in the realtime gateway, where the thing
being granted is a socket that stays open rather than one request.

The default access token is now 15 minutes, and `auth_revocation_lookup_failures_total`
is exposed so an operator can see revocation failing instead of finding out later.

### [CS-045] Email a mention that reached nobody
Email had two uses in this product — invitations and password resets — and no third.
Somebody mentioned while offline, who had never granted push permission, found out when they
next opened the app.

The worker now queues a digest when nothing else could have reached them: no live socket,
no push subscription, not muted, not in do-not-disturb. Five minutes later it sends one
email per person per workspace, and coming online inside that window cancels it — the badge
is already waiting for them, so the email would be describing what they can see. The queue
is a table rather than Redis, because a worker restart inside the window would otherwise
drop the only notification they were going to get.

The email carries who, where and a link, and no message text: mail sits in an inbox on
somebody else's server indefinitely, and is the least private transport this product
touches. Off entirely when SMTP is unconfigured, exactly as Web Push is off without VAPID
keys.

### [CS-046] Huddle and call consumers onto consumer groups
CS-028 moved the notification and hook consumers onto `XREADGROUP`; the huddle and call
consumers stayed on plain pub/sub, which delivers to every subscriber. That kept
`chat-worker` at one replica — a single point of failure for call ringing, which fails in
the quietest possible way: everything else keeps working.

Huddle membership is history — it is written to a table and read back as a call log — so it
is now durable and read through a consumer group with acknowledgement. The ring stays
ephemeral, because a call announced two minutes late is worse than one not announced, and
instead the notification is claimed by `(huddle_id, to_user_id)` before it is written, the
way CS-028 claims `(hook_id, event_id)`.

Worth stating plainly: `record_join` was already an upsert and `end_session` already only
succeeded once, so those two were no-ops on a redelivery rather than bugs. The ring was the
one place a second replica produced a second effect.

**Revisited 2026-09-02: the ring goes on the stream after all.** Keeping it on pub/sub
protected against a late ring but left the more common failure open — a worker restarting
at the moment of the call rang nobody, ever. The ring is now a durable event read through
the notification consumer's group; the "late ring" concern is handled where it belongs, by
dropping any ring older than sixty seconds at delivery, and the replay path skips rings so
a reconnecting client is never rung for a call that ended. `huddle_ring_claims` is dropped;
a unique index on the notification row absorbs a redelivery. `chat-worker` runs two
replicas in production.

### [CS-038] Mobile client — the responsive PWA
**Decided 2026-08-13: Option A.** Web Push shipped in Wave 9, which was the half that made
an installed PWA worth having on a phone.

The inventory came first, as the ticket demanded, and it showed the drawer and the
right-hand panels were already responsive while the modals, the composer and the shell were
not. What that inventory turned into:

- **The viewport tag was doing half its job.** `viewport-fit=cover` is what makes
  `env(safe-area-inset-*)` non-zero, and `interactive-widget=resizes-content` is what makes
  the virtual keyboard shrink the viewport instead of scrolling the composer out from under
  the person typing.
- **The editor was 14px.** iOS Safari zooms the page whenever a focused editable is under
  16px, so tapping the composer scaled the whole app up and left people pinching back out.
- **Modals were trapped inside the navigation drawer.** The drawer animates with
  `translate-x`, and a transformed ancestor becomes the containing block for
  `position: fixed` — so a dialog opened from the sidebar was laid out inside its 304px
  instead of over the screen. Found by the new mobile spec, fixed by rendering the shared
  `Modal` through a portal. One change, eight callers.
- Modals are now sheets that reach the bottom edge below `sm`, and centred dialogs above it.

A `mobile` Playwright project runs the core journey at a phone width in CI, and
`docs/manual-qa.md` gained the list of things emulation cannot check — the keyboard, safe
areas on a real notch, the back gesture, background/foreground, and push to a closed app.

---

## Wave 12 — Architecture follow-ups ✅ shipped

Not a ticketed wave: a review of the whole codebase on 2026-09-02, once every feature was in,
asking where the code had grown a hand-rolled solution that a library or a different shape
would do better. Seven items, done as one branch, merged 2026-09-03.

### Durable events end to end
`XAUTOCLAIM` recovers a stream entry a crashed worker was holding; the huddle ring moved from
pub/sub onto the workspace stream (migration 42, `huddle_ring_claims` dropped); and every
durable event is now staged in `event_outbox` inside the transaction that writes its row,
appended to the stream after commit, and relayed by the worker if the fast path died in
between (migration 44). The gateway's typing and conversation fan-out lost their per-socket
database checks in favour of the subscriptions it already holds.

### The contract is generated
`utoipa` on every handler, `/api/openapi.json`, `openapi-typescript` for the client types and
`openapi-fetch` behind `ApiClient.typed(...)` — the untyped `get/post` methods are gone. The
WebSocket frames are one `ServerFrame` enum with `ts-rs` generating the frontend union, and
eight frame handlers that nothing ever emitted went with it. Two real bugs surfaced: `@here`
reached nobody (presence was scanned under a key layout the gateway no longer wrote), and
fourteen handler names collided across features so the generated TypeScript mapped paths to
the wrong operation.

### Libraries for the boring parts
`object_store` replaced the file storage abstraction and the AWS SDK; `garde` validates
request DTOs on the field; `askama` renders the invite email; `outbound_emails` (migration 43)
queues every email for the worker to deliver with retries; `ipnet` parses trusted proxies
once at startup; `backon` supervises restarts. Every query became a compile-time-checked
`sqlx` macro with the cache committed, closing CS-005 as a single pass.

### One message model
Migration 45 folded conversations into channels of type `dm`/`group_dm`, ids preserved.
Conversation messages, reactions, edits, attachments, saved and scheduled rows moved onto the
channel tables; the parallel routes, frames, repo and frontend view were deleted and the two
thread rules the DM path had (same channel, one level deep) moved onto the channel path.
Retention policies now reach direct messages.

### Files that had become too large to read
`auth/service.rs` split by concern, `ChannelSidebar.tsx` into eight components and
`useWorkspaceController` into four hooks; one shared test `AppConfig` instead of three copies.

### [CS-047] Prove it at import scale ✅ shipped
The largest dataset the system had run against was about 2,200 messages. A generator now
writes a synthetic Slack export of 500,000 messages (`generate-slack-export.py`, seeded, 22
seconds), the real importer loads it, and `measure-scale.py` times search, history and the
unread counters as a signed-in member. The numbers are in RUNBOOK; the ones that matter:

- **The importer is not the bottleneck.** 500,000 messages in 6 min 50 s, and a second import
  running alongside live searches costs them 15–25% and nothing else. No maintenance window.
- **History and unread counters do not notice the size.** 20 ms and 3 ms.
- **The backfill is fine** at 600,000 rows: 69 s, a third of it its own pauses.
- **Search is not.** A common word takes 1.6 s, and on a restored copy of the same data the
  planner chose a per-channel loop that took up to 13 s. That became CS-048 rather than a
  speculative rewrite here.

**The restore drill found that backups did not restore.** The search functions named
`unaccent` without a schema; `pg_dump` restores with an empty `search_path`, so recreating the
trigram index failed — and the documented command had no `ON_ERROR_STOP`, so it reported one
error, kept going, and left every substring search as a sequential scan on a database whose
row counts all matched. Migration 46 schema-qualifies the function bodies (not `SET
search_path`, which would stop them inlining), the runbook restores with `ON_ERROR_STOP` and
analyzes afterwards, and the upgrade note says what to do with a dump taken before the fix.
After it: 600,000 messages dumped in 15 s and restored in 18 s, counts and indexes identical.

**Docker's 64 MB `/dev/shm` was too small for Postgres.** A parallel `VACUUM` on the messages
table failed with `could not resize shared memory segment`; parallel search scans draw on the
same memory. Postgres now gets `shm_size: 256mb` in both compose files.

---

## Wave 13 — What the scale measurement found ✅ shipped

### [CS-048] Search that holds up at 500,000 messages
CS-047 measured search at 1.6 s for a common word and 2–13 s on a restored copy of the same
data, with three causes visible in `EXPLAIN ANALYZE`. Each got its own change in
`MessageRepo::search`:

- **Every search is planned for its own term.** sqlx keeps the statement prepared, and after
  five calls Postgres may switch to a generic plan that cannot see the term; on the restored
  database that plan looped over all 195 visible channels and scanned both indexes for each.
  The query now runs in a transaction that sets `plan_cache_mode = force_custom_plan` locally
  — `set_config(…, true)` in a checked macro rather than `.persistent(false)`, which the
  macros' return type does not offer.
- **Only the newest 1,000 exact matches are ranked.** A word in 334,000 messages no longer
  sorts all of them, spilling to disk, to return twenty. Chat search wants recent results,
  and page 51 of a common word is not a request anyone makes.
- **Fuzzy matching runs only when exact matching runs short.** The trigram branch was OR-ed
  into every search, so a rare word paid for 43,000 trigram candidates after its 370 exact
  hits had already filled the page. It now runs when the exact matches number fewer than
  the page reaches, takes the newest 1,000 candidates, and ranks them below every exact
  match, as before.

| Search, p95 | Before | After |
|---|---|---|
| a word in 21,000 messages | 190 ms | 24 ms |
| a word in 370 messages | 470 ms | 7 ms |
| a substring | 480 ms | 158 ms |
| a word in 334,000 messages | 1.7 s | 13 ms |
| the same, on a restored and analyzed database | 2.0–13.2 s | 6–147 ms |

What it does not fix: a database with **no statistics at all** still plans badly — up to 3 s
for a common word in the minute between a restore and autovacuum's first `ANALYZE`. The
restore procedure now ends with `vacuumdb --analyze-only`, which closes that window.

---

## Wave 14 — What a team from Slack misses on day one ✅ shipped

### [CS-049] Link previews ✅ shipped
A link now arrives with a card. The worker's `unfurl` consumer reads `message.created` and
`message.updated`, takes the first three links outside code, and fetches each one; the
result goes into `metadata.unfurls` and out as `message.updated`, so the client needed no
new event — only the cache patch for that event, which had been copying content and nothing
else.

- **The fetch connects to the address it checked.** `hooks::ssrf::resolve_outbound` returns
  the resolved addresses and reqwest is pinned to them, so a DNS answer cannot turn private
  between the check and the connection; redirects are followed by hand, each hop checked
  again. Strict mode always: an operator who allows webhooks onto their private network has
  not thereby let every member point the server at it.
- **Idempotent without a claims table.** `metadata.unfurls_source` hashes the link list, and
  the write only happens while the stored source differs and the content is still what was
  read. A redelivered event, the consumer's own `message.updated`, and an edit that keeps
  its links all do nothing; an edit racing a fetch loses the fetch, not the edit.
- **No HTML parser dependency.** A preview needs the `<meta>` tags and `<title>` of the
  head; a small tokenizer that understands quoting and entities does that without pulling
  html5ever into the build.
- **People can say no.** The author removes a message's previews (`DELETE
  /messages/:id/unfurls`, sticky across edits); a workspace admin turns them off.

Not done as the ticket proposed: the end-to-end test that unfurls a page on the stack
itself, because a preview never fetches a private address and CI has no public page to
point at. The fetcher is tested against a local server through its private-address switch
instead, and the refusal of that same server in strict mode is a test of its own.

---

### [CS-050] Mark as unread ✅ shipped
`POST /channels/:id/unread` moves the member's read position to just before the chosen
message and recomputes both counters from the same rule the unread reconciler uses —
undeleted messages by somebody else, at or after the boundary — so the badge it produces is
the one the reconciler would converge on anyway. Mentions are recomputed from the mention
notifications on messages past the boundary; a muted channel never stored those, so marking
one unread brings back its count but not a mention badge. On a 63,000-message channel the
call takes about 30 ms, and 1.7 s cold when the boundary is the very first message.

**The ticket assumed a frame the read path did not have.** Reading a channel never told the
reader's other tabs anything, so the "existing" read frame it meant to reuse did not exist.
`channel.read_state` is new: published by both `/read` and `/unread`, sent by the gateway
only to that user's connections, ephemeral like typing (a missed one is corrected by the
next unread-count fetch). Reading a channel on a phone now clears its badge on the laptop.

The open channel did not need the auto-read suppression the ticket planned: the client only
marks a channel read when you enter it, so a channel marked unread while open stays unread
until you leave and come back. *Mark unread* is in the message menu, the mobile action
sheet, and on Alt+click.

---

### [CS-051] All unreads ✅ shipped
Catching up no longer means clicking through every bold channel. `GET
/workspaces/:id/unreads` reads the CS-026 counters for the channels with anything unread —
muted last, then newest activity first — and fetches each one's newest 20 unread messages
in a single `LATERAL` query rather than one per channel. Guests need no rule of their own:
counters exist only for channel memberships. At 500,000 messages, a member with about 200
unread channels gets the first page in 8 ms at p95; `measure-scale.py` now carries the
scenario.

The view is a route (`/app/:workspace/unreads`), so it survives a reload and the back
button; the routing hook learned not to redirect it to #general. Opening the view marks
nothing read — the per-channel button and Esc on a focused group do, through the ordinary
read endpoint, so the badge clears on every device by way of CS-050's `channel.read_state`.
New messages and read changes refresh the view while it is open.

---

### [CS-052] Custom sidebar sections ✅ shipped
Forty channels can be grouped the way people think about them. A section is a row in
`sidebar_sections` (migration 47), owned by one person in one workspace; a channel's place is
a nullable `section_id` on that person's `channel_members` row, so the layout costs nothing
for anybody who never makes a section, leaving a channel forgets where it was, and deleting a
section sends its channels back to *Channels* by `ON DELETE SET NULL` rather than by code.
Names are unique per person ignoring case.

The collapsed state of the built-in sections moved to the server too
(`sidebar_collapsed_builtins`), so the layout follows the person to another device;
`localStorage` is only the first paint before the layout arrives.

Channels move by drag and drop between sections or by *Move to…* on the row, which is the
keyboard and touch path to the same thing; sections reorder with *Move up* / *Move down* from
their menu rather than by dragging, for the same reason. Every change is optimistic and rolls
back to the server's copy if it fails.

Wave 14 is done with this ticket.

---

## Wave 15 — Links people actually paste at work ✅ shipped

### [CS-053] Rich previews from connected apps (Jira, GitHub, Google Drive) ✅ shipped
The links a team pastes most sit behind a login, so CS-049's anonymous OpenGraph fetch can
say nothing about them. A Jira link now becomes a card built from Jira's API: issue type and
key, summary, status (coloured by Jira's status category), priority, assignee, *Open in Jira*
and the time it was fetched.

**Whose permission is the design.** The card is fetched with the connected account of the
person who posted the link, never with a workspace-wide token; everyone in the channel sees
what the poster could already see and chose to share. A poster who has not connected gets a
private prompt under their message instead of a card, and connecting refreshes that message.

**Setup is the admin's two minutes, not a deployment.** A self-hosted instance cannot ship
OAuth credentials, because Atlassian binds them to the redirect URL, so *Apps* shows exactly
what to paste into Atlassian's console — callback URL, scopes, the steps — and takes the
client ID and secret back. Connecting your own account is the test.

The framework is provider-shaped so GitHub and Google Drive add a matcher, a fetch and a
mapping to the same card: `workspace_apps` and `app_connections` (migration 48) with the
client secret and tokens sealed like TOTP secrets (`sealed.rs`, its own key purpose), OAuth
2.0 with PKCE whose state lives in Redis for ten minutes and is readable once, tokens
refreshed when they are about to expire, connections deleted when a member leaves the
workspace (a foreign key on the membership), is suspended or is erased, and every setup and
connection in the audit log. The client renders every provider's card with one component.

**GitHub** came second, on the same framework with no frontend card code. It is a GitHub
App, not an OAuth App: an OAuth App's only scope for private code is `repo`, which can also
write, while a GitHub App is read-only and limited to the repositories it is installed on.
Public repositories need no setup at all — the card comes from the anonymous API, cached for
ten minutes per URL because that limit is 60 requests an hour per server and the answer is the
same for everyone. A private link falls through to the poster's connection, then to a
*Connect* prompt. Checked against the real API: a `/pull/N` that GitHub serves for an issue
gets the issue card, and an `/issues/N` that is a pull request gets the pull request card.

**Google Drive** came last. It asks for `drive.metadata.readonly` and nothing else — the
card shows a file's type, owner and last change, and the instance can never read what is in
a document. Even the account name comes from Drive's `about` endpoint rather than adding
`openid` and `email` scopes for it. Google has no anonymous API, so every Drive card is the
poster's; Google's token endpoint returns no new refresh token on refresh, which the
token store already allowed for by keeping the old one. RUNBOOK says to make the consent
screen *Internal* where a Workspace organization can: no Google review, and none of the
seven-day expiry an unverified external app's connections get.

---

## Wave 16 — The rest of what a team pastes ✅ shipped

### [CS-054] Slack-grade app previews, Figma and Confluence ✅ shipped
Put next to Slack, CS-053's cards were missing the small things a member notices: the
*Connect* prompt had no way to say *never*, cards had no picture, *Apps* was a list of forms,
a video link was a still image — and a design-and-docs team's two most pasted services were
not covered. What stays different is the admin's one-time setup per service: Slack's apps
are hosted by their vendors, and a self-hosted instance cannot ship OAuth credentials.
Removing that step would mean a broker that every customer's tokens pass through.

**The prompt** offers *Connect*, *Not now* and *Never ask me again*, as Slack's does. The
last is a row per member and app (`app_prompt_optouts`, keyed on the membership); the
unfurl service turns that poster's prompt into an ordinary OpenGraph preview, and
connecting clears it. **Cards** show `image_url` and drop it if it fails to load — provider
thumbnails are signed and expire, and a snapshot does not justify copying them into our
storage. **Apps** shows each app's icon (copied from simple-icons rather than adding a 5 MB
package for eight paths), a line on what it previews and the member's state.

**Video** came before the new apps because it needs none. YouTube, Vimeo and Loom are
described by their oEmbed endpoints — fixed hosts, so the SSRF guard's resolution path is not
involved, and no YouTube consent page — and play in the message on click. The client builds
the player URL from a validated ID and never renders oEmbed's `html`; `frame-src` lists
exactly the three player hosts.

**Figma** is a private Figma OAuth app — no review, only the organization can authorize it —
with `file_metadata:read` and `current_user:read`, the narrowest pair that names a file and
an account. Its token endpoints take client credentials as HTTP Basic, and its refresh
returns no new refresh token, which the token store already allowed for.

**Confluence** is its own provider, as in Slack, on the same Atlassian OAuth client as Jira:
each person consents to it separately, so connecting one never widens what the other reads.
An admin who has Jira set up adds the Confluence API to that app and ticks *Use the Jira app*;
the client ID and sealed secret are copied on the server and the secret never travels back
to the browser. The card is the page plus its space and the owner's and last editor's names,
fetched in parallel; Confluence's v2 API takes only granular scopes.

---

---

## Wave 17 — Files and images ✅ shipped

### [CS-055] Files and images the way Slack does them ✅ shipped
A file used to leave the moment it was picked, pasted or dropped, as a message of its own whose
body was `[file: name](url)`. That body *was* the link between a message and its file: the
server parsed storage keys out of the text to know what to keep and what to delete.

**Attachments belong to the message.** `files.message_id` is now the only link, and the linked
files are copied into `messages.metadata.attachments` in the message's own transaction, so
every path that returns or broadcasts a message carries them with no join and no new frame.
A send takes up to ten of the sender's own *draft* uploads; an edit keeps a subset; a forward
copies the files under new keys so each message still owns exactly its own; a scheduled
message holds its drafts until a day after its send time. Drafts nobody sends are swept after
a day. Migration 52 moved the old links out of message text — only where the file belonged to
that message, so a forward's quote of someone else's file stayed a link. The composer got
Slack's tray: files upload with a progress bar each, can be removed, and go out with the text;
a pasted screenshot is never sent unseen.

**Images.** Uploads are recognised by their bytes. PNG, JPEG, GIF and WebP up to 30 MB are
processed off the request's thread: absurd canvases are refused before decoding, the upright
size is stored so a message reserves the picture's box, wide images get a 720 px thumbnail
(JPEG, or PNG with transparency — `image`'s WebP encoder is lossless only), and EXIF, XMP and
text chunks are stripped from the original without re-encoding it, keeping a minimal EXIF
block where orientation needs one. A phone photo's location never reaches storage.

**Names, viewing, playing, finding.** File names keep what people wrote — *Izveštaj o
radu.pdf* — with an ASCII key underneath and `filename*` on download. The preview became a
gallery over a conversation's images. Downloads stream from storage and answer byte ranges,
which is what lets video and audio play and seek in the message, and stops a download costing
its file's size in memory. A channel's *Files* panel lists what was shared there by kind.
Building it found that `files` had no index on `message_id`, `storage_key` or
`thumbnail_key` — every download and every message's attachments had been a scan — which
migration 54 added.

Left out on purpose: HEIC conversion (libheif), video thumbnails and transcoding (ffmpeg),
alt text and recorded clips.

---

## Wave 18 — What a Slack user notices in the first hour ✅ shipped

### [CS-056] One set of controls ✅ shipped
The *Browse Channels* dialog's *Close* sat 16 px inside the edge its rows ended on, and at
16 px among 14 px text. It was one symptom: with no shared button, every screen hand-styled its
own, and the copies had drifted — 36 primary buttons, 31 spinners and 17 panel headers, ten of
them with a close button a screen reader announced as "button".

`shared/components` now has `Button` (with `buttonStyles` for links), `IconButton`, `Spinner`,
`PanelHeader` and `DialogHeader`/`DialogFooter`, and every modal and panel is built from them;
the forward dialog, which had no visible title and doubled its padding, got both fixed on the
way. Titles went to sentence case. The migration was mostly mechanical — a script turned the
hand-styled buttons into `<Button>` and the spinners into `<Spinner>`, the six buttons with a
spinner inside became `loading` by hand — and two lint rules keep it from drifting back: a
`no-restricted-syntax` pattern for the primary-button and spinner classes, and a twenty-line
local rule for icon-only buttons without a name.

### [CS-057] Later ✅ shipped
Saved messages and reminders were two panels over two tables, and neither could be finished:
a saved message only left by being deleted, and a fired reminder vanished. Slack's *Later* is
the same idea as one list with a status, and that is now the model: `later_items`, an item
being a saved message, a reminder, or a saved message with a due time, *in progress*,
*completed* or *archived*. Migration 55 moved both tables over (a reminder on an already saved
message folds into that item) and dropped them; the saved and reminder routes went with them
rather than living beside `/later`.

The reminder checker moved from the hooks module to `later`, and it now stamps `reminded_at`
instead of a `delivered` flag, so a fired reminder stays in the list until it is ticked and a
new due time re-arms it. Every change sends `later.updated` to the owner's sessions, so the
sidebar count and the list agree across tabs. On a message, the bookmark shows whether it is
saved and the *More* menu has *Remind me about this*. Slack's `A` to save the focused message
waited for keyboard focus on messages, which [Wave 19](#wave-19--getting-around--shipped) added.

---

## Wave 19 — Getting around ✅ shipped

### [CS-058] Getting around like in Slack ✅ shipped
A Slack user moves without the mouse, narrows a search in the box and keeps up with their
conversations from one *Threads* view. Here there were two shortcuts, a search panel with no
filters, and a reply in your thread was visible only by opening it. Three PRs closed it.

**Keyboard.** One table in `navigation/shortcuts.ts` drives both the handler and the ⌘/
dialog, so the list cannot drift from what the keys do. ⌥↑/↓ walk the sidebar in its own order
and ⌥⇧↑/↓ only the unread entries; Esc marks the open channel read and ⇧Esc the whole
workspace, which needed `POST /workspaces/{id}/read` so it is one request rather than one per
channel. Messages are a single Tab stop with a roving `tabIndex`: ↑/↓ move the focus, the
focused row shows its actions, and `E`, `T`, `R`, `A`, `P`, Enter and Esc act on it — which
also gave *Later* the `A` it had been waiting for. Nothing fires while typing.

**Search filters.** `GET /search` gained `has` (file, image, link — attachments read from the
message's metadata, links by URL in the text), `after` and `before`, and runs on filters
alone. The panel has *From*, *In*, *Has* and *Date* chips, and the same modifiers typed into the
box turn into chips once they are finished (a space or Enter), as in Slack; what it cannot
place stays as words rather than failing silently.

**Threads.** `thread_follows` records who follows which thread and how far they have read.
Replying follows and reads it; the first reply follows it for the root's author, read up to
just before, so that reply is their one unread; a mention follows it for the person named.
Unfollowing is a row, not a deletion, so a later reply by someone else leaves it unfollowed.
The `reply` notification type had existed unused since the notifications work; the worker now
sends it to followers, in-app and push but not email, with mute and DND honoured and the
mentioned left out because they already have the mention. Migration 56 follows every existing
thread for its author and repliers, read up to the upgrade, so it arrives quiet. The sidebar's
*Threads* entry counts threads with unread replies; the view shows each with its root, the
latest three replies, *N new replies* and a reply box, and reads them as they are shown. A
`thread.read_state` frame on the new `events:thread` channel keeps tabs in step.

---

## Wave 20 — Dialogs and large calls

### [CS-059] Dialogs that look finished ✅ shipped
CS-056 had given every dialog the same buttons and close button, but side by side they still
read as five different hands: each picked its own width, the long ones scrolled their title
and buttons away, 67 inputs carried 19 class strings in two heights, confirmations were inline
rows, and one dialog held the profile, the theme, notifications and two-factor with two save
buttons. Three PRs closed it.

**The pieces.** `Modal` has three sizes (400, 520, 720 px), a fixed header and footer around a
body that is the only scroll area, hairlines that appear only while there is more to scroll,
and a 150 ms fade in and 100 ms fade out that `Overlay` runs for the X, Escape, the backdrop
and the dismiss button; a consumer closing it from code closes at once. `Overlay` also holds
the portal, the focus trap and where focus starts (`data-autofocus`, then the first field).
`shared/components/Form` has `Field`, `TextInput`, `TextArea`, `Select`, `Checkbox` and
`RadioCards`, which wire the label, hint, error and counter for assistive technology once.
`ConfirmDialog` focuses its action, shows it busy, and stays open with the reason when the
action fails. The first CI run caught two naming faults the tests had been written for: a
label wrapping its description made the description part of the checkbox's name, and the space
before *(optional)* disappeared inside a span.

**Every dialog, and every field.** No compatibility layer was kept, so every field in the app
moved to the shared controls, sign-in pages and panels included, and a lint rule now refuses a
raw `<input>`, `<textarea>` or `<select>` anywhere else (file pickers excepted). *Create
channel* picks public or private from cards and shapes the name as Slack does, on the client
only. *Browse channels* gained a count, a sort, *Create channel* in the header and one action
per row. Deleting a message or an integration asks in a `ConfirmDialog`; the message one shows
the message and sits outside the row, so its keys never reach the row's shortcuts.

**Profile and Preferences.** *Edit profile* puts the fields on the left and the photo on the
right, and saves the profile and the status with one button: the status is only sent when it
changed, so an untouched status keeps its expiry. *Preferences* is a vertical tab list —
arrow keys, Home and End — over the panels that already saved as they changed. The *You*
menu has both entries.


---

## Wave 21 — Notifications that fit how a team works

### [CS-060] Notification levels ✅ shipped
A channel was muted or it was not, and the decision of what rang was split between the
server, which wrote a row for a mention, and the browser, which played a sound for any
message it happened to see. A direct message without an `@` never reached a phone: push and
email only knew about mentions.

Every channel member now has a level, *All new messages*, *Mentions* or *Nothing*, and every
workspace member a default that a channel follows until it sets its own. One SQL function,
`effective_notify_level`, resolves the two, with DMs falling back to *All*, and the
notification consumer asks it once per message for every member. The answer is a reason —
mention, reply, direct, or a plain message on *All* — and the reason alone decides the rest:
an Activity row, the frame's new `kind`, push, and email for mentions and DMs that reached
no device. The digest counts mentions and direct messages apart. The browser no longer rings
on `message.new`; it rings on the `notification` frame, and stays quiet for the conversation
it is looking at.

Two behaviours changed on purpose. A mention of someone outside the channel used to notify
them about a message they could not open; only members (and thread followers) are notified
now. And *Nothing* is quieter than a mention, as in Slack: no row, no frame, no push. The
dead `notifications` and `is_muted` columns on `channel_members` went with migration 57.

The bell sits in the channel header, the DM header and on a sidebar row's hover, and the
default is the first panel of *Preferences → Notifications*.
