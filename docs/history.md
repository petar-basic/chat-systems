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

## Wave 21 — Notifications that fit how a team works ✅ shipped

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

### [CS-061] Keyword notifications ✅ shipped
The words a person cares about more than any channel — the product's name, *outage*, a
customer — could only be heard by setting every channel to *All*. *My keywords* in
*Preferences* holds up to 50 per workspace, and a message containing one notifies its
reader as a mention does, through the same single decision CS-060 introduced: a keyword is
one more reason, after a mention, a followed reply and a DM, so a message is still one
notification per person, and *Nothing* and mute silence it like the rest.

Matching is one query per message, in the database, next to the `search_normalize` that
search already uses, so "the same word" has one definition: case and diacritics are folded,
the keyword's regex characters are escaped, and it has to stand as a whole word. The
client folds the same way to highlight the word in the message, from a store rather than a
query per message. The mention badge deliberately does not move for a keyword — the
counters are written with the message, and a per-member regex scan does not belong on the
send path — and `docs/frontend.md` says so before somebody files it as a bug.

### [CS-062] Notification schedule ✅ shipped
Do not disturb was a timer, so a working day had to be set by hand every evening, and in
practice people got pushes at 23:00 or turned push off. A person now keeps one window — the
same start and end on the days they pick, in their profile's timezone, possibly across
midnight — and outside it notifications wait in Activity.

"Paused" has one definition, `notifications_paused_until(user, at)` in SQL: the later of the
timer and the schedule's next opening. The push path, the email digest, the `/dnd` status
and the member list all ask it, and every caller passes the instant in, so tests pin the
clock instead of trusting the database's (a Docker VM's clock drifts). The window is
compared in local wall time, which keeps 9:00 at 9:00 on the mornings the clocks change.
*Resume* during a scheduled pause stores when the next window opens and ignores the
schedule until then, as Slack does.

The ticket assumed a pause already shown on the profile card and in the *You* menu; neither
existed, the only control being the Activity panel's moon. The pause controls became one
component used in both places, saying whether the schedule or a timer is the reason, and the
member list carries each person's `notifications_paused_until` for the hover card.

---

## Wave 22 — Finding files ✅ shipped

### [CS-063] Search finds files by name ✅ shipped
Search read message text only, so a file posted without a caption, the usual way, could not
be found by its name. `files.search_name` is the name passed through `search_normalize`,
generated by the database and indexed with trigrams: names are `Q3_budget-final(2).xlsx`,
not sentences, and people type pieces of them. A name containing the typed fragment ranks
first, a near miss after it, then the newest. `%` and `_` are escaped, so they are
characters, not patterns.

The channels a search may look in were a CTE inside the message search; they are now
`searchable_channels()` in SQL, called by both searches, so a guest held to membership and a
DM kept to its participants cannot drift apart between them. A file is found only through a
live message: a deleted message's file and an upload never sent are not.

The ticket wanted the server to own the kind rules and the frontend to keep only icons. The
server does own them (`file_kind_of()`, used for the `kind` filter and returned on every hit),
but the composer tray draws a file before the server has seen it and message attachments are
stored without a kind, so the frontend keeps the same rules. One case table,
`fileKindCases.json`, is asserted by a backend test against the SQL function and by a vitest
against `kindOf()`, so the two cannot disagree silently. `code` became `text`, which is what
the icon already said. The route is `GET /search/files` with the query parameters of
`/search` (`workspace_id`, `channel_id`, `user_id`, `kind` for `type:`), not a path under the
workspace, so the panel builds both requests the same way. The gallery shows images only, so
a video opens at its message, where it plays.

### [CS-064] Search inside files ✅ shipped
A contract named `Agreement_v7.pdf` is now found by the client's name written in it. The
worker reads each attached file once into `file_texts`, a table of its own so the `files` row
every message query touches stays small, and the files search matches the name or the text,
ranking every name match above every content match and returning a snippet of the text with
the words marked.

The PDF crate was chosen by measurement, as the ticket asked: over thirteen real PDFs `lopdf`
was three to nine times faster but lost the spaces a PDF expresses by position, joining words
(14 % of the breaks in one paper), while `pdf-extract` kept them and called an encrypted file
an error rather than empty. Office files needed no library of their own: they are zip
archives of XML, and reading the `t` elements with `quick-xml` in part order gives the text,
with a break at every paragraph, cell and slide so runs split mid-word (`Konfe` + `rencija`)
join and neighbours do not.

The ticket's status list had no state for a file being read. Claiming by inserting a
`pending` row (`FOR UPDATE SKIP LOCKED`, `ON CONFLICT DO NOTHING`) is what makes two workers
never read the same file, and `attempts` with `claimed_at` let a claim left by a worker that
died mid-file be taken again, up to three times, before it is `failed`. A password-protected
PDF is `unsupported` rather than `failed`: it is not broken, the server just cannot read it.
Formats are checked before downloading, so the backfill of an instance full of photos costs
nothing. The snippet is `ts_headline` over a copy of `simple` that unaccents each word, so
`cacak` marks `Čačak` as written; its markers are control characters the extractor strips
from every text, and the server turns them into `{ text, hit }` parts, so a file's text can
reach the browser only as text.


## Wave 23 — Structured work: polls, lists, workflows ✅ shipped

### [CS-065] Polls ✅ shipped
"Lunch Thursday or Friday?" was answered with reactions and a hand count. A poll is now an
ordinary message whose content is the question, so search and history find it, and whose
`metadata.poll` carries the options and their counts, never the voters. A vote rewrites that
metadata and stages `message.updated`, the same path an unfurl takes, so every client sees
the counts move without a poll-specific frame.

The ticket asked for a partial unique index to keep a single-choice poll at one vote per
person under concurrent clicks. An index on `poll_votes` cannot see the poll's
`multiple_choice`, so each vote row carries `single_choice`, copied from the poll, and the
index is `(poll_id, user_id) WHERE single_choice`. It is the backstop, not the mechanism: a
vote locks the poll row first (`FOR UPDATE`), so two clicks serialize, the second replaces
the first, and the counts written into the message are always the counts in the table. A
composite foreign key keeps an option of another poll out of the vote.

`/poll` lives in the browser: the message box reads it before sending, opens the dialog for
`/poll` alone and creates the poll for `/poll "Q" "A" "B"`. The server's slash command list
names it, and a client that sends it as a command gets a hint, because the server-side
commands answer with text and cannot open a dialog. Your own choice is
`GET /polls/{id}/vote`, since the metadata deliberately holds no voters. A poll message
cannot be edited: its content is the question its votes were cast on. A vote after
`closes_at` is refused even before the worker has stamped `closed_at`.

The ticket called the vote button a primitive for CS-067's workflow forms. It is built for
polls; what CS-067 can reuse is the shape: state in its own tables, the summary in
`metadata`, a locked write that stages `message.updated`. The live `message.updated` handler
used to patch the channel list only, so a vote in a thread did not move there; it now patches
the thread cache too.

