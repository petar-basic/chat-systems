-- Per-(user, workspace, partner) last-read marker for direct messages, so DM
-- unread state is durable (survives reload) and synced across devices instead
-- of living only in the client's in-memory Zustand set.
CREATE TABLE IF NOT EXISTS dm_reads (
    user_id       UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    workspace_id  UUID        NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    partner_id    UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    last_read_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, workspace_id, partner_id)
);
