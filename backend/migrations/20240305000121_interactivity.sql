ALTER TABLE bot_apps ADD COLUMN interactivity BOOLEAN NOT NULL DEFAULT false;

ALTER TABLE app_deliveries ADD COLUMN response JSONB;

CREATE TABLE app_triggers (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    app_id     UUID NOT NULL REFERENCES bot_apps(id) ON DELETE CASCADE,
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    stack_id      UUID,
    invocation_id UUID,
    expires_at    TIMESTAMPTZ NOT NULL,
    used_at       TIMESTAMPTZ
);

CREATE INDEX idx_app_triggers_expires ON app_triggers (expires_at);

CREATE TABLE app_interactions (
    id         UUID PRIMARY KEY,
    app_id     UUID NOT NULL REFERENCES bot_apps(id) ON DELETE CASCADE,
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    message_id UUID NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    uses_left  INTEGER NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_app_interactions_expires ON app_interactions (expires_at);

CREATE TABLE app_views (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    stack_id     UUID NOT NULL,
    position     SMALLINT NOT NULL,
    app_id       UUID NOT NULL REFERENCES bot_apps(id) ON DELETE CASCADE,
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    view         JSONB NOT NULL,
    hash         TEXT NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (stack_id, position)
);

CREATE INDEX idx_app_views_user ON app_views (user_id, workspace_id);
CREATE INDEX idx_app_views_updated ON app_views (updated_at);
