# Ticket index

One file per ticket, numbered in **execution order**. The number *is* the schedule —
when two tickets touch the same file, the lower number lands first and the higher one
is written against the result.

A ticket's file is deleted when it ships; the record of what it did lives in the git
history and in the docs it changed. A bug found mid-wave gets a suffixed number
(`CS-005a`) so it lands where it belongs in the order without renumbering everything
below it.

**Every wave through 17 is done, bar the SFU in Wave 9.** Wave 12 was the un-ticketed
architecture review of 2026-09 (written up in [history.md](../history.md)). The E2E suite is
green — desktop and a phone viewport — and the `e2e` job blocks merges. CS-047 measured the
system at 500,000 messages and closed with Wave 12; what its numbers justified, CS-048, is
Wave 13 and is shipped too. Wave 14 closes the daily-use gaps a team arriving from Slack
notices first, Waves 15 and 16 (connected-app previews) and Wave 17 (files and images) are
shipped too. Waves 18 and 19 are shipped too. In Wave 20, CS-059 shipped in 3.2.0;
[CS-037](CS-037-huddle-sfu.md) is held for the next major release, 4.0.0. Waves 21–23,
chosen in the 2026-10-23 review, shipped in 3.3.0. Wave 24, from the 2026-10-26 review, shipped
in 3.4.0. Wave 25, which moves the profile and notification settings to each workspace, shipped
in 3.5.0. Wave 26 fixes what the 2026-10-27 product survey found, for 3.6.0.

Waves are groupings, not gates: you can start the next ticket in a wave before the
previous one merges, but you should not start a wave before the wave above it is done,
because later waves assume the primitives the earlier ones introduce.

## Why this order

1. **Wave 0 first, always.** ✅ Shipped. It installed the safety net (E2E in CI) and
   the three structures half the remaining tickets depend on: `authz` as the single
   home for permission predicates, `sessions::revoke` as the single way a session
   ends, and `chat-worker` as a process separate from the API. Every access-control
   fix in Wave 1 lands inside those structures; doing Wave 1 first meant writing each
   fix twice.
2. **Access control before abuse limits.** A rate limit on an endpoint that leaks
   data is a slower leak. Close the holes, then bound the traffic.
3. **Governance after access control.** An audit log is only meaningful once the
   thing it audits is actually enforced.
4. **Performance after correctness.** `CS-024` (drop the per-message editor) must
   precede `CS-025` (virtualization) — virtualizing a list of editor instances
   optimizes the wrong layer.
5. **Durable delivery after the worker split.** `CS-028` reworks the same pub/sub
   layer `CS-004` moves. Doing it first means doing it twice.
6. **Compliance before parity features.** Retention, export and SSO are what a
   security review asks for; custom emoji is not.

## Conventions every ticket assumes

