CREATE TABLE workspace_apps (
    workspace_id            UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    provider                TEXT NOT NULL CHECK (provider IN ('jira', 'github', 'google_drive')),
    client_id               TEXT NOT NULL,
    client_secret_encrypted BYTEA NOT NULL,
    client_secret_nonce     BYTEA NOT NULL,
    enabled                 BOOLEAN NOT NULL DEFAULT true,
    configured_by           UUID REFERENCES users(id) ON DELETE SET NULL,
    configured_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (workspace_id, provider)
);

CREATE TABLE app_connections (
    workspace_id            UUID NOT NULL,
    user_id                 UUID NOT NULL,
    provider                TEXT NOT NULL,
    external_account        TEXT NOT NULL,
    access_token_encrypted  BYTEA NOT NULL,
    access_token_nonce      BYTEA NOT NULL,
    refresh_token_encrypted BYTEA,
    refresh_token_nonce     BYTEA,
    expires_at              TIMESTAMPTZ,
    resources               JSONB NOT NULL DEFAULT '[]'::jsonb,
    connected_at            TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (workspace_id, user_id, provider),
    FOREIGN KEY (workspace_id, user_id)
        REFERENCES workspace_members (workspace_id, user_id) ON DELETE CASCADE,
    FOREIGN KEY (workspace_id, provider)
        REFERENCES workspace_apps (workspace_id, provider) ON DELETE CASCADE
);
