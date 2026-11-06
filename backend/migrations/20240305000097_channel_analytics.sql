CREATE TABLE channel_member_activity (
    channel_id UUID NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    day        DATE NOT NULL,
    messages   INT NOT NULL DEFAULT 0,
    reactions  INT NOT NULL DEFAULT 0,
    read       BOOLEAN NOT NULL DEFAULT FALSE,
    PRIMARY KEY (channel_id, user_id, day)
);

CREATE INDEX idx_channel_member_activity_day ON channel_member_activity (channel_id, day);

TRUNCATE member_daily_activity, analytics_counted_events;

UPDATE analytics_backfill SET counted_from = NOW(), finished_at = NULL;
