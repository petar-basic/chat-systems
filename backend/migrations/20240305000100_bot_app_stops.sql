ALTER TABLE bot_apps
    ADD COLUMN disabled_at TIMESTAMPTZ,
    ADD COLUMN disabled_reason TEXT;

CREATE INDEX bot_apps_created_by ON bot_apps (created_by) WHERE disabled_at IS NULL;
CREATE INDEX workspace_invites_created_by ON workspace_invites (created_by) WHERE email IS NULL;
