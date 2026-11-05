CREATE TABLE member_daily_activity (
    workspace_id     UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    user_id          UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    day              DATE NOT NULL,
    public_messages  INT NOT NULL DEFAULT 0,
    private_messages INT NOT NULL DEFAULT 0,
    direct_messages  INT NOT NULL DEFAULT 0,
    files            INT NOT NULL DEFAULT 0,
    reactions        INT NOT NULL DEFAULT 0,
    PRIMARY KEY (workspace_id, user_id, day)
);

CREATE INDEX idx_member_daily_activity_day ON member_daily_activity (workspace_id, day);

CREATE TABLE analytics_counted_events (
    event_id   UUID PRIMARY KEY,
    counted_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_analytics_counted_events_at ON analytics_counted_events (counted_at);

CREATE TABLE analytics_backfill (
    id           BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (id),
    counted_from TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    finished_at  TIMESTAMPTZ
);

INSERT INTO analytics_backfill DEFAULT VALUES;
