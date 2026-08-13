-- Nothing was ever deleted. "How long do you keep our chat data" had no answer
-- other than "forever", and a deletion request had no mechanism behind it.
--
-- NULL means keep forever, and that stays the default for messages and files:
-- turning on deletion has to be a deliberate act, because there is no undo.
CREATE TABLE retention_policies (
    workspace_id       UUID PRIMARY KEY REFERENCES workspaces(id) ON DELETE CASCADE,
    message_days       INT,
    file_days          INT,
    -- Longer than messages by default: the audit log is what answers questions
    -- about data that is already gone.
    audit_days         INT NOT NULL DEFAULT 730,
    notification_days  INT NOT NULL DEFAULT 90,
    updated_by         UUID REFERENCES users(id),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT retention_policies_positive CHECK (
        (message_days IS NULL OR message_days > 0)
        AND (file_days IS NULL OR file_days > 0)
        AND audit_days > 0
        AND notification_days > 0
    )
);

-- The purge walks by age within a workspace; these are the orders it needs.
CREATE INDEX idx_messages_created ON messages(created_at);
CREATE INDEX idx_files_created ON files(created_at);
CREATE INDEX idx_notifications_created ON notifications(created_at);
CREATE INDEX idx_refresh_tokens_expiry ON refresh_tokens(expires_at);
CREATE INDEX idx_hook_executions_executed ON hook_executions(executed_at);
