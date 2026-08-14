-- no-transaction
--
-- The conversation side of the same substring search.

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_conversation_messages_content_trgm
    ON conversation_messages USING GIN (search_normalize(content) gin_trgm_ops);
