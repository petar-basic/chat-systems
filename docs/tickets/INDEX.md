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
shipped too. Waves 18 and 19 are shipped too. Wave 20 is shipped too: CS-059 in 3.2.0,
and CS-037 in 4.0.0. Waves 21–23,
chosen in the 2026-10-23 review, shipped in 3.3.0. Wave 24, from the 2026-10-26 review, shipped
in 3.4.0. Wave 25, which moves the profile and notification settings to each workspace, shipped
in 3.5.0. Wave 26, which fixes what the 2026-10-27 product survey found, shipped in 3.6.0.
Wave 27, which fixes what the 2026-10-28 survey of 3.6.0 found, shipped in 3.7.0. Wave 28, the
feedback on 3.7.0 and CS-037, shipped in 4.0.0.
After it, releases are bug fixes only for a while. Wave 29, a desktop app, shipped in 4.1.0. Wave 30,
themes per workspace and the preferences a team from Slack looks for, shipped in 4.2.0. Wave 31, passkeys
and emailed sign-in codes, shipped in 4.3.0. Wave 32, analytics for admins, shipped in 4.4.0. Wave 32a,
which fixes what the 2026-11-10 survey of 4.6.0 found, comes next, then Wave 33 (mobile apps, 5.0.0).

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
| CS-037 ✅ → Wave 20 | SFU for large huddles | backend · frontend |

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

### Wave 20 — Dialogs and large calls ✅ done

The dialogs first: they are the part of the app every user sees each day, the work is
frontend only, and it lands in a few days. The SFU follows; it is the one ticket that adds a
service to the stack, so it goes last in the wave and into the release with its own upgrade
note.

| # | Ticket | Area |
|---|---|---|
| CS-059 ✅ | Dialogs that look finished: sizes, fields, confirmations, profile and preferences | frontend |
| CS-037 ✅ | SFU (LiveKit) for huddles above the mesh's ceiling, with promotion mid-call | backend · frontend · ops |

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
| CS-086 ✅ | Getting people in | frontend · backend |
| CS-087 ✅ | A photo that shows everywhere | backend · frontend |
| CS-088 ✅ | A Slack import that keeps what it found | backend |

### Wave 27 — What the second survey found

A second survey on 2026-10-28 read 3.6.0 against the first and listed twenty-four open
findings and nine smaller ones. Twenty-two held when checked in the code; a demoted admin's
hooks (SEC-3) held only in part — the executor already stops at channels they cannot read —
and a doubled unread count after a laptop wakes (RT-1) corrects itself on the next event, so
it was dropped, as was the release build's `PREVIOUS_IMAGE`, which is the previous release by
design. Two findings are regressions of Wave 26: a re-import archiving a merged channel
(CS-088) and *Mark all read* loading a whole channel (CS-082). What lets somebody keep access
or data they should have lost comes first.

| # | Ticket | Area |
|---|---|---|
| CS-089 ✅ | Gone means gone: SSO, erase, demotion, workflow links | backend |
| CS-090 ✅ | A Slack import that respects what is already here | backend |
| CS-091 ✅ | An archived channel is read-only all the way | backend · frontend |
| CS-092 ✅ | Read state that matches what you read | backend · frontend |
| CS-093 ✅ | A failed send stays where you sent it | frontend |
| CS-094 ✅ | Addresses, workspace photos and hooks that keep working | backend · frontend |
| CS-095 ✅ | Push to the right device, from the right server | realtime · backend · frontend |
| CS-096 ✅ | The survey's small things | backend · frontend · realtime |
| CS-097 ✅ | A workspace icon everyone sees, and that stays | backend · frontend |

### Wave 28 — What 4.0.0 needs

Feedback on 3.7.0 on 2026-10-29 named five gaps: a GitHub page nobody can find the project
from and a README naming an old release, presence with no *away*, an integration story with
no bots, events or app directory and with slash commands that must answer in three seconds,
and one secret that signs sessions, encrypts connected apps' tokens and two-factor secrets,
and is shared with the gateway. With the SFU held since 3.2.0 they make 4.0.0. Key separation
comes before EdDSA, because removing `JWT_SECRET` needs nothing else to depend on it; bots
come before events and events before the directory, which lists both.

| # | Ticket | Area |
|---|---|---|
| CS-098 ✅ | A project people can find: homepage, About, topics, README version | docs · ops |
| CS-099 ✅ | Away, on purpose and when idle | backend · realtime · frontend |
| CS-100 ✅ | Stored secrets get a key of their own | backend · ops |
| CS-101 ✅ | Sessions signed with EdDSA, with keys that rotate | backend · realtime · ops |
| CS-102 ✅ | Slash commands that may take their time | backend · frontend |
| CS-103 ✅ | Apps with bot users and a Web API | backend · frontend · docs |
| CS-104 ✅ | Events for apps, over HTTP or a socket | backend · realtime · docs |
| CS-105 ✅ | An app directory | frontend · backend · docs |
| CS-037 ✅ | SFU for large huddles, promoted mid-call | backend · frontend · ops |

### Wave 29 — A desktop app

