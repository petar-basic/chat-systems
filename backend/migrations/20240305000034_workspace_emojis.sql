-- Per workspace, because a shortcode is a shared vocabulary: two workspaces on
-- one instance can each have their own `:shipit:` without either being wrong.
CREATE TABLE workspace_emojis (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name         VARCHAR(64) NOT NULL,
    storage_key  TEXT NOT NULL,
    created_by   UUID NOT NULL REFERENCES users(id),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (workspace_id, name)
);

CREATE INDEX idx_workspace_emojis_workspace ON workspace_emojis(workspace_id, name);
