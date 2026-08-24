-- A handle is a name people type, so it is unique per workspace and stored
-- lowercase: `@Backend` and `@backend` are the same group, and two groups that
-- differ only in case would be indistinguishable in a message.
CREATE TABLE user_groups (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    handle       VARCHAR(64) NOT NULL,
    name         VARCHAR(120) NOT NULL,
    description  VARCHAR(500),
    created_by   UUID NOT NULL REFERENCES users(id),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (workspace_id, handle)
);

CREATE TABLE user_group_members (
    group_id  UUID NOT NULL REFERENCES user_groups(id) ON DELETE CASCADE,
    user_id   UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    added_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (group_id, user_id)
);

CREATE INDEX idx_user_group_members_user ON user_group_members(user_id);
