-- Indexes for foreign-key / filter columns left unindexed by the initial schema
-- (tables created in 20240305000001_initial_schema.sql). These columns are used
-- for FK cascade checks and per-user lookups but had no usable leading-column
-- index, forcing sequential scans as data grows.

-- messages.user_id: authored-by lookups and FK to users(id).
CREATE INDEX IF NOT EXISTS idx_messages_user_id
    ON messages (user_id);

-- refresh_tokens.user_id: per-user session listing and FK cascade.
CREATE INDEX IF NOT EXISTS idx_refresh_tokens_user_id
    ON refresh_tokens (user_id);

-- reactions.user_id: per-user reaction lookups and FK cascade (the UNIQUE
-- constraint leads with message_id, so user_id is not indexable on its own).
CREATE INDEX IF NOT EXISTS idx_reactions_user_id
    ON reactions (user_id);

-- workspace_members.user_id: "which workspaces is this user in" lookups and FK
-- cascade (the PK leads with workspace_id).
CREATE INDEX IF NOT EXISTS idx_workspace_members_user_id
    ON workspace_members (user_id);

-- channel_members.user_id: "which channels is this user in" lookups and FK
-- cascade (the PK leads with channel_id).
CREATE INDEX IF NOT EXISTS idx_channel_members_user_id
    ON channel_members (user_id);

-- reminders.target_user_id: per-recipient reminder lookups and FK to users(id).
CREATE INDEX IF NOT EXISTS idx_reminders_target_user_id
    ON reminders (target_user_id);

-- notifications.workspace_id: per-workspace notification filtering and FK.
CREATE INDEX IF NOT EXISTS idx_notifications_workspace_id
    ON notifications (workspace_id);
