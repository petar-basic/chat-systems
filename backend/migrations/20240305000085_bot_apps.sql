ALTER TABLE users ADD COLUMN is_bot BOOLEAN NOT NULL DEFAULT false;

CREATE TABLE bot_apps (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id  UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    bot_user_id   UUID NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
    name          VARCHAR(100) NOT NULL,
    description   TEXT,
    icon_url      VARCHAR(500),
    scopes        TEXT[] NOT NULL DEFAULT '{}',
    token_hash    VARCHAR(64) NOT NULL UNIQUE,
    created_by    UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (workspace_id, name)
);

ALTER TABLE hooks ADD COLUMN app_id UUID REFERENCES bot_apps(id) ON DELETE CASCADE;
CREATE INDEX hooks_app_id ON hooks (app_id) WHERE app_id IS NOT NULL;
