CREATE TABLE direct_messages (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id    UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    from_user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    to_user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    content         TEXT NOT NULL,
    edited_at       TIMESTAMPTZ,
    deleted_at      TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Efficient bidirectional conversation lookup
CREATE INDEX idx_dm_conversation ON direct_messages (
    workspace_id,
    LEAST(from_user_id, to_user_id),
    GREATEST(from_user_id, to_user_id),
    created_at DESC
) WHERE deleted_at IS NULL;

-- For listing all conversations a user is part of
CREATE INDEX idx_dm_from_user ON direct_messages (workspace_id, from_user_id, created_at DESC) WHERE deleted_at IS NULL;
CREATE INDEX idx_dm_to_user ON direct_messages (workspace_id, to_user_id, created_at DESC) WHERE deleted_at IS NULL;
