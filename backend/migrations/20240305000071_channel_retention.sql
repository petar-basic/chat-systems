ALTER TABLE retention_policies ADD COLUMN channel_overrides BOOLEAN NOT NULL DEFAULT FALSE;

CREATE TABLE channel_retention (
    channel_id   UUID PRIMARY KEY REFERENCES channels(id) ON DELETE CASCADE,
    message_days INT CHECK (message_days IS NULL OR message_days > 0),
    updated_by   UUID REFERENCES users(id) ON DELETE SET NULL,
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