### [CS-066] Lists ✅ shipped
A launch checklist lived in a pinned message edited by hand, or in a spreadsheet somewhere
else. A list is now a table kept in one conversation, or a personal one only its creator
sees: up to 30 typed columns (text, number, select, multi-select, person, date, checkbox,
link) and 10,000 items. Where a list lives decides who sees it, through the conversation's own
access, so there is no second permission system to reason about, and an announcement
channel's list is read-only to everyone but its posters. It shipped in four PRs.

The model took two decisions the ticket left open. `PATCH /list-items/{id}` merges
(`values || $1`), so two people editing different cells of one row both keep their edit; a
column change locks the list row and a cell edit takes a share lock, so edits run side by side
but never against a column that is changing. A kind change that would clear cells is 409
"This change clears N cells" until `confirm_cleared` is that N: a confirmation given before
somebody else filled more cells cannot clear those. `list.columns_changed` says
`items_rewritten` instead of sending up to 10,000 rows, and every frame carries `owner_id` so
a personal list's frames reach its owner's sessions.

The table is an ARIA grid that keeps focus on itself and points `aria-activedescendant` at
the active cell, so virtualized rows can come and go without losing it; the active and edited
cell are kept by item and column id, so a row arriving from somebody else never moves an
edit. Enter commits and moves down as a spreadsheet does, which put *open the item* on
Shift + Enter and *move the row* on Alt + ↑/↓. Hidden columns ended up shared, on the list,
and views did not take them over.

Views are saved on the list and shared, as in Slack, and always at least one. Filtering and
sorting run in the client over the items it already holds; the server stores the rules,
checks each against its column's kind, and prunes every view in the transaction that changes
or deletes a column, so a saved view never points at something gone. `"me"` in a person
filter is whoever looks, which makes one shared *Only mine* view personal.

Every item has a comment thread, written with the message composer and rendered by the
message renderer; commenting takes the right to edit the list, and
`list_items.comment_count` moves in the comment's transaction, so the row's own frame carries
the count. A mention, or a person cell set to somebody new by somebody else, becomes a
`list` notification through the CS-060 consumer, for whoever can still read the list, and
opens the item. The due date is one date column a list marks; the item's title is its first
text column and *done* is its first checkbox. `list_due_assignments()` answers, in each
assignee's own time zone, which open items are due by tomorrow and for whom; the reminder
checker turns those into Later rows (09:00 on the day) and drops the ones no longer due,
assigned or open. Waiting ten minutes for that sweep after assigning somebody felt broken, so
writing an item or a column of a list with a due date also syncs that one list at once; the
sweep is left for the day turning over. Ticking the reminder off in Later checks the item
through the ordinary item update, so it is only done for someone who may still edit the list.

### [CS-067] Workflow builder ✅ shipped
The coordination a team repeats by hand (greet whoever joins `#general`, post "standup in 10"
at 9:50, turn a 🚨 reaction into a post in `#incidents`, collect a time-off request and keep
it in a list) is now built in the app by whoever needs it, without code. It shipped in four
PRs: the engine with schedule and webhook triggers and the builder; reaction and member-joined
triggers; shortcuts and forms; list steps and templates. The `bot` and `scheduled` members of
`hook_type`, declared in the initial schema and used nowhere, are gone.

A workflow is a typed `Definition` (serde-tagged triggers, steps and recipients) with its own
check rather than `garde`, so a refusal names the step: "Step 2: {{webhook.x}} isn't
available for this trigger". Saving also checks, as the builder, everything a step would do:
the channel it posts in, the person it DMs, the list it writes to. A new workflow is a draft
until *Publish*; editing a published one keeps it on and bumps `version`, and a run that
resumes after an edit stops rather than finishing someone else's version. Every step runs
with what its builder can do at that moment, so losing access fails the step and the builder
leaving the workspace turns their workflows off and tells the admins.

Every run is a row unique on `(workflow_id, trigger_key)`: the scheduled instant, the
webhook's `Idempotency-Key`, `event:<stream event id>` for a reaction or a join, so a second
delivery never starts a second run. Each step commits together with its `workflow_run_steps`
row, so a run picked up again after a crash never repeats a step. A schedule's next instant
is computed in Postgres (`workflow_next_run()`, DST-correct); one missed by more than 15
minutes is skipped rather than posted late. Nothing emitted a join before, so adding a member
to a channel now stages a durable `channel.member_joined` event, which `workflow_listener`
reads through its own consumer group. Workflow messages never start workflows, no step adds a
reaction or a member, and past 30 starts a minute a trigger is recorded as a skipped run.

A form step parks the run as `waiting` with the form in `workflow_forms`; the submit records
the step and queues the run again, so it continues within one runner tick, and a form nobody
answers expires after 7 days. Shipping it found a real bug in CI: a refused submit dropped a
transaction holding `FOR UPDATE` on the run, sqlx rolls a dropped transaction back
asynchronously, and the runner's `SKIP LOCKED` claim passed over a run that was free. The
submit now rolls back explicitly before answering.

`add_list_item` fills up to 30 columns of a list its builder can write to; `update_list_item`
names the earlier step that added an item (`from_step`) and sets a person, select or checkbox
column on it, which is how a request becomes *Approved* or gets an owner. The item's id is
kept in `workflow_run_steps.list_item_id`. Text and link columns take any text with variables;
every other column takes one value or one variable. A typed variable is checked when saving:
`{{trigger.user}}` and a person answer fit only a person column, a date answer only a date,
and a choice answer only a select that has every option the question offers. A text variable
fits any column and is checked when the step runs, so "“Days” needs a number, not “three”"
fails that step in the history. The item goes through the list's own `create_item_in` and
`update_item_in`, inside the step's transaction, so its frames, assignment notifications and
due-date reminders behave exactly as for an edit by hand.

The builder's *New* opens a choice of a blank workflow or four templates (welcome new
members, daily standup reminder, incident from a reaction, time-off request into a list),
each an ordinary draft to change before publishing. A list step lists the columns of the
picked list, and *Fill from the form answers* matches columns to questions by name. Moving
or removing a step keeps every update step pointing at the step that adds its item.

## Wave 24 — Ready for a company ✅ shipped

### [CS-068] Workflows without drafts ✅ shipped
*Save draft* saved a workflow that did not run, but only once it was complete and valid, so
it was not a draft in any sense a person expects; and the editor stayed open over the list,
so the saved row was out of sight and, once closed, looked exactly like a workflow somebody
had turned off. Drafts are gone. *Create workflow* saves and turns it on in one statement
(`next_run_at` computed in the insert), and an existing one has *Save changes*, which keeps it
on or off, and *Turn off* / *Turn on*. `POST /workflows/{id}/publish` became
`/workflows/{id}/enable`, the pair of `/disable`; nothing outside this repository used it in
the two days since 3.3.0. A draft saved before the upgrade is simply a workflow that is off.

