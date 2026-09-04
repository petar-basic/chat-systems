# Ticket index

One file per ticket, numbered in **execution order**. The number *is* the schedule —
when two tickets touch the same file, the lower number lands first and the higher one
is written against the result.

A ticket's file is deleted when it ships; the record of what it did lives in the git
history and in the docs it changed. A bug found mid-wave gets a suffixed number
(`CS-005a`) so it lands where it belongs in the order without renumbering everything
below it.

**Every wave through 12 is done, bar the SFU in Wave 9.** Wave 12 was the un-ticketed
architecture review of 2026-09 (written up in [history.md](../history.md)). The E2E suite is
green — desktop and a phone viewport — and the `e2e` job blocks merges. Next ticket:
[CS-047](CS-047-prove-it-at-import-scale.md), which measures what the last wave changed.

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
| [CS-037](CS-037-huddle-sfu.md) | SFU for large huddles | backend · frontend |

### Wave 11 — Guest containment, operational readiness and mobile ✅ done

CS-041 guest directory · CS-042 guest DM scope · CS-043 announcement channels · CS-044
revocation · CS-045 mention emails · CS-046 huddle consumers · CS-038 the responsive PWA.
See [ROADMAP.md](../ROADMAP.md#wave-11--guest-containment-operational-readiness-and-mobile--shipped).

### Still open

| # | Ticket | Area | Why it is not done |
|---|---|---|---|
| [CS-037](CS-037-huddle-sfu.md) | SFU for large huddles | backend · frontend | LiveKit is infrastructure of its own; the mesh is right for everything under six people |
| [CS-047](CS-047-prove-it-at-import-scale.md) | Prove it at import scale | backend · ops | Deferred out of Wave 11 on purpose: it measures what Wave 11 changed, so it runs after it |

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
