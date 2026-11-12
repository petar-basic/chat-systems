-- no-transaction

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_messages_content_bigm
    ON messages USING GIN (public.search_fold(content) gin_bigm_ops);
