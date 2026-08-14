-- no-transaction
--
-- DMs were never searchable; this is the index that makes them so.

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_conversation_messages_search_vector
    ON conversation_messages USING GIN (search_vector);