Decided on 2026-11-02 ([plan](../native-apps-plan.md)): one app we publish, which opens any
server, built on Tauri 2 around each server's own web app. The spike comes first because a
desktop app that cannot hold a huddle on macOS changes the choice of framework; the shell
comes next because everything else lives inside it.

| # | Ticket | Area |
|---|---|---|
| CS-106 ✅ | Does a Tauri shell hold a huddle? | desktop |
| CS-107 ✅ | A desktop app that opens any server | desktop |
| CS-108 ✅ | One rail for servers and workspaces, notifications, badge and tray | desktop · frontend |
| CS-109 ✅ | SSO that finishes in the app | desktop · backend |
| CS-110 ✅ | Desktop builds with every release | desktop · ops · docs |

### Wave 30 — Make it yours

Raised on 2026-11-04 after 4.1.1, from a walk through the app beside Slack's *Preferences*.
Themes come first because they are what was asked for; CS-113 comes before CS-114 and
CS-115 because it adds the store their settings are kept in, and so before CS-117 too.
CS-116 stands alone. CS-117a is a bug found on 2026-11-05: leaving a workspace, or being
removed from it, emptied every DM with that person, and coming back started new ones.

| # | Ticket | Area |
|---|---|---|
| CS-111 ✅ | A theme for each workspace | frontend · backend |
| CS-112 ✅ | Custom themes, shared themes, and a desktop window in the workspace's colours | frontend · desktop |
| CS-113 ✅ | Preferences that follow you, and how messages look | backend · frontend |
| CS-114 ✅ | Enter to send or for a new line, and where a channel opens | frontend |
| CS-115 ✅ | Time zones and local time | backend · frontend |
| CS-116 ✅ | Title and pronouns on the profile | backend · frontend |
| CS-117 ✅ | Text size and typeface | frontend · backend |
| CS-117a ✅ | Conversations outlive a departure | backend · frontend |

### Wave 31 — Signing in

Planned on 2026-11-04, shipped in 4.3.0. Ways to sign in without a password, done before
the phone apps so those start with them. Passkeys come first because the emailed code takes
the same second step after it.

| # | Ticket | Area |
|---|---|---|
| CS-118 ✅ | Passkeys | backend · frontend · desktop |
| CS-119 ✅ | Sign in with a code sent by email | backend · frontend |

### Wave 32 — What an admin can see

Planned on 2026-11-04, shipped in 4.4.0. The numbers an admin is asked for: who uses the
workspace, which channels carry the work, and which ones nobody posts in any more.

| # | Ticket | Area |
|---|---|---|
| CS-120 ✅ | Workspace analytics | backend · frontend |
| CS-121 ✅ | Channel analytics and quiet channels | backend · frontend |

### Wave 32a — What the third survey found

A third survey on 2026-11-10 read 4.6.0 (`1162971`) against the one of 3.6.0 and listed
twenty-seven open findings and a dozen smaller ones. Every finding held when checked in the
code, though four rest on reasoning the code cannot settle alone: the new message in the
middle of the history (MSG-14) has not been seen in the app yet, how often a reader turns
inactive (NOT-8) and the guess rate on emailed codes (SEC-12) are worked out from the
constants, and a late click on a desktop banner (DSK-2) depends on how each OS delivers it.
One smaller finding was dropped: the *Incoming huddle* row under a pause is the rule every
notification follows, decided in Wave 27. A doubled unread count after a replay (RT-1), dropped
in Wave 27 because it corrects itself, is back in: coming back to a workspace replays
everything since you left, so it happens on every switch. The numbers carry a letter so the
wave lands before Wave 33 without renumbering it. What lets somebody into a conversation, a
role or a channel they should not have comes first.

| # | Ticket | Area |
|---|---|---|
| CS-121a ✅ | A conversation stays between the people in it, and an archived channel stays read-only | backend |
| CS-121b ✅ | Ownership is handed over, never taken | backend · frontend |
| CS-121c ✅ | What someone built stops when they lose the right to it | backend · frontend |
| CS-121d ✅ | A Slack import that respects what changed since the last run | backend · frontend |
| CS-121e ✅ | A new message lands at the bottom | frontend |
| CS-121f ✅ | Read when you read it, and a *New* line that knows what you read | frontend |
| CS-121g ✅ | Threads that know where they are | frontend · backend |
| CS-121h ✅ | Slash commands in every message box | frontend · backend |
| CS-121i ✅ | Emailed codes with a ceiling, and instance roles that apply at once | backend · frontend · realtime |
| CS-121j ✅ | Erase and export that cover everything about a person | backend · frontend |
| CS-121k ✅ | The desktop app after signing out and after a reload | frontend · desktop |
| CS-121l ✅ | The third survey's small things | realtime · frontend · backend |
| CS-121m ✅ | Notifications in the reader's language | backend · frontend |

### Wave 33 — Mobile apps

Planned on 2026-11-04, to go out as 5.0.0, the major version because CS-122 moves every
route under `/api/v1`. Phone apps on Tauri 2, like the desktop app, if the spike (CS-123)
says a huddle and a push work there; the push relay waits on who runs it.

