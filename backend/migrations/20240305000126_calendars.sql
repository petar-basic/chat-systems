ALTER TABLE workspace_apps DROP CONSTRAINT workspace_apps_provider_check;
ALTER TABLE workspace_apps ADD CONSTRAINT workspace_apps_provider_check
    CHECK (provider IN ('jira', 'github', 'google_drive', 'figma', 'confluence',
                        'google_calendar', 'outlook_calendar'));

ALTER TYPE workspace_bot_kind ADD VALUE 'google_calendar';
ALTER TYPE workspace_bot_kind ADD VALUE 'outlook_calendar';

ALTER TABLE workspace_members ADD COLUMN status_source TEXT
    CHECK (status_source IN ('google_calendar', 'outlook_calendar'));

ALTER TABLE users ADD COLUMN calendar_away_until TIMESTAMPTZ;

CREATE TABLE calendar_syncs (
    workspace_id    UUID NOT NULL,
    user_id         UUID NOT NULL,
    provider        TEXT NOT NULL,
    sync_token      TEXT,
    window_end      TIMESTAMPTZ,
    next_sync_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    leased_until    TIMESTAMPTZ,
    last_synced_at  TIMESTAMPTZ,
    last_error      TEXT,
    status_on       BOOLEAN NOT NULL DEFAULT true,
    reminders_on    BOOLEAN NOT NULL DEFAULT true,
    agenda_on       BOOLEAN NOT NULL DEFAULT true,
    agenda_at       TIME NOT NULL DEFAULT '09:00',
    agenda_sent_on  DATE,
    status_event_id TEXT,
    PRIMARY KEY (workspace_id, user_id, provider),
    FOREIGN KEY (workspace_id, user_id, provider)
        REFERENCES app_connections (workspace_id, user_id, provider) ON DELETE CASCADE
);

CREATE INDEX calendar_syncs_due ON calendar_syncs (next_sync_at);

CREATE TABLE calendar_events (
    workspace_id UUID NOT NULL,
    user_id      UUID NOT NULL,
    provider     TEXT NOT NULL,
    event_id     TEXT NOT NULL,
    title        TEXT,
    starts_at    TIMESTAMPTZ NOT NULL,
    ends_at      TIMESTAMPTZ NOT NULL,
    all_day      BOOLEAN NOT NULL,
    kind         TEXT NOT NULL CHECK (kind IN ('event', 'meeting', 'out_of_office')),
    attending    BOOLEAN NOT NULL,
    private      BOOLEAN NOT NULL,
    join_url     TEXT,
    reminded_at  TIMESTAMPTZ,
    PRIMARY KEY (workspace_id, user_id, provider, event_id),
    FOREIGN KEY (workspace_id, user_id, provider)
        REFERENCES calendar_syncs (workspace_id, user_id, provider) ON DELETE CASCADE,
    CHECK (ends_at > starts_at),
    CHECK (NOT private OR title IS NULL)
);

CREATE INDEX calendar_events_starts ON calendar_events (starts_at);
CREATE INDEX calendar_events_ends ON calendar_events (ends_at);
