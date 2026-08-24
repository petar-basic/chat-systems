-- no-transaction
--
-- Over the normalized text, not the raw text: a query typed without diacritics
-- has to reach a message written with them, and an expression index is only used
-- when the query repeats the expression exactly.

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_messages_content_trgm
    ON messages USING GIN (search_normalize(content) gin_trgm_ops);
