# CS-048 — Search that holds up at 500,000 messages

**Wave:** 13 — opened by CS-047 on 2026-10-21, with its numbers
**Area:** backend
**Blocked by:** ~~CS-047~~ ✅ shipped (the measurement this is written against)
**Blocks:** —
**Audit finding:** HIGH — measured, user-visible

## Problem

Measured by CS-047 on a 500,000-message instance (synthetic Slack export, one member of 66
channels, `./measure-scale.py`, numbers in [RUNBOOK](../RUNBOOK.md#measured-at-500000-messages)):

| Search | p50 | p95 |
|---|---|---|
| a word in 21,000 messages | 165 ms | 190 ms |
| a word in 370 messages | 415 ms | 470 ms |
| a substring | 350 ms | 480 ms |
| a word in 334,000 messages | **1.6 s** | **1.7 s** |
| the same searches after restoring that database from a dump | **1.8–12.9 s** | **2.0–13.2 s** |

History pages in 20 ms and the unread counters in 3 ms on the same data, so this is the
search query and nothing around it. Three separate causes, each visible in `EXPLAIN ANALYZE`
of `MessageRepo::search` ([`messaging/repo.rs`](../../backend/api/src/messaging/repo.rs)):

1. **The plan depends on luck.** sqlx prepares the statement, and after five executions
   Postgres may switch to a generic plan that does not know the search term. On the
   restored database it did: a nested loop over the requester's 195 visible channels, each
   iteration running both bitmap index scans over the whole table — 195 × 7.6 ms before a
   single row is ranked. `ANALYZE` did not bring the good plan back. A query whose latency
   moves by 10× with the statistics is not a query that can be promised to anyone.
2. **Ranking sorts every match.** `ORDER BY ts_rank … LIMIT 20` ranks all 334,000 rows that
   contain a common word, and the sort spills ~120 MB to disk.
3. **The fuzzy branch runs on every search.** `search_normalize($1) <% …` is OR-ed with the
   exact match unconditionally. For a rare word the vector index finds 370 rows and the
   trigram index hands back 43,000 candidates, each rechecked with `word_similarity`; that
   is where the 415 ms goes. The synthetic vocabulary exaggerates the trigram overlap, so
   expect real text to be somewhat better — but the branch is paid for even when the exact
   match already filled the page.

## Approach

Measure each step with `./measure-scale.py` against the CS-047 corpus, on a freshly imported
database **and** on one restored from a dump; keep a step only if its numbers say so.

1. **Plan per call.** Run the search with `.persistent(false)` so every call is planned
   with its actual term, or restructure the query so the good plan does not depend on the
   term. Prove it on the restored database, where the generic plan was chosen.
2. **Cap the candidate set**, as CS-047 proposed: rank within the newest N matches
   (start at 1,000) rather than all of them. Chat search wants recent results first anyway;
   page 51 of a common word is not a real request.
3. **Fuzzy only when exact is not enough.** Run the vector match first; add the trigram
   candidates only when it returns fewer rows than the page asks for. Keeps typo tolerance,
   stops paying for it on every query.

## Acceptance

- [ ] Every search scenario in `measure-scale.py` has p95 under 300 ms at 500,000 messages.
- [ ] The same holds on a database restored from a dump, before and after `ANALYZE`.
- [ ] Ranking behaviour the existing search tests pin down is unchanged: exact matches
      above fuzzy ones, diacritics folded, guests scoped.
- [ ] RUNBOOK's search rows are re-measured and replaced.

## Tests

The existing `http_tests` search coverage must stay green. Add a repo test that a search
whose exact match fills the page returns no fuzzy-only rows, and one that a term with no
exact match still finds its fuzzy match.