### [CS-069] Small UI fixes and a README that says what ships ✅ shipped
What a new team sees in its first hour. The channel header had *Channel members* twice, the
count and an icon, and a pin count beside a pin icon; now the count is the one members
button, the pin button carries the count, and *Channel settings*, used by moderators and
rarely, moved into the ⋯ menu, which on desktop only a moderator has. Every icon has a
tooltip. The rail showed one letter on a grey tile, so three workspaces named D… were three
identical tiles; a workspace without an icon now gets a tile coloured from its id, from the
avatar palette, and up to two letters (`Dev Team` → `DT`, `Design` → `De`), with its name
as the accessible name.

A workflow's form summary was a stack of bold labels and answers, taller than the screen for
five questions. The summary step still appends the same Markdown, which stays the content
so search, notifications and other clients keep working, and also stores the answers as
`metadata.fields`. The web client lays those out as a two-column grid, a long answer across
both, and splits the intro off the content only when the content still ends in exactly that
Markdown; anything else renders as it is. `create_bot_message_in` takes the whole metadata
object now rather than building `{ bot }` itself. The README's *Features* lists what ships as
of 3.3.0, grouped the way the app is.

### [CS-070] Workspace admin in one place, with the settings that had no screen ✅ shipped
An admin's work was nine entries of the workspace menu, each a narrow right panel, and part
of what a company needs existed only as an API. Three PRs. The first put *General*,
*Members*, *User groups*, *Custom emoji*, *Apps*, *Integrations*, *Slack import* and *Audit
log* in one *Workspace settings* dialog built like *Preferences*, saving as you change a
setting, with `?settings=<section>` as a deep link; a member gets the four sections they use.
Escape now closes only the topmost layer, from one stack.

The second gave the API-only settings a screen. *Data retention* (owner) shows what a change
would remove now, counted by the purge's own queries through
`GET /workspaces/{id}/retention/preview`, before it saves; it saves with a button, the one
place that does, because nothing it deletes comes back. *Exports* lists past exports from a
new `GET /workspaces/{id}/exports`, with the single-use link only for whoever asked for it.
An instance admin exports or erases a person from their *Members* row, the erasure behind a
list of what goes and their name typed; both stay instance-admin only, as the endpoints were,
since a person's export is everything they wrote on the instance. *SCIM provisioning* mints,
rotates and revokes tokens, each shown once, and the token routes moved onto the OpenAPI
router.

The third lets a channel keep its messages for its own period. The owner allows it beside
the policy (`retention_policies.channel_overrides`); a channel moderator then sets a number
of days, forever, or the workspace default in *Channel settings*, stored in
`channel_retention` (`NULL` days meaning forever), announced in the channel by a message and
audited as `retention.changed` with the channel and both ends. The purge and its dry-run
count resolve each message's period in the same `DELETE` — a `LEFT JOIN` on
`channel_retention` gated by the flag — so a workspace kept forever still trims a channel
that asked for it, and a channel kept forever outlives a workspace period. While overrides
are off the rows stay and are ignored. Files and list history keep the workspace period.

### [CS-071] First run, and an empty workspace that says what to do ✅ shipped
A new instance got its first admin only from `ADMIN_EMAIL` and `ADMIN_PASSWORD` in `.env`.
Leave them out and nobody could sign in; set them and the password stayed in that file for
the life of the instance. Two PRs. The first gave an instance with no instance admin a
setup token, logged on every start until it is used, and a `/setup` page that takes it with
the first admin and the first workspace. The table keeps a nonce and the token is an HMAC
of it under `JWT_SECRET`, so a database dump does not hand anyone the instance and every
replica prints the same one. It is claimed by one `DELETE … RETURNING` guarded by "no
instance admin yet", and put back if the account cannot be created. The environment
variables still work for scripted installs and are optional now. *Send a test email*, at
the end of setup and in *Instance admin → Email*, sends past the outbox and names what went
wrong: nothing listening at `SMTP_HOST:SMTP_PORT`, a TLS mode that does not match the port,
or credentials refused.

The second tells a new workspace's admin what comes next. While a workspace has fewer than
three members or no channel but `#general`, its admins see *Get <workspace> going* above
the messages: *Invite your team*, *Create a channel*, *Set an icon* and *Connect SSO*, the
last a link to the RUNBOOK. Each step is ticked from what is already there (a pending
invite or a second member, a channel of its own, an icon, SSO configured), not from a
record of clicks, so doing it anywhere counts. Dismissing it is remembered in the browser,
like the app-connect prompt. An empty channel now says what it is for (its description or
topic, or what `#general` is) and offers *Add people* and *Add a bookmark* to whoever may
do them: in `#general`, or while the admin is alone in the workspace, that is inviting to
the workspace; elsewhere the channel's member list opens already adding. Creating a channel
now opens it: the app used to navigate before the channel list knew about it and fell back
to `#general`.

### [CS-072] Fewer round trips on start and on switching channels ✅ shipped
Signing in made 31 requests before the first channel was drawn, two of them twice, and each
switch to a channel not opened before made eight: the channel's members, pins, bookmarks,
lists and workflow shortcuts one by one, then the messages, a call marking the channel's
notifications read and a second fetch of the unread count it had just changed. One PR,
measured before and after with `frontend/scripts/measure-requests.mjs`; the numbers are in
the RUNBOOK. `GET /channels/{id}/context` answers the five in one request with the same
authorization as each, except that it gives an empty shortcut list instead of a 403 in a
channel the caller can read but not post in. The five hooks ask for the context when theirs
is the first fetch for the channel, and the context seeds every key, so the components
still read the keys they read before. When the context is under 30 seconds old, a fetch can
only be an invalidation of one key, so it goes to that key's own route. The newest page of
messages carries the read mark: `mark_read=true` moves the read position, marks the
channel's notifications read and answers with the workspace's notification count, which
replaces the two separate calls and the refetch. A channel opened from the address bar or
on start is now marked read on the server too. Before, only a click in the sidebar did
that, and only once the messages were already loaded. The sign-in page reads
`/instance/info` through the same query as the rest of the app. Start is down to 24
requests with none twice, a first switch to two, and a switch back to one.

### [CS-073] One frontend structure ✅ shipped
Code for one feature sat in three places: `components/`, `hooks/queries/` and `features/`,
and which one a new file went into depended on who wrote it. Three PRs. The first moved
every file in `components/` and `hooks/queries/` into the feature it belongs to, with a new
`features/instance` for sign-in, instance administration and the install prompt, and
`models/` into `shared/models/`. The second split the two files that did several jobs:
`MessageItem` is now the row, with its actions bar, body, editor and reactions beside it,
and `HuddleWindow` is the frame around the stage, its tiles and the control bar. The third
adds `structure/feature-structure`, an ESLint rule that refuses a file outside `features/`,
`shared/`, `pages/`, `lib/`, `stores/`, `api/`, `test/` and `types/`, and a feature
importing another feature's files except through its `index.ts`. 108 imports in 56 files
went through a barrel for the first time, and `instance`, `later`, `polls` and `search`
gained one. A barrel imports its whole feature, so features that use each other now form
import cycles; that is safe while no module reads another feature's export as it loads, and
the one that did, search's file-kind filter, now reads `lib/fileKinds`, which moved out of
messaging because both use it. `docs/frontend.md` describes the layout once, at the top.

