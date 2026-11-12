-- no-transaction

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_files_name_bigm
    ON files USING GIN (public.search_fold(filename) gin_bigm_ops);
