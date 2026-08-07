# CS-034 — Configurable full-text search language

**Wave:** 9 — Product parity
**Area:** backend
**Blocked by:** CS-030 (both rewrite the `messages` table; do the retention work first so
this migration runs against a smaller table)
**Blocks:** —
**Audit finding:** P2 (MEDIUM)

## Problem

Search is hard-coded to English at the schema level
([migration 1, line 129](../../backend/migrations/20240305000001_initial_schema.sql#L129)):

```sql
content_search TSVECTOR GENERATED ALWAYS AS (to_tsvector('english', content)) STORED
```

and the query matches it
([`messaging/repo.rs:291`](../../backend/api/src/messaging/repo.rs#L291)).

For a non-English team this is wrong in both directions: English stemming mangles the
actual language, and words that differ only by diacritics do not match. For a Serbian,
Croatian or Bosnian team — Latin and Cyrillic in the same channel — search is close to
useless.

Because the column is `GENERATED ALWAYS ... STORED`, this is not a configuration change.
Altering it rewrites the entire `messages` table.

## Approach

Stop pinning a language in the schema, and make matching diacritic- and script-tolerant.

1. **Switch the stored vector to the `simple` configuration** plus `unaccent`, which does
   no stemming and no stop-word removal — correct for a mixed-language instance and never
   actively wrong, unlike a language guess:
   ```sql
   CREATE EXTENSION IF NOT EXISTS unaccent;
   CREATE EXTENSION IF NOT EXISTS pg_trgm;
   ```
   Rebuild the generated column over `unaccent(content)` with the `simple` configuration.
   The query uses the matching `plainto_tsquery('simple', unaccent($1))`.
2. **Add trigram search for substring and typo tolerance**, which is what users actually
   expect from a chat search and what `to_tsvector` cannot give:
   ```sql
   CREATE INDEX idx_messages_content_trgm ON messages USING GIN (content gin_trgm_ops);
   ```
   Combine both signals in the ranking rather than choosing one: exact vector match ranks
   above trigram similarity.
3. **Make the configuration an instance setting** (`SEARCH_TEXT_CONFIG`, default `simple`)
   for deployments that are genuinely monolingual and want stemming. Because the column is
   generated, changing it requires a migration — document that, rather than pretending it
   is a runtime toggle.
4. **Transliteration is out of scope.** Matching Cyrillic input against Latin text is a
   separate feature; note it here and do not smuggle it in.
5. **The migration rewrites the table.** Write it as a two-phase change — add the new
   column and index concurrently, backfill in batches, swap the query, drop the old column
   — so a large instance is not offline. Document the expected duration in `RUNBOOK.md`.
6. **Extend search to conversations.** DMs are not searchable at all today, which is a
   product gap of the same shape. Give `conversation_messages` the same vector and index
   and add a scope parameter to the search endpoint, keeping the visibility rule from
   CS-010 for channels and participant checks for conversations.

## Acceptance

- [ ] Search matches regardless of diacritics.
- [ ] Substring and near-miss queries return sensible results, ranked below exact matches.
- [ ] DMs are searchable, scoped to participants.
- [ ] The migration runs online on a large table; duration documented.
- [ ] `SEARCH_TEXT_CONFIG` is documented as migration-time, not runtime.

## Tests

`http_tests/messaging.rs`: index messages with and without diacritics and assert both
directions match. Assert a substring query hits. Assert conversation results respect
participation and channel results respect CS-010's guest rule.
