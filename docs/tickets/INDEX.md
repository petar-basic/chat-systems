# Ticket index

One file per ticket, numbered in **execution order**. The number *is* the schedule —
when two tickets touch the same file, the lower number lands first and the higher one
is written against the result.

A ticket's file is deleted when it ships; the record of what it did lives in the git
history and in the docs it changed. A bug found mid-wave gets a suffixed number
(`CS-005a`) so it lands where it belongs in the order without renumbering everything
below it.

**Waves 0, 1 and 2 are done.** The E2E suite is green and the `e2e` job blocks merges.
Next ticket: [CS-016](CS-016-uniform-login-failure.md).

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

### Wave 3 — Authentication hardening

| # | Ticket | Area |
|---|---|---|
| [CS-016](CS-016-uniform-login-failure.md) | Uniform login failure and constant-time path | backend/api |
| [CS-017](CS-017-auth-transport-defaults.md) | Password policy and mail transport defaults | backend/api |

### Wave 4 — Governance

| # | Ticket | Area |
|---|---|---|
| [CS-018](CS-018-audit-log-coverage.md) | Audit log coverage | backend/api |
| [CS-019](CS-019-scope-outgoing-webhooks.md) | Scope outgoing webhooks per channel | backend/api · frontend |
| [CS-020](CS-020-file-moderation.md) | File moderation and attachment lifecycle | backend/api |

### Wave 5 — Correctness

| # | Ticket | Area |
|---|---|---|
| [CS-021](CS-021-scheduled-reauthorize.md) | Re-authorize scheduled messages at delivery | backend/api |
| [CS-022](CS-022-scoped-idempotency-id.md) | Scope client-supplied message id to its conversation | backend/api |
| [CS-023](CS-023-validation-gaps.md) | Close remaining input validation gaps | backend/api |

### Wave 6 — Performance

| # | Ticket | Area |
|---|---|---|
| [CS-024](CS-024-static-message-renderer.md) | Replace the per-message editor with a static renderer | frontend |
| [CS-025](CS-025-virtualize-message-list.md) | Virtualize the message list | frontend |
| [CS-026](CS-026-unread-counts.md) | Unread counts without per-channel subqueries | backend/api · frontend |
| [CS-027](CS-027-presence-without-scan.md) | Presence without a Redis keyspace scan | realtime |

### Wave 7 — Reliability

| # | Ticket | Area |
|---|---|---|
| [CS-028](CS-028-durable-delivery.md) | Durable realtime delivery (Redis Streams) | realtime · frontend |

### Wave 8 — Compliance

| # | Ticket | Area |
|---|---|---|
| [CS-029](CS-029-message-edit-history.md) | Message edit history | backend/api · frontend |
| [CS-030](CS-030-retention-policies.md) | Retention policies and token cleanup | backend |
| [CS-031](CS-031-data-export.md) | Workspace and user data export | backend |
| [CS-032](CS-032-sso-and-2fa.md) | SSO (OIDC) and 2FA | backend/api · frontend |
| [CS-033](CS-033-scim-deprovisioning.md) | SCIM deprovisioning | backend/api |

### Wave 9 — Product parity

| # | Ticket | Area |
|---|---|---|
| [CS-034](CS-034-search-language.md) | Configurable full-text search language | backend |
| [CS-035](CS-035-web-push.md) | Web Push for closed-app delivery | backend/api · frontend |
| [CS-036](CS-036-slack-import-export.md) | Slack import / export CLI | backend |
| [CS-037](CS-037-huddle-sfu.md) | SFU for large huddles | backend · frontend |
| [CS-038](CS-038-mobile-client.md) | Mobile client | frontend |
| [CS-039](CS-039-remaining-parity.md) | Custom emoji, user groups, bots and slash commands | backend/api · frontend |

## Conflict map

Tickets that touch the same files, and the order that avoids rework:

| File | Tickets, in order |
|---|---|
| `backend/api/src/lib.rs` router wiring | CS-012 → CS-030 |
| `backend/api/src/authz.rs` | CS-009 → CS-010 → CS-020 |
| `backend/api/src/sessions.rs` | CS-008 → CS-033 |
| `backend/api/src/auth/service.rs` | CS-008 → CS-016 → CS-017 → CS-032 |
| realtime event consumer | CS-007 → CS-014 → CS-027 → CS-028 |
| `frontend/src/features/messaging/` | CS-024 → CS-025 → CS-029 |
| `messages` table schema | CS-029 → CS-030 → CS-034 |