### [CS-074] A channel shows what posts into it, and its admin can add a webhook ✅ shipped
Every integration lived in the workspace admin's panel and nothing in a channel said which
of them touched it, and making any hook took a workspace admin, so the team that owns
`#deploys` asked one each time it wanted a CI notification. Two PRs. The first added
`GET /channels/{id}/integrations`: the incoming webhooks bound to the channel, the outgoing
webhooks and slash commands whose `channel_ids` include it, and the workflows whose trigger
listens to it or whose `send_message` step posts into it, one SQL query per kind, for anyone
with access and with no URL, token or secret in it. `authz::can_manage_channel_hook` lets a
channel admin create, reveal, rotate and delete the incoming webhooks of their own channel;
outgoing webhooks and slash commands send what people type to a third-party URL, so they
stay with workspace admins, and every write keeps its audit row. A private channel the
caller is not in answers 403 like every other channel route, not the 404 the ticket named.
The second turned *Channel settings* into *Channel details*, which everyone in the channel
opens from ⋯: *About* (editable by a moderator, text for everyone else), *Settings*
(announcement channel and message retention, moderators only) and *Integrations*. A channel
admin adds an incoming webhook there and sees its URL once, under it; a workflow row opens
the workflow in the *Workflows* panel for someone who may build them; the *Integration* badge
in the header opens the tab. The workspace *Integrations* section links each hook's
channels.

### [CS-075] Who may notify a whole channel, and who may make one ✅ shipped
Anyone who could post could ping a whole channel with `@channel`, `@here` or `@everyone`, and
any member could create channels, so a company of a few hundred had no way to keep either to
the people meant to. Two PRs. The first put three rules beside `workflow_builders` in the
workspace settings, `broadcast_mentions`, `public_channel_creators` and
`private_channel_creators`, all `WhoMay` (`members` by default, or `admins`), and a channel's
own `allow_broadcast_mentions`. `authz::broadcast_refusal` gives the reason: a channel that
turned broadcasts off refuses everyone, admins included; an admins-only rule leaves workspace
admins and the channel's admins. A message, poll or scheduled message the author may not
broadcast is refused with 403 and that reason; a workflow is refused when it is saved, and
anything already queued still posts but notifies nobody, since `expand_mentions` checks the
same rule. Creating a channel checks the rule for its kind. The second is the frontend: a
*Permissions* group in *General*, the channel switch in *Channel details → Settings*, a
composer that leaves the broadcasts out of its suggestions for someone who would be refused
and asks before notifying a channel of more than 25 people, and *Create channel* offering only
the kinds the person may make. `lib/channelPermissions.ts` mirrors the server's checks so the
UI does not offer what will be refused; the server stays the boundary. A refused send now
shows the server's reason instead of *Message failed to send*.

## Wave 25 — One person, several workspaces ✅ shipped

### [CS-076] A profile and notification settings of your own in each workspace ✅ shipped
A status set in a hobby workspace showed at work, and pausing one workspace silenced all of
them, because the name, photo, bio, status and every notification setting lived on the
account. Slack keeps them per workspace, and now so does this. What stays with the account
belongs to the person or the device: email, password, second factor, SSO, time zone, theme,
sound, desktop notifications and push subscriptions. Two PRs, each backend and frontend, so
`main` worked between them.

The first moved the profile. Migration 73 gives `workspace_members` a name, photo, bio and
status, copies each live status into every workspace the person is in (a custom emoji only
where that workspace has it) and drops the status from `users`, along with
`user_preferences` and `bookmarked_messages`, which nothing had read since *Later* replaced
the second. Name, photo and bio fall back to the account's when a workspace has none, so a
name changed by SCIM or SSO still reaches every workspace nobody customised; the status has
no fallback. `GET`/`PATCH /workspaces/{id}/profile` and `PUT`/`DELETE
/workspaces/{id}/profile/status` replaced `/users/me/status`, and every read of a person
inside a workspace — the member list, push and email names, a workflow's mention, list
exports, the audit log, the Slack import's mapping — goes through the workspace profile.
*Edit profile* edits the current workspace's and says so, with the account's values as
placeholders and *Use account photo*; *Preferences → Account* edits the account's.

The second moved notifications. Migration 74 gives `workspace_members` the `/dnd` timer, the
schedule override and the mention-email switch, rebuilds `notification_schedules` keyed by
workspace and member (cascading when someone leaves), copies today's values into every
workspace, and drops them from `users`. `notifications_paused_until(workspace, user, at)`
is still the one definition of paused, now per workspace, and push, the email digest, the
huddle ring and the member list pass the workspace they are about. `/dnd` pauses the
workspace of the channel it is typed in. The routes moved under `/workspaces/{id}/notifications/`,
and *Preferences → Notifications* names the workspace it changes.

## Wave 26 — What the survey found ✅ shipped

### [CS-077] Access that holds: huddles, email invites, private channels ✅ shipped
A product survey after 3.5.0 found four checks that did not hold. The gateway authorised
`huddle.join` against the `channel_id` the client sent, so anyone in a channel could join a
private channel's huddle by naming its id next to their own channel, and `active-huddles`
handed those ids out. The join now looks the huddle up in `huddle_sessions` and asks whether
the caller is in its workspace and in its channel, one of its two DM people, or invited;
migration 75 adds `huddle_invites`, written by the invite route, which now needs an open
huddle and an inviter who is in it. `active-huddles` lists only channels the caller is in.

An emailed invite was a seven-day registration JWT carrying its own role and never compared
with the invite row, so revoking it changed nothing. The token now carries only `invite_id`
and expires with the invite; verify, accept and complete-registration load the row, refuse
one that is gone, expired or used up, take the role from it and claim a use. Links sent
before the change are refused and the changelog says to re-send them.

`GET /channels/{id}/members` took workspace membership as enough; it now uses
`require_channel_access`. The Slack import merged a channel into any same-named one, private
into public included; it now merges only into the same kind and otherwise creates
`<name>-slack`, naming the rename in the report.

### [CS-078] A suspended or removed person stops acting ✅ shipped
Suspending an account revoked its sessions and app connections and nothing else: its
scheduled messages still sent, its workflows still ran, and its outgoing webhooks kept
forwarding private channels to somebody else's server. Scheduled messages and workflow runs
re-checked membership and channel access when they fired but never the account's status, and
the hook consumer checked nothing at all.

`authz::still_acting(workspace, user, channel?)` is now the one question every executor asks
when it fires: is the account active, still in the workspace, and able to read the channel. A
scheduled message fails with `author_inactive`; a workflow run fails, and a builder who is
suspended or gone turns the workflow off and tells the admins; an outgoing webhook, or an
incoming one when it is called, is turned off with the reason in `hooks.disabled_reason`
(migration 76), which *Integrations* shows. Suspending or erasing an account turns off every
hook and workflow the person made, audited as the admin who did it; removing someone from a
workspace does the same there, now hooks included. Activating the account turns none of them
back on.

A workflow whose builder had left could only be deleted, since only its builder could edit or
enable it. `Workflow.creator_gone` says so, and `POST /workflows/{id}/owner` lets a workspace
admin take it over — *Take over* in the editor — leaving it off for its new owner to check and
turn on, with `workflow.taken_over` in the audit log.

