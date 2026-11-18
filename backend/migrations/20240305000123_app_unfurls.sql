CREATE TABLE app_unfurl_domains (
    app_id       UUID NOT NULL REFERENCES bot_apps(id) ON DELETE CASCADE,
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    domain       TEXT NOT NULL,
    position     SMALLINT NOT NULL,
    PRIMARY KEY (app_id, domain),
    UNIQUE (workspace_id, domain)
);

CREATE TABLE app_link_shares (
    app_id     UUID NOT NULL REFERENCES bot_apps(id) ON DELETE CASCADE,
    message_id UUID NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    urls       TEXT[] NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (app_id, message_id)
);

CREATE INDEX idx_app_link_shares_message ON app_link_shares (message_id);
CREATE INDEX idx_app_link_shares_expires ON app_link_shares (expires_at);
