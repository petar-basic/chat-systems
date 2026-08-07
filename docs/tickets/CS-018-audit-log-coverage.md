# CS-018 — Audit log coverage

**Wave:** 4 — Governance
**Area:** backend/api
**Blocked by:** ~~CS-002~~ ✅ shipped as `authz.rs`, ~~CS-004~~ ✅ shipped as `chat-worker`
**Blocks:** CS-031
**Audit finding:** compliance (HIGH for a 70-person org)

## Problem

The `audit_log` table has every column an audit trail needs — `workspace_id`, `user_id`,
`action`, `resource_type`, `resource_id`, `details` JSONB, `ip_address`, `created_at`
([migration 1](../../backend/migrations/20240305000001_initial_schema.sql#L226)) — and an
index on `(workspace_id, created_at DESC)`.

It is written from three call sites:

- [`admin/routes.rs:258`](../../backend/api/src/admin/routes.rs#L258) — user suspend
- [`admin/routes.rs:286`](../../backend/api/src/admin/routes.rs#L286) — user activate
- [`hooks/repo.rs:92`](../../backend/api/src/hooks/repo.rs#L92) — hook secret reveal

Nothing records message deletion, channel deletion or archival, member removal, role
changes, invite creation, workspace deletion, file access or login. After an incident
there is no way to answer "who deleted that channel" or "who was in this private channel
in March". `ip_address` is never populated at all.

There is also no way to read the log — no endpoint, no UI.

## Approach

Auditing must be impossible to forget, which means it cannot be a line each handler
remembers to add.

1. **One typed action enum, one writer.** `backend/api/src/audit.rs`:
   ```rust
   pub enum AuditAction {
       MessageDeleted, MessageEditedByAdmin,
       ChannelCreated, ChannelArchived, ChannelVisibilityChanged,
       ChannelMemberAdded, ChannelMemberRemoved, ChannelRoleChanged,
       WorkspaceMemberRemoved, WorkspaceRoleChanged,
       WorkspaceCreated, WorkspaceDeleted, WorkspaceRestored,
       InviteCreated, InviteRevoked, InviteAccepted,
       HookCreated, HookDeleted, HookRevealed, HookRotated,
       FileDeleted,
       UserSuspended, UserActivated, InstanceRoleChanged,
       AuthLoginSucceeded, AuthLoginFailed, AuthPasswordChanged, AuthPasswordReset,
       SessionRevoked,
   }

   pub async fn record(state: &AppState, entry: AuditEntry) -> ()
   ```
   `record` never fails a request — it logs at `warn` on error, like the existing
   publisher calls. An audit write must not be able to block a user action.
2. **Capture the client IP once.** A small `ClientIp` extractor resolving through the
   trusted-proxy helper from CS-015, so `ip_address` is populated consistently instead of
   per-handler.
3. **Emit from the layer that owns the decision.** Destructive operations already go
   through `authz::require_*`; record immediately after the mutation succeeds, in the
   route handler, with the resource id from the result. Not in the repo — the repo has no
   notion of an actor.
4. **Auth events go through the worker.** `AuthLoginSucceeded` / `AuthLoginFailed` on the
   hot path would add a write per login attempt, including failed ones, which is exactly
   what a brute force generates. Publish them as events and let the worker (CS-004)
   persist them, so a flood degrades the audit lag rather than the login endpoint.
   `AuthLoginFailed` records the attempted address — that is the point of it — so treat
   the log as sensitive data for CS-030 retention.
5. **Make it readable.** `GET /api/workspaces/:ws_id/audit-log` gated at
   `WorkspaceRole::Admin`, cursor-paginated on `(created_at, id)` following the pagination
   convention in `docs/backend.md`, filterable by `action`, `user_id` and date range. Plus
   `GET /api/admin/audit-log` for instance admins across workspaces. A simple table view
   in the admin panel — this data is worthless if reading it requires `psql`.
6. **Retention is CS-030's job**, but note here that the audit log is one of the tables it
   must handle, and that it needs a *longer* retention than messages, not the same one.

## Acceptance

- [ ] Every action in the enum is recorded, verified by a test per action.
- [ ] `ip_address` is populated on every entry that has a request context.
- [ ] Audit writes never fail the originating request.
- [ ] Workspace admins can read and filter their workspace's log in the UI.
- [ ] Login events are written by the worker, not on the request path.

## Tests

`http_tests`: for each auditable endpoint, perform the action and assert exactly one row
with the expected `action`, `resource_id` and `ip_address`. A test that simulates an audit
write failure and asserts the request still returns 200. A pagination test on the read
endpoint, and the standard authorization matrix on both read endpoints.