| # | Ticket | Area |
|---|---|---|
| CS-122 | `/api/v1`, and a client and a server that know each other's version | backend · frontend · ops · docs |
| CS-123 | Does a Tauri phone app hold a huddle and receive a push? | mobile |
| CS-124 | Signing in from a client the server does not serve | backend · frontend · mobile |
| CS-125 | Phone apps for iOS and Android | mobile · frontend · ops |
| CS-126 | A push relay for phones | backend · mobile · ops |

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
| huddle join authorization | CS-041 ✅ → CS-077 ✅ → CS-037 ✅ |
| notification consumer and `is_dnd_active` | CS-046 ✅ → CS-060 ✅ → CS-061 ✅ → CS-062 ✅ |
| `NotificationStream` and `wsQuerySync` sound/desktop | CS-060 ✅ → CS-061 ✅ |
| search visibility (`searchable_channels()`) | CS-048 ✅ → CS-063 ✅ → CS-064 ✅ |
| search panel tabs and filter bar | CS-058 ✅ → CS-063 ✅ → CS-064 ✅ |
| `chat-worker` supervised loops | CS-064 ✅ → CS-065 ✅ → CS-067 ✅ |
| workspace `settings` JSON and the *General* section | CS-070 ✅ → CS-075 ✅ |
| *Channel settings* / *Channel details* | CS-070 ✅ → CS-074 ✅ → CS-075 ✅ |
| *Integrations* panel and hook permissions | CS-070 ✅ → CS-074 ✅ |
| `workspace_members` profile and notification columns | CS-076 ✅ |
| invite tokens and *Members* invite list | CS-077 ✅ → CS-086 ✅ |
| message route resolution | CS-079 ✅ → CS-080 ✅ → CS-082 ✅ |
| Slack import channel mapping | CS-077 ✅ → CS-088 ✅ |
| archived channels | CS-085 ✅ → CS-088 ✅ |
| `auth/oidc_routes.rs` and account lookup by email | CS-089 → CS-094 |
| Slack import channel placement and archiving | CS-090 |
| channel writers' archive checks in `messaging/routes.rs` | CS-091 → CS-092 |
| `notifications/repo.rs` read and badge queries | CS-092 → CS-095 |
| `stores/unsent.ts` and `MessageInput.tsx` | CS-093 → CS-096 |
| `profile/avatar.rs` and the download route's photo keys | CS-094 → CS-097 |
| presence sets and `presence.changed` | CS-095 → CS-099 |
| `sealed.rs`, `auth/totp.rs` and the token signing in `auth/service/tokens.rs` | CS-100 → CS-101 |
| `commands/routes.rs`, hooks and the *Integrations* section | CS-102 → CS-103 → CS-104 → CS-105 |
| realtime gateway auth and connection handling | CS-101 ✅ → CS-104 ✅ → CS-037 ✅ |
| sidebar tokens and *Preferences → Appearance* | CS-111 → CS-112 |
| `workspace_members` profile columns | CS-111 → CS-116 |
| `users.preferences` and the *Preferences* tabs | CS-113 → CS-114 → CS-115 → CS-117 |
| sign-in page, `auth/routes.rs` login and the second step | CS-118 → CS-119 → CS-124 |
| *Preferences → Security* | CS-118 |
| worker rollups and *Workspace settings → Analytics* | CS-120 → CS-121 |
| route prefixes, nginx, Caddy and the E2E helpers | CS-122 |
| `/auth/oidc/exchange` and the deep link | CS-109 ✅ → CS-118 → CS-124 → CS-125 |
| the Tauri project's mobile targets | CS-123 → CS-125 → CS-126 |
| `MessageInput.tsx` key handling | CS-114 |
| `membership::detach`, `remove_member_in` and the conversation list | CS-117a ✅ → CS-121c |
| `membership::add_to_channel` and the channel member routes in `workspace/routes.rs` | CS-121a |
| invite, role and member-removal routes in `workspace/routes.rs`, `accept_invite` | CS-121b |
| `commands/routes.rs` `invoke` and `commands/builtin.rs` | CS-121a → CS-121c → CS-121h |
| `hooks/repo.rs` `enable`, `bot_apps` and `stand_down` | CS-121c |
| Slack import reuse branch and `analytics/repo.rs` | CS-121d |
| `lib/messageCache.ts`, `useMessages.ts` and `message.new` in `wsQuerySync.ts` | CS-121e → CS-121f |
| `readOnScreen`, `MessageList` read boundary and unread hydration | CS-121f |
| `ThreadPanel.tsx` and `ThreadsView.tsx` | CS-121f → CS-121g → CS-121h |
| `useComposer`, `ConversationView` and `MessageInput.tsx` | CS-121h → CS-121l |
| `auth/email_code_routes.rs`, instance-admin checks and `update_instance_role` | CS-121i |
| `export/routes.rs` erase and `export/job.rs` | CS-121j |
| `lib/notifications.ts` and the desktop report | CS-121k → CS-121m |
| desktop `report_theme` | CS-112 |