### [CS-079] Opening a message lands on it ✅ shipped
A push, an Activity row, a desktop notification or a search hit opened
`/app/{ws}/{channel}/{message}`, and the router only knew the channels in the sidebar: a DM or
a channel the person had not joined redirected to `#general`, and a thread reply, which is not
in the channel's timeline, was paged for forever. Search stopped at twenty.

`GET /messages/{id}/location` answers where a message lives — workspace, channel, `channel`,
`dm` or `group_dm`, its thread parent and whether the caller is in the channel — and a 404 for
one the caller cannot read. `useOpenMessage` resolves it and navigates, and every entry point
uses it: Activity, desktop notifications, search (messages and files), *Later*, pins and
channel files. The message route resolves through the same query, so an old link to a DM
moves to `/app/{ws}/c/{conversation}/{message}`, a reply opens its parent with the thread
beside it scrolled to the reply, and a gone message says so and opens its channel. A public
channel not in the sidebar now opens as a preview with a *Join channel* bar in place of the
composer, from a link or after leaving it. Push payloads and the `notification` frame carry
`thread_parent_id`, which the service worker turns into `?thread=1`. Search pages with
*Show more* for messages and files.

### [CS-080] Threads that keep up ✅ shipped
The thread panel loaded fifty replies and stopped. `reply_count` only went up, an open thread
did not see other people's reactions or deletions, and when retention or an erasure removed a
thread's parent its replies fell into the channel as top-level messages. There was no *Also
send to #channel*, and the thread composer could not schedule.

`GET /messages/{id}/thread` now pages newest first by cursor, with reactions, and the panel is
an infinite query with *Show earlier replies*. Deleting a reply takes it off the parent's count
in the same statement and announces the parent; the thread caches take message, reaction, pin
and delete frames through the same patch as the channel. A thread is a unit (DAT-1): a parent
with live replies is kept as a tombstone with its content and metadata emptied, so the thread
stays listed, followed and repliable, and retention removes root and replies together once the
newest of them is past the period. An erased person's answered roots become tombstones the
same way. `also_to_channel` marks a reply `metadata.also_in_channel`; the channel lists it with
a *Replied to a thread* line that opens the thread on it, and it notifies like a channel
message. Scheduled messages carry `thread_parent_id` and `also_in_channel` (migration 77) and
deliver as replies, failing with `thread_gone` when the thread no longer exists.

### [CS-081] Sending never loses work ✅ shipped
A failed send kept its row but *Retry* dropped its files, a thread could not retry at all, and
a refetch while a row was failed removed it with its text. Composer files were cleared on
switching channel while the draft text stayed, and a failed upload was dropped silently at
send. A failed scheduled message looked like it was still waiting, and both *Cancel* and
*Reschedule* answered an error. Forwarding quoted the original, so its mentions notified
again.

A failed send now also lands in an unsent store keyed by channel or thread, merged into the
list and the thread panel, so a refetch cannot drop it; *Retry* resends with the same client
id and files, *Discard* puts the text into that composer's draft and the files back in its
tray. Composer trays are a store keyed by channel, conversation or thread: uploads keep going
while another channel is open, and a failed upload keeps its file and reason and blocks
*Send* until it is retried or removed, named by its cause (the connection dropped, the server
did not answer). A row not yet on the server offers no reactions, thread, pin or save, which
would refer to an id the server never saw. A failed scheduled message can be deleted,
rescheduled (queued again) or sent with `POST /scheduled-messages/{id}/send`, which answers
the reason when delivery is refused; *Scheduled* shows it with *Send now* and *Remove*. A
forward sends `forwarded_from` (formerly `copy_attachments_from`), recorded as
`metadata.forwarded_from`, and the server reads mentions and broadcasts only from the lines
outside the quote.

### [CS-082] Unread, Activity and the badge agree ✅ shipped
A mention in the channel already open still left a badge and an unread Activity row, reading
a channel also cleared its thread mentions while reading a thread cleared nothing, and thread
replies counted toward the channel's unread even when they were never shown there. The app
icon badge flipped between one workspace's count (push) and the total (the app), a realtime
mention from another workspace could land in the open one's lists, and a channel always
opened at the bottom, with no divider on a cold load.

One rule now, written down in `docs/frontend.md`: a channel notification clears when the
channel is read, a thread notification when the thread is read, and the message on screen
counts as read. Reading a channel, a conversation or a whole workspace marks only its channel
notifications read; `POST /threads/{id}/read` marks that thread's and always announces the
new state. A reply counts toward the channel only when it is also sent to the channel. A
message that arrives in the channel or conversation open in a focused window is read up to
the newest delivered message, debounced per channel. Realtime read state and new messages
for a channel that belongs to another workspace leave the open workspace alone, and push
`badge_count` is the total across workspaces. `GET /channels/{id}/messages?mark_read=true`
answers `last_read_before`, so a cold load opens on the first unread with the *New* divider,
and *Jump to first unread* shows while the divider is above the view.

### [CS-083] Notifications that arrive once, where they should ✅ shipped
*Email me what I missed* was queued only when no push device accepted the notification, so a
push the service took but nobody saw never became an email, and the digest dropped everything
the moment the person had any socket open, read or not; its link opened `/app`. Push was held
back whenever any socket was open, including a tab left in the background for hours. A pause
silenced the push for a huddle ring but not the ring the gateway delivers, and the pause menu
did not say which workspace it paused. A webhook posts under its creator's id, so the creator
was never notified of it, and a notification about a bot message was titled with the
creator's name. The in-tab desktop notification switch had no control.

A mention or DM with email on is queued once per person and message (a unique index), whatever
happened to push. The sweep drops the rows whose Activity notification has been read and
sends one digest per person and workspace for the rest, claiming the rows and queueing the
email in one transaction, with a link to the message when there is one and to the workspace
otherwise. The client reports `activity` over the socket — active while the page is visible
and has had input in the last two minutes, idle as soon as it is hidden — and the gateway
keeps a per-person hash of connection expiries refreshed on its heartbeat; push is held only
while one has not expired. Reporting hidden at once is what makes a phone's background socket
harmless: how long the browser keeps it no longer matters to push. A ring to a paused person
carries `quiet` and the gateway drops it, while the `Call` row still lands in Activity; the
pause menu reads *Pause notifications in {workspace}*. A message with `metadata.bot.hook_id`
excludes nobody as its author, and every notification about a bot message carries the bot's
name. *Preferences → Notifications* gains *Desktop notifications while the app is open*, and
the in-tab notification takes its channel as its tag, as the push does, so the two replace
each other.

### [CS-084] Reconnecting, presence and new versions ✅ shipped
After a laptop slept, the socket waited out its backoff — up to thirty seconds — and a socket
the sleep left half-open still looked open until the server's heartbeat gave up on it.
Presence was written for the workspaces a person belonged to when the socket opened, so a
workspace joined later showed them offline until they reconnected. The README promised a
replay of 10 000 events while the gateway stopped at 1 000, and the fallback refetch left
unread counts, pins and the sidebar stale. After a deploy an open tab still ran the old
bundle, and a lazy page or the emoji picker asked for chunks the new image no longer had.

