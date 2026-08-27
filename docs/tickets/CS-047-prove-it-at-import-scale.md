# CS-047 — Prove it at import scale

**Wave:** 10 — Guest containment and operational readiness
**Area:** backend · ops
**Blocked by:** ~~CS-034~~ ✅ shipped (search rewrite), ~~CS-036~~ ✅ shipped (Slack import)
**Blocks:** —
**Audit finding:** MEDIUM — unmeasured risk

## Problem

The largest dataset this system has ever run against is roughly **2,200 messages**. A
company migrating off Slack arrives with two to three orders of magnitude more on day one,
in a single import, and every judgement made about performance so far has been made against
the small number.

Specific things that are reasoned about but not measured:

- **Ranked search pagination.** The query plan is right — `EXPLAIN` shows a `BitmapOr`
  across `idx_messages_search_vector` and `idx_messages_content_trgm`, no sequential scan —
  but `ORDER BY ts_rank(...) DESC ... LIMIT n OFFSET m`
  ([`messaging/repo.rs`](../../backend/api/src/messaging/repo.rs)) sorts every matching row
  before returning twenty. For a common word in a large corpus that is the classic full-text
  pagination trap, and nobody has seen the number.
- **The import itself.** Two passes, per-row triggers now writing a search vector on every
  inserted message, files fetched over the network, all inside one job.
- **The search backfill** (`search::backfill`) has only ever run against a few thousand
  rows.
- **Restore.** The procedure in RUNBOOK is written and has never been executed against a
  real dump.

None of these is predicted to be broken. All of them are unknown, and they are unknown
exactly where a migration would discover them: in production, on the first day, in front of
the whole company.

## Approach

1. **Build a realistic corpus** — 500k messages across 200 channels and 40 users, with a
   realistic length distribution and a realistic proportion of threads, reactions and files.
   A generator script committed next to `seed.sh`, so the number can be reproduced rather
   than remembered.
2. **Measure, then decide.** Record p50/p95 for: a one-word search, a common-word search, a
   substring search, channel history paging, and the unread counters. Publish the numbers in
   RUNBOOK. Only optimise what the numbers say is slow — the point of this ticket is the
   measurement, not a speculative rewrite.
3. **If ranked pagination is the problem, cap the candidate set** rather than reaching for a
   new index type: rank within the top N matches by recency, which is what a chat search
   actually wants, and is a change to one query.
4. **Rehearse the import end to end** on a real Slack export of comparable size, timed, with
   the instance serving traffic. Record how long it takes and what it does to query latency
   while it runs — an import that makes the product unusable for two hours needs to be
   documented as a maintenance window, not discovered as an outage.
5. **Run the restore drill once, for real.** Take a production-shaped backup, restore onto a
   fresh host, and time it. A restore procedure that has never been executed is a document,
   not a capability.

## Acceptance

- [ ] A reproducible generator produces a 500k-message instance.
- [ ] Search, history and unread latencies are measured and published in RUNBOOK.
- [ ] A full-size Slack import is timed, and its effect on live query latency is recorded.
- [ ] The backfill is timed at that size.
- [ ] The restore drill is executed once end to end and its duration documented.
- [ ] Any query the numbers show to be slow has a ticket of its own, with the number in it.

## Tests

Not a unit-test ticket. The deliverable is the generator, the measurements in RUNBOOK, and
whatever tickets the numbers justify. Keep the generator out of CI — it exists to be run
deliberately, not on every push.
