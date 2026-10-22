CREATE TABLE app_prompt_optouts (
    workspace_id UUID NOT NULL,
    user_id      UUID NOT NULL,
    provider     TEXT NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (workspace_id, user_id, provider),
    FOREIGN KEY (workspace_id, user_id)
        REFERENCES workspace_members (workspace_id, user_id) ON DELETE CASCADE
);