The client reconnects at once on `online` and when the tab is shown with a closed socket, and
probes one that still looks open with a `ping`, replacing it if nothing comes back within two
seconds. `subscribe` marks the person online in that workspace before the roster is read and
adds it to what the heartbeat refreshes and the disconnect clears. The stream keeps 10 000
entries for the worker's sake (about 0.8 KB each, measured, so 8 MB for a busy workspace) and
a client is replayed at most 1 000; the README now says both, and the gap fallback
invalidates every key a missed event could touch. nginx serves `index.html` and
`version.json` uncached; the client compares `version.json` with the build it is running when
the tab returns, the network does and every ten minutes, and offers *Reload*. A
`vite:preloadError` reloads once (guarded through `sessionStorage`, so a chunk that is really
missing surfaces instead of looping), and the release image copies the previous release's
chunks in beside its own, listed in `release-assets.txt` so they last one release and do not
pile up.

### [CS-085] Archive, export and retention that say what they did ✅ shipped
Archiving a channel only hid it from the sidebar: the composer stayed, posts, reactions,
scheduled messages and webhooks still landed, nobody else viewing it heard, and there was no
way back. An export without DMs still listed the files posted in them, and ignored its own
date range for files. The retention job added each workspace's purge to one running total and
audited that total, so every workspace's row also counted the ones purged before it.

`authz::can_post` refuses an archived channel and `require_not_archived` guards reactions and
incoming webhooks; the scheduled executor fails such a message with `channel_archived`.
Archiving (`DELETE /channels/{id}`) and the new `POST /channels/{id}/unarchive` share one
transaction that flips `is_archived` only if it changes, posts a system line
(`channel_archived` / `channel_unarchived`) and stages `channel.updated`; the audit log gains
`channel.unarchived`. The client patches the open channel from that frame, so the composer
becomes an *Archived* bar with *Unarchive* for workspace and channel admins, and the person who
archives stays in the channel instead of being sent to `#general`. *Browse channels* lists
archived channels to read, without *Join*. `files.jsonl` joins each file to its message's
channel and applies the same DM filter and `since`/`until` as `messages.jsonl`. The purge
counts each workspace on its own, audits that, and adds it to the run's total only for the log
and the metrics.

Checking it live turned up an older gap: opening a channel you are not in sends `channel.join`,
which the gateway refuses, but the client remembered the channel as joined, so after *Join
channel* it never asked again and the channel stayed silent until a reconnect. A successful
join now calls `rejoinChannel`, which forgets the refused id and sends it again, and refetches
the messages the preview could not hear.

### [CS-086] Getting people in ✅ shipped
An instance without SMTP said *Invite sent* and the link existed only in a log line. An invite
link signed in through SSO forgot the invite on the way to the provider, so a new SSO user
landed in no workspace. *Forgot password* sent a reset to people who had never set a password
(the invited but unregistered, who it could not let in, and SSO accounts), and the reset
spent its link before checking the new password, so a typo of a short password burned it.
Pending invites could not be seen or revoked in the app.

Creating an email invite returns its `invite_url` and `email_sent`; `can_send_email` is false
for an empty `SMTP_HOST`, which is now how an instance runs without email. A pending email
invite's link can be fetched again (`/invites/{id}/link`), minted from the invite's own expiry
so revoking it still ends every copy. *Members* shows the link after inviting and a list of
pending invites with *Copy invite link* and *Revoke*. `/auth/oidc/start?invite=` stores the
link token with the pending login in Redis; the callback redeems it for the account that came
back, which SSO activates if it was still pending, and lands on the workspace — or back on the
invite page when the provider's address is not the invited one. `forgot_password` returns
whether the account is still waiting on an invite, and the route re-sends that invite;
password-less accounts get a note about SSO instead; `reset_password` validates before it
consumes the token.

### [CS-087] A photo that shows everywhere ✅ shipped
An account photo was uploaded as a file of whichever workspace was open, so the others got
`403` for it and that workspace's file retention deleted it with its old files. `is_avatar`
made the file readable inside its own workspace by matching `users.avatar_url`, which is the
one place an account photo was never needed alone.

`POST /users/me/avatar` takes the photo itself: sniffed, decoded and stripped of metadata like
any image, shrunk to 720 px when wider, and stored as `avatars/{user_id}/{uuid}.{ext}` with no
`files` row, which is what keeps retention, file search and attachments away from it. It is
set on the account in the same call and the previous object is deleted, as it is when
`PATCH /users/me` replaces or clears a photo and when an account is erased, so no orphan is
left behind. The download route reads the owner out of the key and lets through the owner
and anyone who shares a workspace with them. `is_avatar` now only matches workspace photos,
which stay workspace files (CS-076). The move of existing photos needs the object store, so it
is a worker start-up task rather than a SQL migration: it copies each account photo still
under a workspace prefix, swaps the URL only if it has not changed meanwhile, and leaves the
original to its workspace's retention; it runs to nothing on the next start.

CI caught a race in *Edit profile* on the way: with the profile query not yet enabled the
fields rendered empty, and the profile's arrival reset the status a person had started to
pick. The form now waits for the profile, as its *Save* button already did.

### [CS-088] A Slack import that keeps what it found ✅ shipped
The import made Slack's guests full members and skipped every deleted account with no address,
taking its messages with it. Archived channels came in open, reactions knew fifteen standard
emoji and left every other one as text, and an import that was running when the worker
restarted said `running` for ever, with nothing to start it again from.

`is_restricted` and `is_ultra_restricted` users join as guests. A deleted account is created
suspended — under `slack-<id>@deleted.invalid` when the export kept no address — so its messages
keep an author and a name, and nobody can sign in as it or claim it through an invite; an
existing account matched by email is not touched. A channel archived in Slack is archived once
its messages are in, and on every re-run; one merged into an existing channel stays open and
the report says why. Reactions resolve through `emoji_shortcodes.tsv`, the emoji-mart set the
composer uses, generated from `@emoji-mart/data` with its aliases and skin tones, and anything
else keeps its `:name:`, which is how a custom emoji renders; a name that is neither is reported
once. The worker beats `slack_imports.heartbeat_at` every 30 seconds around a run and, before
each claim, fails `running` rows silent for five minutes. Since a failed run keeps its archive,
`POST …/retry` puts it back to `pending`, and the import's idempotency turns the second attempt
into a resume.

## Wave 27 — What the second survey found ✅ shipped

### [CS-089] Gone means gone: SSO, erase, demotion, workflow links ✅ shipped
A survey of 3.6.0 found four ways to keep what had been taken away. `link_or_create` returned an
account found through its `oauth_accounts` link before looking at its status, so a suspended or
erased person signed in through the provider got an access token for its whole lifetime. A hard
erase kept an answered message as a tombstone but left its `message_edits`, so every earlier
version of the text was still there, along with the person's files (objects included), reactions
and the rest. The hook executor's `still_acting` asked whether the creator was active, a member
and able to read the channel, not whether they still held the role the hook needed. And a
taken-over workflow kept its `webhook_token`.

