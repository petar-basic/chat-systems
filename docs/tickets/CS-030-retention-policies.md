# CS-030 — Retention policies and token cleanup

**Wave:** 8 — Compliance
**Area:** backend
**Blocked by:** ~~CS-004~~ ✅ shipped as `chat-worker`, CS-018, CS-020, CS-029
**Blocks:** CS-031
**Roadmap:** existing item, expanded

## Problem

Nothing is ever deleted. Messages, files, audit entries, notifications, hook execution
logs and expired tokens accumulate for the life of the instance.

Concretely:

- `password_reset_tokens` rows are consumed but never removed
  ([`auth/repo.rs`](../../backend/api/src/auth/repo.rs)) — the table grows monotonically
  and holds `jti` values for every reset ever requested.
- `refresh_tokens` rows past `expires_at` are never removed.
- `hook_executions` records the full payload of every webhook call.
- Soft-deleted messages and, after CS-020, soft-deleted files are retained indefinitely
  along with their object-store contents.

A company cannot answer "how long do you keep our chat data" with "forever" and pass a
review. The absence of a purge also means a GDPR deletion request has no mechanism.

## Approach

A single scheduled job in the worker, with per-workspace configuration.

1. **Policy per workspace, defaults instance-wide:**
   ```sql
   CREATE TABLE retention_policies (
       workspace_id       UUID PRIMARY KEY REFERENCES workspaces(id) ON DELETE CASCADE,
       message_days       INT,
       file_days          INT,
       audit_days         INT NOT NULL DEFAULT 730,
       notification_days  INT NOT NULL DEFAULT 90,
       updated_by         UUID REFERENCES users(id),
       updated_at         TIMESTAMPTZ NOT NULL DEFAULT NOW()
   );
   ```
   `NULL` means keep forever, which stays the default for messages and files — turning on
   deletion must be a deliberate act. **Audit retention is longer than message retention by
   default**, because the log is what answers questions about deleted data.
2. **Nightly job in the worker**, one workspace at a time, deleting in bounded batches with
   a sleep between them so a first run on a large instance does not lock the database.
   Order matters: dependent rows first (reactions, edits, attachments), then the message.
3. **Files need two steps.** Delete the object from storage, then the row — and tolerate a
   missing object, since a previous run may have half-completed. Objects for
   soft-deleted files (CS-020) are purged here rather than on the request path.
4. **Unconditional cleanups**, no policy required, because there is no argument for keeping
   them: consumed and expired `password_reset_tokens`, expired `refresh_tokens`, expired
   `workspace_invites` (after CS-006 they all have an expiry), `hook_executions` older than
   30 days, and orphaned files with no owner older than 7 days (CS-020 step 5).
5. **Every purge is audited** with counts per table, and exported as
   `retention_rows_deleted_total{table}`. A retention job that quietly deletes the wrong
   thing must be visible in a metric before it is visible in a support ticket.
6. **Dry-run mode.** `RETENTION_DRY_RUN=true` logs what would be deleted without deleting.
   Anyone enabling retention on a live instance should be able to see the blast radius
   first, and it makes the feature testable against production-shaped data.
7. **Do not partition yet.** Partitioning `messages` and `audit_log` is the right move at a
   scale this instance is nowhere near. Revisit when a metric says so, not before.

## Acceptance

- [ ] Retention is configurable per workspace and defaults to keeping messages and files.
- [ ] The job deletes in bounded batches without long locks.
- [ ] Objects are removed from storage before their rows.
- [ ] Expired tokens, invites and hook executions are cleaned unconditionally.
- [ ] Dry-run mode reports without deleting.
- [ ] Deletion counts are audited and exported as metrics.
- [ ] `RUNBOOK.md` documents enabling retention and the irreversibility of it.

## Tests

Worker tests: seed data across the boundary and assert only the older side is removed, that
dependent rows go first, and that a missing object does not abort the run. Assert dry-run
deletes nothing. Assert audit rows survive longer than the messages they describe.
