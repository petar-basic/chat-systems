CREATE TABLE sidebar_sections (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name         VARCHAR(80) NOT NULL,
    position     INT NOT NULL,
    collapsed    BOOLEAN NOT NULL DEFAULT false,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX idx_sidebar_sections_name
    ON sidebar_sections (user_id, workspace_id, lower(name));
CREATE INDEX idx_sidebar_sections_owner
    ON sidebar_sections (user_id, workspace_id, position);

ALTER TABLE channel_members
    ADD COLUMN section_id UUID REFERENCES sidebar_sections(id) ON DELETE SET NULL;

CREATE TABLE sidebar_collapsed_builtins (
    user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    section      TEXT NOT NULL CHECK (section IN ('channels', 'dms', 'people')),
    PRIMARY KEY (user_id, workspace_id, section)
);
