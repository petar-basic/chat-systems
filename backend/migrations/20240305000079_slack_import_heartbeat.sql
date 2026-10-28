ALTER TABLE slack_imports ADD COLUMN heartbeat_at TIMESTAMPTZ;

CREATE INDEX idx_slack_imports_running ON slack_imports(heartbeat_at)
    WHERE status = 'running';
