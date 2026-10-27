CREATE TABLE huddle_invites (
    huddle_id  UUID NOT NULL REFERENCES huddle_sessions(id) ON DELETE CASCADE,
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    invited_by UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    invited_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (huddle_id, user_id)
);
