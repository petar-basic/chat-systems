# CS-036 — Slack import / export CLI

**Wave:** 9 — Product parity
**Area:** backend
**Blocked by:** CS-031 (shares the serialization format and the job runner)
**Blocks:** —
**Roadmap:** existing item

## Problem

A company migrating from Slack has years of history there. Without an import they either
abandon it or keep paying Slack purely as an archive — and in practice the second option
means people keep going back, and the migration fails.

## Approach

A CLI in the worker crate, not an HTTP endpoint. An import is a long, restartable, operator
-supervised operation, not a request.

1. **Input: a standard Slack export ZIP.** `users.json`, `channels.json`, and per-channel
   directories of per-day message JSON. Enterprise exports additionally carry DMs; handle
   their presence or absence explicitly rather than assuming.
2. **Map by email.** Slack users match existing accounts by email; unmatched users are
   created in `pending` status so their history is attributed correctly and they activate
   through the normal invite flow. Keep the mapping in a table, not in memory — a restart
   must not re-map.
3. **Two-pass import.** First pass creates users, channels and memberships. Second pass
   inserts messages, so `thread_ts` → `thread_parent_id` can resolve against rows that
   already exist. Threads are the part that breaks in single-pass importers.
4. **Preserve identity of the record.** Original timestamps, authorship, thread structure,
   reactions and pins. Slack `mrkdwn` converts to the markdown subset the composer
   produces — anything outside it degrades to text rather than being dropped.
5. **Files are fetched, not linked.** Slack file URLs expire. Download during import and
   store through `FileStorage` with the ownership columns from CS-009 so imported
   attachments carry the same access control as native ones.
6. **Idempotent and resumable.** Record a `slack_ts` per imported message and skip
   duplicates on re-run. A 200k-message import will be interrupted; make that survivable
   rather than a reason to start over.
7. **Dry-run and a report.** `--dry-run` produces counts and a list of what will not
   convert cleanly, before anything is written. Print a final report with per-entity counts
   and every skipped item and why.
8. **Export is CS-031.** Do not build a second export path here; if the Slack-shaped
   direction is needed, add it as a format option on the existing engine.

## Acceptance

- [ ] A standard Slack export imports users, channels, messages, threads, reactions, pins
      and files.
- [ ] Users match existing accounts by email; unmatched are created `pending`.
- [ ] Re-running skips already-imported messages.
- [ ] An interrupted import resumes without duplication.
- [ ] `--dry-run` reports without writing.
- [ ] Imported files carry the same access control as native uploads.

## Tests

Worker tests against a fixture export covering threads, reactions, pins, DMs, files, a
deleted user and a renamed channel. Assert idempotency by importing twice and comparing row
counts. Assert an interrupted run resumes. Assert imported attachment access follows the
CS-009 rules.
