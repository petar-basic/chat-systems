-- no-transaction

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_file_texts_content_bigm
    ON file_texts USING GIN (public.search_fold(content) gin_bigm_ops) WHERE status = 'done';
