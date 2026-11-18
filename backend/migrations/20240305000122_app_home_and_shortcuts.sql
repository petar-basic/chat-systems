ALTER TABLE bot_apps ADD COLUMN home_enabled BOOLEAN NOT NULL DEFAULT false;

CREATE TABLE app_shortcuts (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    app_id      UUID NOT NULL REFERENCES bot_apps(id) ON DELETE CASCADE,
    kind        TEXT NOT NULL CHECK (kind IN ('global', 'message')),
    name        TEXT NOT NULL,
    description TEXT NOT NULL,
    callback_id TEXT NOT NULL,
    position    SMALLINT NOT NULL,
    UNIQUE (app_id, callback_id)
);

CREATE TABLE app_homes (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    app_id       UUID NOT NULL REFERENCES bot_apps(id) ON DELETE CASCADE,
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    view         JSONB NOT NULL,
    hash         TEXT NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (app_id, user_id)
);

CREATE INDEX idx_app_homes_user ON app_homes (user_id);
