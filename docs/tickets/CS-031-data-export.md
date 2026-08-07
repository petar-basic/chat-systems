# CS-031 — Workspace and user data export

**Wave:** 8 — Compliance
**Area:** backend
**Blocked by:** CS-018, CS-029, CS-030
**Blocks:** CS-036 (shares the serialization format)

## Problem

There is no way to get data out. Not for a legal hold, not for a subject access request,
not for leaving the product. The only export path is `pg_dump`, which is a backup, not an
export — it cannot be scoped to a workspace or a person, and it is not something you hand
to a regulator or an employee.

Two distinct needs, often confused:

- **eDiscovery / legal hold** — an admin needs everything in a workspace, or everything a
  named person wrote, over a date range, in a form an investigator can read.
- **Subject access and erasure** — an individual needs their own data, and a deletion
  request needs to be executable and provable.

## Approach

One export engine, two scopes, always asynchronous.

1. **Job-based, never synchronous.** An export of a busy workspace is minutes of work and
   hundreds of megabytes. `POST /api/workspaces/:ws_id/exports` creates a row in an
   `export_jobs` table and returns its id; the worker (CS-004) performs it; the result is
   an object in storage fetched through a short-lived, single-use download token. Do not
   stream it out of a request handler.
2. **Scopes:**
   - `workspace` — every channel, message, thread, reaction, attachment, membership change
     and audit entry in the workspace, optionally bounded by date. Requires
     `WorkspaceRole::Owner` — this is the most sensitive operation in the product.
   - `user` — everything authored by one user across the instance, plus their profile and
     memberships. Requires instance admin, or the user themselves for their own data.
3. **Format: JSONL plus a manifest.** One file per entity type, one JSON object per line, so
   a large export streams and can be processed without loading it whole. A `manifest.json`
   records scope, filters, row counts per file, the exporting actor and a SHA-256 of each
   file — the counts and hashes are what make the export defensible rather than merely
   present. Attachments go in a `files/` directory keyed by id.
4. **Include edit history** (CS-029) and the audit log (CS-018). An export without them
   answers "what does it say now", not "what happened", which is the question being asked.
5. **DM scope is a policy decision, not a default.** Private conversations are included in a
   workspace export only when the request explicitly opts in, and that opt-in is itself
   audited. Silently exporting everyone's DMs because an owner clicked a button is the kind
   of default that ends up in a news story.
6. **Erasure is a separate operation from export**, sharing the same scoping code:
   `DELETE /api/admin/users/:user_id/data` anonymizes the user (replace profile fields with
   tombstones, keep message rows attributed to a deleted-user placeholder) or hard-deletes
   their messages, depending on a flag. Anonymize is the sane default — hard-deleting one
   participant's messages makes every conversation they were in unreadable.
7. **Every export and erasure is audited**, including what was requested, by whom, and how
   many rows were produced.

## Acceptance

- [ ] Workspace and user exports run as background jobs with progress and a result link.
- [ ] The archive contains a manifest with per-file row counts and checksums.
- [ ] Edit history and audit entries are included.
- [ ] DMs are excluded unless explicitly requested, and that request is audited.
- [ ] Download links are short-lived and single-use.
- [ ] Erasure supports anonymize and hard-delete, and is audited.

## Tests

Worker tests: export a seeded workspace, assert manifest counts match the database and
checksums verify. Assert DMs are absent without the flag and present with it. Assert a
user-scope export contains only that user's authorship. Assert anonymize leaves messages
readable with a tombstone author, and hard-delete removes them. Standard authorization
matrix on all endpoints.
