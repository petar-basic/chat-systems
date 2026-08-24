-- Phase 2, separated from phase 1 so an operator who wants to be able to roll
-- the binary back can stop after 27 (`sqlx migrate run --target-version`), run
-- the new code against the new column for a while, and only then drop the old
-- one. Applied together it is still correct -- the column is read by nothing
-- once the query has moved.
--
-- DROP COLUMN is a catalog change, not a rewrite: the old vectors stay on disk
-- until the rows are next written, which is why this is safe to run at any time.

DROP INDEX IF EXISTS idx_messages_search;
ALTER TABLE messages DROP COLUMN IF EXISTS content_search;
