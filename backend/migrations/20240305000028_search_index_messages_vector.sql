-- no-transaction
--
-- Built concurrently, and alone in this file: Postgres wraps a multi-statement
-- string in an implicit transaction, and a concurrent build refuses to run
-- inside one. A failed concurrent build leaves an INVALID index behind -- see
-- RUNBOOK.md for how to spot and drop one.

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_messages_search_vector
    ON messages USING GIN (search_vector);