`link_or_create` refuses a suspended account whichever way it found it, and erasing one deletes
its SSO links. The hard erase moved from `MessageRepo::erase_author` to
`ExportRepo::erase_writing`, one transaction that removes the notifications quoting the person's
messages, their pending mention emails, every edit of their messages, their reactions, poll
votes, list comments, scheduled messages and *Later* items, then tombstones or deletes the
messages as before; `files::service::erase_uploads` then deletes their stored objects and file
rows. `removed` in the response and the audit row counts each. `authz::hook_still_acting` adds
`can_manage_hook` to the creator check and stops a hook with `Stopped::Demoted`; the executor and
the incoming webhook use it. Taking over a workflow mints a new token, and
`POST /workflows/{id}/rotate` (*New URL* in the editor, `workflow.link_rotated` in the audit log)
mints one on demand. The ticket said the old URL would answer 404; it answers 401, as any unknown
token always has.

### [CS-090] A Slack import that respects what is already here ✅ shipped
CS-077 stopped a private Slack channel merging into a public one, not into a private one: a
private Slack `#hr` landed in the local private `#hr`, its Slack members were added, and both
sides read each other's history. CS-088 left a merged channel open when Slack had it archived,
but the mapping that run recorded sent the channel down the reuse path on any later run, and
that path queued every mapped archived channel for archiving. And someone who had left Slack,
imported suspended with no password, came back from an unsuspend as an active account nobody
could get into: login refused it, *Forgot password* sent an SSO reminder, an invite found the
account already there.

`place_channel` merges only public into public; anything else takes `<name>-slack`, and the note
says a private channel never joins one already here. Migration 80 adds
`slack_channels.created_by_import`, `load_existing_mappings` reads it into `created_channels`,
and the reuse path archives only a channel the import made. `AuthRepo::reactivate`, used by the
admin *Activate* and SCIM `active: true`, sets an account with no password and no
`oauth_accounts` row to `pending`, where the invite flow finishes it.

### [CS-091] An archived channel is read-only all the way ✅ shipped
CS-085 closed posting, reacting, scheduling and incoming webhooks in an archived channel and
left the rest: editing, deleting and pinning a message, its link previews, poll votes and
closing, file deletion, renaming, members and their roles, bookmarks and huddles all still
wrote. And an archived private channel had no way back, since the sidebar leaves archived
channels out and *Browse channels* listed public ones only.

Every one of those writers calls `authz::require_not_archived` after its permission check, so
the refusal is the same 403 in each. The client reads `is_archived` off the open channel
(`useChannelArchived`) and drops the controls: message actions lose reacting, pinning, editing
and deleting, reaction pills and poll options go inert, and the header, members panel and
bookmark bar lose their writers. `list_browsable_channels` adds archived private channels the
caller is a member of, ordered under the archived public ones, and opening one lands on the
archived bar with *Unarchive* for its admins.

The on-screen mention E2E turned up a race on the way: the messages query resolves only after
`settleChannelNotifications`, so a `message.new` arriving between the GET and that settling found
no cache, and `patchChannel` dropped it while the page it was waiting for had been read before
the message existed. `message.new` now invalidates the channel's messages when there is no cache
yet, which refetches an open channel and leaves one nobody is looking at alone.

### [CS-092] Read state that matches what you read ✅ shipped
The read position is the message the *New* line is drawn after, and three writers each had their
own idea of it. `mark_workspace_read` took the channel's newest message, a thread-only reply
included, which the list never finds, so it paged back to the start of the channel looking. The
conversation read route moved the counters and left the position where it was, so a DM opened at
an old line. And a mention in a reply also sent to the channel kept its notification after the
channel was read, because reading a channel cleared only notifications with no thread, which is
also what the mention email checks before it goes out.

Both writers now take the newest message the channel shows, top level or sent to the channel,
not deleted: `mark_workspace_read` per channel, and `mark_read` at or before the message it is
given, so a client cannot leave it on something the list does not show either. DMs read through
`POST /channels/{id}/read`, and the conversation route is gone; a conversation's list asks the
first page to mark it read as a channel's does. The notification for a reply sent to the channel
records `in_channel`, and both channel reads clear it, so the digest finds it read. For a
position written before this, or a message deleted since, unread counts carry `last_read_at` and
the first page `last_read_before_at`: once a page reaches back past that time without finding
the boundary, the list stops and draws no line. `/location` looks a message up with `find_root`,
so a deleted root that still has replies opens its thread, and keyword matching gets the
forwarder's own words, as mentions do. *Mark all read* also refetches the unread counts, which
had kept the old position for up to half a minute.

