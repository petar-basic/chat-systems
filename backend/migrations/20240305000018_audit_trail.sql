-- The audit trail is append-only history, not relational data: it has to outlive
-- the workspace it describes. With the foreign key in place a hard workspace
-- delete would either fail on the reference or take the record of the deletion
-- down with it.
ALTER TABLE audit_log DROP CONSTRAINT IF EXISTS audit_log_workspace_id_fkey;
ALTER TABLE audit_log DROP CONSTRAINT IF EXISTS audit_log_user_id_fkey;

DROP INDEX IF EXISTS idx_audit_log_workspace;

-- Ordered by the exact tuple the read endpoints paginate on, so a page boundary
-- inside a batch of same-timestamp rows neither skips nor repeats one.
CREATE INDEX idx_audit_log_workspace_keyset
    ON audit_log(workspace_id, created_at DESC, id DESC);
CREATE INDEX idx_audit_log_keyset
    ON audit_log(created_at DESC, id DESC);
CREATE INDEX idx_audit_log_actor ON audit_log(user_id, created_at DESC);
