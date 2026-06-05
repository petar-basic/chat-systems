-- Indexes for hook_executions (table created in 20240305000001_initial_schema.sql).
-- The FK column and the time column were unindexed; outgoing-webhook dispatch now
-- writes a row per delivery, so we need efficient per-hook history lookups and
-- time-ordered scans (recent activity / retention pruning).

-- Per-hook execution history (also speeds FK cascade checks).
CREATE INDEX IF NOT EXISTS idx_hook_executions_hook_id
    ON hook_executions (hook_id);

-- Time-ordered scans of recent executions.
CREATE INDEX IF NOT EXISTS idx_hook_executions_executed_at
    ON hook_executions (executed_at DESC);