### [CS-093] A failed send stays where you sent it ✅ shipped
CS-081 kept a failed message in the channel and the thread panel, and three paths still lost
one. A reply from a *Threads* card failed into the thread panel's cache, which the card does not
read. When the request timed out after the server had saved the message, `message.new` dropped
the unsent copy first and `onError` kept it afterwards, and `withUnsent`, deduping by id, could
not see that the delivered row (keyed by the server's id) was the same message. The unsent
store lived in memory, so a reload lost everything in it; and scheduling sent only the files
that had finished uploading, then cleared the tray.

The card renders `useUnsent(root.id)` under its replies with *Retry* and *Discard*. It still sends
with `mutate`, not the `mutateAsync` the ticket named: the failed reply carries its text and
*Discard* puts it back in the draft, as in the panel, so handing it to the composer as well would
show it twice. A session-wide set of delivered client ids, filled by `message.new` and by
`onSuccess` through `deliver`, makes `keep` ignore a message the server already has, and
`withUnsent` dedupes on `client_message_id` too. The store saves to `chat_unsent` in local
storage on every change and reads it back at start; `useUnsent` shows only the current user's,
so another account on the same browser does not see them. Attachments are already uploaded,
so a kept message refers to its files by id and no file content is stored. `handleSchedule` refuses a
tray with a failed file in the words sending uses.

### [CS-094] Addresses, workspace photos and hooks that keep working ✅ shipped
Three leftovers that each broke something a while after it worked. Nothing but SSO lowercased an
address, so an invite to `Ana@Firma.com` stored that spelling, the provider's `ana@firma.com`
missed it in `find_by_email`, and SSO refused or made a second account; a password sign-in in
another case failed too. A workspace photo went through `/files/upload` as an ordinary file,
readable only because `is_avatar` searched every profile for its key, and the workspace's file
retention deleted it with the old attachments, the problem CS-087 fixed for account photos. And
a hook switched off because its creator left or lost the channel could only be deleted and made
again.

Migration 81 refuses to run while two accounts differ only in case, naming both, and otherwise
lowercases users and invites and adds a `CHECK (email = lower(email))` to each; the existing
unique index then covers case. `AuthRepo` lowercases in SQL on every insert and on
`find_by_email`, and invites are stored lowercase. Workspace photos go through
`POST /workspaces/{id}/profile/avatar`, the account photo's `avatar::store` with the workspace
in the key, `avatars/{user}/{workspace}/`; the download checks membership of that workspace,
`forget` only deletes a key of the workspace it is asked about, so clearing a workspace photo
never removes the account's, and `avatar::migrate` copies existing workspace photos on the
worker's start. Erasing an account deletes its workspace photos too. `is_avatar` is gone, so a
file uploaded the old way is a draft again. `PATCH /hooks/{id}` with `is_active: true` checks
what creating that hook would (`require_hook_maker`: the type's role and every bound channel),
refuses a slash command whose name was taken meanwhile, and hands the hook to the caller, which
is what keeps it on the next time `hooks_to_fire` checks its creator.

### [CS-095] Push to the right device, from the right server ✅ shipped
Push is held while a session is active, and three things made that rule miss. The gateway's
heartbeat renewed `active:user:{user}` whatever the socket did, and a dead socket is noticed only
after the 90-second pong timeout, so a laptop shut mid-use held push to the phone for two to two
and a half minutes. `unread_count_everywhere` counted every unread notification, the app only
those of workspaces the person is in. And the event bus did not say which server a frame came
from, so a desktop notification from a second instance located its message through the current
one's API, got 404 and called it deleted.

The heartbeat now goes through `renew_activity`, which renews the connection's entry when a frame
arrived within the last interval and removes it when none did — removing, not just leaving it to
expire, is what brings a mention to the phone within about a heartbeat. `lib/activity.ts` goes
idle on `pagehide` as it already did on hiding. The badge query joins `workspace_members`.
`globalEventBus.emit` takes the instance URL the `WebSocketClient` was built for, handlers get
it as a second argument, and `useOpenMessage` takes an optional instance to locate through;
navigating to the message's workspace then switches the active instance as any workspace
switch does.

### [CS-096] The survey's small things ✅ shipped
Eight smaller findings from the survey. Six held and are fixed; two were dropped.

Fixed:
- **Guest huddles.** A DM huddle checked only that the partner was in the workspace. It now goes
  through `require_reachable`, the rule a conversation is opened by, and so does inviting into
  a huddle, which skips an invitee the caller could not message.
- **Channel names in search.** `/search` returns `SearchHit` (a `Message` plus `channel_name`)
  and `FileSearchHit` gained the same field, both from one `channel_names` lookup over the page's
  channels. `SearchPanel` labels a hit by it rather than by the sidebar's channels, so a public
  channel the person is not in is named too.
- **A deleted thread with no replies.** `find_root` already hid a deleted root with no replies;
  `thread_root` turned that into 422, the answer for a parent in another channel. It is 404 now,
  and `ThreadPanel` swaps the composer for *This thread was deleted* when `isClosedThread`
  holds: the root is deleted, nothing is loading and no delivered reply is left.
- **Export ranges.** `message_edits`, `reactions`, `polls` and `poll_votes` take the message's
  `created_at` against `since`/`until`, as messages do.
- **Invite email.** `email_sent: bool` became `email_status`: `sent`, `not_configured` or
  `failed`, which is now possible only when queueing to the outbox fails. *Members* words each
  through `inviteMessage`.
- **Threads view composer.** The card had neither scheduling nor *Also send to*. It now passes
  `scheduleTarget`, the workspace and instance to `MessageInput`, and sends `alsoToChannel` from
  a checkbox worded as the panel's.

Dropped:
- **Huddle Activity under a pause.** Writing the *Incoming huddle* row before the pause check
  is the rule every notification follows. A mention or a DM under DND also lands in Activity
  and only the push is held, and manual QA 8.5 asks for exactly that for a ring.
- **Presence on joining.** The claim does not hold. Opening the new workspace sends
  `subscribe`, which adds it to the connection's presence set, writes presence there and
  publishes `presence.changed` to its members at once.

### [CS-097] A workspace icon everyone sees, and that stays ✅ shipped
CS-094 gave workspace photos their own storage place and left the workspace icon for later.
*General* uploaded the icon through `/files/upload` and saved its URL. The file had no message,
so `require_file_access` treated it as the uploader's draft: every other member got 403 and a
broken image. As a row in `files` it was also within reach of retention, and a replaced or
removed icon stayed in storage.

`POST /workspaces/{id}/icon` (admin) stores the icon under `icons/{workspace}/` through
`workspace::icon`. It reuses the photo pipeline, `avatar::shrink` and `avatar::image_field`,
which were split out of `store` and the profile routes. The download route serves an `icons/`
key to members of its workspace. The upload, a `PATCH` that changes `icon_url` and a hard delete
call `icon::forget`, which deletes only a key of the same workspace. `icon::migrate` runs after
the photo migration on the worker's start and copies each `/api/files/download/` icon outside
`icons/` into place.

## Wave 28 — What 4.0.0 needs

### [CS-098] A project people can find ✅ shipped
Feedback on 3.7.0 found the repository had no description, website or topics, so it did not
come up in GitHub search, and that the README's image example named 3.3.0 while 3.7.0 was out.
There was also no page that showed what the app looks like.

`site/index.html` is the homepage: one static page with no build step and no third-party
requests, in the app's own palette with light and dark themes. It covers what the project is,
the features, the stack, how to run it, and what differs from Slack. `pages.yml` publishes it
to GitHub Pages from `main` and, on a pull request, checks that every local link resolves. The
app is drawn in HTML rather than shown in screenshots. A screenshot shows a real instance's data,
needs a copy per theme and goes stale with every change to the interface; the drawing follows
the theme and stays sharp at any size. The README, the RUNBOOK and `.env.example` now point at the
latest release instead of naming one. CONTRIBUTING says so, and that a change a team would notice
updates the README's *Features* and the homepage. The repository's description, website and
topics are set in its settings, not in the code.

### [CS-099] Away, on purpose and when idle ✅ shipped
Presence came only from whether the gateway held a socket, so a laptop left open overnight
showed green, and nobody could say *I am away* without signing out. The client already
reported input (`activity` frames, CS-095), but only push routing used it.

`users.manual_away` (migration 82) is set with `PUT /users/me/presence` or the new `/away`,
which replaces the comment in `commands/builtin.rs` that had ruled `/away` out. The gateway
now works out `online` or `away` for a connected person. They are away if they chose it, or if
`last_active:user:{id}` is more than eight minutes old. The client reports idle two minutes
after the last input, so that is about ten minutes without input. Each connection does this
on connect, on every `activity` frame and on every heartbeat. `SET presence:status:{id} … GET`
means a change is announced once, however many connections or nodes see it. The API's
`presence.recompute` is handled only by a node holding one of the person's sockets.
`presence.batch` carries each member's status. The frames' status is now a typed `Presence`
enum, so the generated TypeScript says `"online" | "away" | "offline"`. `presence.changed` now
also reaches the person themselves, which is how their other devices and their own dot
follow. The frontend already drew an amber dot for `away`; *You* gained *Set yourself
away / active* and the dot on your own avatar.

The ticket named the states `active`/`away`/`offline`. The wire already said `online`, and
the client already handled `online`, so the names stayed as they were.
