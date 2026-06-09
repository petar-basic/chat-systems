-- Huddle history: persistent record of live voice/video sessions and who joined.
-- Live membership/signaling is ephemeral (realtime + Redis); these tables exist
-- for "missed huddle" surfacing and analytics, and as the source of truth the
-- API consumer uses to emit huddle.ended when the last participant leaves.

CREATE TABLE huddle_sessions (
    id              UUID PRIMARY KEY,
    workspace_id    UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    channel_id      UUID REFERENCES channels(id) ON DELETE CASCADE,
    dm_partner_id   UUID REFERENCES users(id) ON DELETE CASCADE,
    initiated_by    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    started_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ended_at        TIMESTAMPTZ
);

CREATE INDEX idx_huddle_sessions_channel ON huddle_sessions(channel_id) WHERE channel_id IS NOT NULL;
CREATE INDEX idx_huddle_sessions_workspace ON huddle_sessions(workspace_id);

CREATE TABLE huddle_participants (
    huddle_id       UUID NOT NULL REFERENCES huddle_sessions(id) ON DELETE CASCADE,
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    joined_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    left_at         TIMESTAMPTZ,
    PRIMARY KEY (huddle_id, user_id)
);

CREATE INDEX idx_huddle_participants_active ON huddle_participants(huddle_id) WHERE left_at IS NULL;