Read [CONTRIBUTING.md](../CONTRIBUTING.md#coding-standards) once; it is not repeated
per ticket.

- **Zero comments.** A comment that feels necessary is a signal to rename or split.
- **Backend layering.** `routes` parse + authorize + delegate · `service` for
  multi-step logic · **all SQL in `repo`** · `publisher`/`consumer`/`executor` for
  Redis work. A feature never calls another feature's repo.
- **No `unwrap`/`expect`/`panic` outside startup config validation.**
- **Parameterized SQL only.** Every handler returns `AppResult<Json<T>>` and converts
  `None` to `AppError::NotFound`.
- **Frontend:** strict TS, feature-modular, logic in hooks, TanStack Query for server
  state and Zustand for UI state, query keys only via the `QUERY_KEYS` factory.
- **Every ticket ships tests.** Backend changes get `#[sqlx::test]` coverage in
  `http_tests` including the authorization matrix (member-ok / non-member-forbidden /
  no-token / not-found). Security tickets get a regression test that fails on `main`.

## Tickets

### Wave 0 — Safety net and structural groundwork ✅ done

| # | Ticket | Landed as |
|---|---|---|
| CS-001 | Run the Playwright suite in CI | `e2e` job in [`ci.yml`](../../.github/workflows/ci.yml) |
| CS-002 | Central authorization module | [`backend/api/src/authz.rs`](../../backend/api/src/authz.rs) |
| CS-003 | Session revocation and live-disconnect primitive | [`backend/api/src/sessions.rs`](../../backend/api/src/sessions.rs) |
| CS-004 | Split background workers into `chat-worker` | [`backend/api/src/bin/chat-worker.rs`](../../backend/api/src/bin/chat-worker.rs) |
| CS-005 | Decide on compile-time-checked sqlx queries | decision in [CONTRIBUTING.md](../CONTRIBUTING.md#backend-rust) |

| CS-005a | Role-gated UI disappears when the workspace role is not populated | [`useCurrentWorkspaceRole.ts`](../../frontend/src/features/workspace/hooks/useCurrentWorkspaceRole.ts) |

### Wave 1 — Access control ✅ done

| # | Ticket | Landed as |
|---|---|---|
| CS-006 | Invite lifecycle: expiry, max uses, email binding | `InviteLifetime` + migration `…16` |
| CS-007 | Propagate membership removal to live sockets | `channel.member_removed` / `workspace.member_removed` |
| CS-008 | Revoke access tokens on password reset/change | `sessions::revoke` from the auth routes |
| CS-009 | Attachment access control for conversations | [`files/service.rs`](../../backend/api/src/files/service.rs) + migration `…17` |
| CS-010 | Guest scoping in message search | `requester_is_guest` in `MessageSearch` |

### Wave 2 — Abuse and resource limits ✅ done

| # | Ticket | Landed as |
|---|---|---|
| CS-011 | Streaming upload with an enforced size cap | `UploadSink` in [`files/storage.rs`](../../backend/api/src/files/storage.rs) |
| CS-012 | Write rate limit on every mutating router | `crate::protected` + budget classes |
| CS-013 | Fail-closed rate limiting on auth paths | `LimiterFailure` |
| CS-014 | WebSocket inbound message rate limiting | `InboundState` in `ws_handler.rs` |
| CS-015 | Per-IP limit for incoming webhooks | [`net.rs`](../../backend/api/src/net.rs) |

### Wave 3 — Authentication hardening ✅ done

| # | Ticket | Landed as |
|---|---|---|
| CS-016 | Uniform login failure and constant-time path | one answer for every failure + a dummy verify |
| CS-017 | Mail transport defaults (password policy dropped by decision) | `SmtpTlsMode` |

### Wave 4 — Governance ✅ done

| # | Ticket | Landed as |
|---|---|---|
| CS-018 | Audit log coverage | [`audit.rs`](../../backend/api/src/audit.rs) + migration `…18` |
| CS-019 | Scope outgoing webhooks per channel | `config.channel_ids` + migration `…19` |
| CS-020 | File moderation and attachment lifecycle | `delete_for_*_message` in [`files/service.rs`](../../backend/api/src/files/service.rs) |

### Wave 5 — Correctness ✅ done

| # | Ticket | Landed as |
|---|---|---|
| CS-021 | Re-authorize scheduled messages at delivery | `DeliveryFailure` in [`scheduled/executor.rs`](../../backend/api/src/scheduled/executor.rs) |
| CS-022 | Scope client-supplied message id to its conversation | `client_message_id` + migration `…20` |
| CS-023 | Close remaining input validation gaps | [`validation.rs`](../../backend/shared/common/src/validation.rs) + `is_unique_violation` |

### Wave 6 — Performance ✅ done

| # | Ticket | Landed as |
|---|---|---|
| CS-024 | Replace the per-message editor with a static renderer | [`messageMarkdown.ts`](../../frontend/src/lib/messageMarkdown.ts) + `MessageContent` |
| CS-025 | Virtualize the message list | [`VirtualMessageList.tsx`](../../frontend/src/features/messaging/VirtualMessageList.tsx) |
| CS-026 | Unread counts without per-channel subqueries | `channel_members` counters + migration `…21` |
| CS-027 | Presence without a Redis keyspace scan | `presence:ws:{id}` sorted set |

### Wave 7 — Reliability ✅ done

| # | Ticket | Landed as |
|---|---|---|
| CS-028 | Durable realtime delivery (Redis Streams) | `stream:ws:{id}` + [`replay.rs`](../../backend/realtime/src/replay.rs) + `StreamGroup` |

### Wave 8 — Compliance ✅ done

CS-029 edit history · CS-030 retention · CS-031 export and erasure · CS-032 SSO and TOTP ·
CS-033 SCIM deprovisioning. See [ROADMAP.md](../ROADMAP.md#wave-8--compliance--shipped).

### Wave 9 — Product parity (partly done)

CS-034 search, CS-035 web push, CS-039 emoji/groups/bots/commands and CS-040 the small Slack
gaps (DM threads, saved items, bookmarks, forwarding, status, reminders) and CS-036 the Slack
importer are shipped — see
[ROADMAP.md](../ROADMAP.md#wave-9--product-parity-partly-shipped). What is left:

| # | Ticket | Area |
|---|---|---|
| CS-037 → Wave 20 | SFU for large huddles | backend · frontend |

### Wave 11 — Guest containment, operational readiness and mobile ✅ done

CS-041 guest directory · CS-042 guest DM scope · CS-043 announcement channels · CS-044
revocation · CS-045 mention emails · CS-046 huddle consumers · CS-038 the responsive PWA.
See [ROADMAP.md](../ROADMAP.md#wave-11--guest-containment-operational-readiness-and-mobile--shipped).

### Wave 13 — What the scale measurement found ✅ done

CS-047 (shipped with Wave 12) loaded 500,000 messages and measured. History, unread
counters, the importer, the backfill and a restore all held up once the restore was fixed;
search did not, and CS-048 fixed it. See
[history.md](../history.md#wave-13--what-the-scale-measurement-found--shipped).

### Wave 14 — What a team from Slack misses on day one ✅ done

Chosen in the 2026-10-20 review: the four things a Slack user reaches for in the first
week and does not find. The activity view a review might add to this list already exists
(the notifications panel's All / Unread / Mentions filters).

| # | Ticket | Area |
|---|---|---|
| CS-049 ✅ | Link previews | backend · frontend |
| CS-050 ✅ | Mark as unread | backend · frontend |
| CS-051 ✅ | All unreads | backend · frontend |
| CS-052 ✅ | Custom sidebar sections | backend · frontend |

### Wave 15 — Links people actually paste at work ✅ done

| # | Ticket | Area |
|---|---|---|
| CS-053 ✅ | Rich previews from connected apps (Jira, GitHub, Google Drive) | backend · frontend · ops |

### Wave 16 — The rest of what a team pastes ✅ done

| # | Ticket | Area |
|---|---|---|
| CS-054 ✅ | Slack-grade app previews (opt-out, pictures, directory, inline video), Figma and Confluence | backend · frontend · ops |

### Wave 17 — Files and images ✅ done

| # | Ticket | Area |
|---|---|---|
| CS-055 ✅ | Files and images the way Slack does them: composer tray, attachments on the message, thumbnails, names, gallery, video, a channel's files | backend · frontend · ops |

### Wave 18 — What a Slack user notices in the first hour ✅ done

| # | Ticket | Area |
|---|---|---|
| CS-056 ✅ | One set of controls: buttons, icon buttons, panels, dialogs | frontend |
| CS-057 ✅ | Later: saved messages and reminders as one to-do list | backend · frontend |

### Wave 19 — Getting around ✅ done

| # | Ticket | Area |
|---|---|---|
| CS-058 ✅ | Getting around like in Slack: keyboard, search filters, Threads | backend · frontend |

### Wave 20 — Dialogs and large calls

The dialogs first: they are the part of the app every user sees each day, the work is
frontend only, and it lands in a few days. The SFU follows; it is the one ticket that adds a
service to the stack, so it goes last in the wave and into the release with its own upgrade
note.

| # | Ticket | Area |
|---|---|---|
| CS-059 ✅ | Dialogs that look finished: sizes, fields, confirmations, profile and preferences | frontend |
| [CS-037](CS-037-huddle-sfu.md) | SFU (LiveKit) for huddles above the mesh's ceiling, with promotion mid-call | backend · frontend · ops |

### Wave 21 — Notifications that fit how a team works

Chosen in the 2026-10-23 review, after 3.2.0. A direct message that never reaches a phone is
the gap a team notices first, and the per-channel level is where it gets fixed. Keywords and
the schedule both build on the level's single server-side decision.

| # | Ticket | Area |
|---|---|---|
| CS-060 ✅ | Notification levels per channel and a default per person; DMs that reach your phone | backend · frontend |
| CS-061 ✅ | Keyword notifications | backend · frontend |
| CS-062 ✅ | Notification schedule | backend · frontend |

### Wave 22 — Finding files

| # | Ticket | Area |
|---|---|---|
| CS-063 ✅ | Search finds files by name | backend · frontend |
| CS-064 ✅ | Search inside files (PDF, Office, text) | backend · frontend · ops |

### Wave 23 — Structured work: polls, lists, workflows

Polls come first because they introduce the message with server-owned buttons that workflow
forms reuse. Lists come before the workflow builder's last part, which writes into them.

| # | Ticket | Area |
|---|---|---|
| CS-065 ✅ | Polls | backend · frontend |
| CS-066 ✅ | Lists: typed tables in a conversation, with board view and item comments | backend · frontend |
| CS-067 ✅ | Workflow builder | backend · frontend · ops |

### Wave 24 — Ready for a company

Chosen in the 2026-10-26 review, after 3.3.0, which walked the app as a company trying it
would. The features are there; what a team notices is the rough edges, an admin's work
spread over nine panels with retention, exports, erasure and SCIM tokens reachable only by
API, and an instance that cannot be set up without editing `.env`. Small fixes come first,
the admin dialog before first run (whose checklist points into it), and the frontend's move
to one structure after the admin dialog has moved its panels. A comparison with Slack's
split between workspace rules and channel settings added the last two: a channel that shows
what posts into it, and limits on who notifies a whole channel and who creates one. They
come after the structure move because they rebuild *Channel settings* as *Channel details*.

| # | Ticket | Area |
|---|---|---|
| CS-068 ✅ | Workflows without drafts | backend · frontend |
| CS-069 ✅ | Small UI fixes and a README that says what ships | frontend · backend · docs |
| CS-070 ✅ | Workspace admin in one place, with the settings that have no screen | frontend · backend |
| CS-071 ✅ | First run and an empty workspace that says what to do | backend · frontend · ops |
| CS-072 ✅ | Fewer round trips on start and on switching channels | frontend · backend |
| CS-073 ✅ | One frontend structure | frontend |
| CS-074 ✅ | A channel shows what posts into it, and its admin can add a webhook | backend · frontend |
| CS-075 ✅ | Who may notify a whole channel, and who may make one | backend · frontend |

### Wave 25 — One person, several workspaces

Raised on 2026-10-27, after 3.4.2: a status set in one workspace showed in every other, because
the profile and the notification settings belong to the account. Slack keeps them per
workspace, and so does this wave; email, password, second factor, time zone and the
device's own settings stay with the account.

| # | Ticket | Area |
|---|---|---|
| CS-076 ✅ | A profile and notification settings of your own in each workspace | backend · frontend |

### Wave 26 — What the survey found

A product survey on 2026-10-27, after 3.5.0, read the code end to end and listed twenty-four
findings and a dozen smaller ones. The access holes come first (an invite link that outlives
its revocation, a private huddle anyone can join), then the places a message opens to the
wrong screen, since every later messaging fix is reached through them. Findings marked
unverified in the survey are checked first in their ticket and dropped if they do not hold.

| # | Ticket | Area |
|---|---|---|
| CS-077 ✅ | Access that holds: huddles, email invites, private channels | backend · realtime · frontend |
| CS-078 ✅ | A suspended or removed person stops acting | backend · frontend |
| CS-079 ✅ | Opening a message lands on it | frontend · backend |
| CS-080 ✅ | Threads that keep up | frontend · backend |
| CS-081 ✅ | Sending never loses work | frontend · backend |
| CS-082 ✅ | Unread, Activity and the badge agree | frontend · backend |
| CS-083 ✅ | Notifications that arrive once, where they should | backend · frontend · realtime |
| CS-084 ✅ | Reconnecting, presence and new versions | frontend · realtime · ops |
| CS-085 ✅ | Archive, export and retention that say what they did | backend · frontend |
| [CS-086](CS-086-getting-people-in.md) | Getting people in | frontend · backend |
| [CS-087](CS-087-account-photos.md) | A photo that shows everywhere | backend · frontend |
| [CS-088](CS-088-slack-import-fidelity.md) | A Slack import that keeps what it found | backend |

### Still open

| # | Ticket | Area | Why it is not done |
|---|---|---|---|
| [CS-037](CS-037-huddle-sfu.md) | SFU for large huddles | backend · frontend · ops | Held for 4.0.0 — it adds a service to the stack, so it ships in a major release |

## Conflict map

Tickets that touch the same files, and the order that avoids rework:

| File | Tickets, in order |
|---|---|
| `backend/api/src/lib.rs` router wiring | CS-012 ✅ → CS-030 ✅ |
| `backend/api/src/authz.rs` | CS-009 ✅ → CS-010 ✅ → CS-020 ✅ → CS-041 ✅ → CS-042 ✅ → CS-043 ✅ |
| `backend/api/src/sessions.rs` | CS-008 ✅ → CS-033 ✅ |
| `backend/api/src/auth/service.rs` | CS-008 ✅ → CS-016 ✅ → CS-017 ✅ → CS-032 ✅ |
| realtime event consumer | CS-007 ✅ → CS-014 ✅ → CS-027 ✅ → CS-028 ✅ → CS-046 ✅ |
| `workspace/repo.rs` member queries | CS-041 ✅ → CS-042 ✅ |
| notification worker path | CS-028 ✅ → CS-035 ✅ → CS-045 ✅ → CS-046 ✅ |
| `frontend/src/features/messaging/` | CS-024 ✅ → CS-025 ✅ → CS-029 ✅ |
| `messages` table schema | CS-029 ✅ → CS-030 ✅ → CS-034 ✅ |
| `channel_members` read position and counters | CS-026 ✅ → CS-050 ✅ → CS-051 ✅ → CS-052 ✅ |
| message metadata and `message.updated` | CS-029 ✅ → CS-049 ✅ → CS-053 ✅ → CS-054 ✅ → CS-055 ✅ → CS-065 ✅ |
| `files` and the download route | CS-009 ✅ → CS-011 ✅ → CS-020 ✅ → CS-055 ✅ |
| `frontend/src/shared/components` dialogs and controls | CS-056 ✅ → CS-059 ✅ |
| huddle join authorization | CS-041 ✅ → CS-037 |
| notification consumer and `is_dnd_active` | CS-046 ✅ → CS-060 ✅ → CS-061 ✅ → CS-062 ✅ |
| `NotificationStream` and `wsQuerySync` sound/desktop | CS-060 ✅ → CS-061 ✅ |
| search visibility (`searchable_channels()`) | CS-048 ✅ → CS-063 ✅ → CS-064 ✅ |
| search panel tabs and filter bar | CS-058 ✅ → CS-063 ✅ → CS-064 ✅ |
| `chat-worker` supervised loops | CS-064 ✅ → CS-065 ✅ → CS-067 ✅ |
| workspace `settings` JSON and the *General* section | CS-070 ✅ → CS-075 ✅ |
| *Channel settings* / *Channel details* | CS-070 ✅ → CS-074 ✅ → CS-075 ✅ |
| *Integrations* panel and hook permissions | CS-070 ✅ → CS-074 ✅ |
| `workspace_members` profile and notification columns | CS-076 ✅ |
| invite tokens and *Members* invite list | CS-077 ✅ → CS-086 |
| message route resolution | CS-079 ✅ → CS-080 ✅ → CS-082 ✅ |
| Slack import channel mapping | CS-077 ✅ → CS-088 |
| archived channels | CS-085 → CS-088 |
