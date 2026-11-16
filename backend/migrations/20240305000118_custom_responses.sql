CREATE TYPE workspace_bot_kind AS ENUM ('responses');

CREATE TABLE workspace_bots (
    workspace_id  UUID NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    kind          workspace_bot_kind NOT NULL,
    user_id       UUID NOT NULL UNIQUE REFERENCES users (id) ON DELETE CASCADE,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (workspace_id, kind)
);

CREATE TABLE custom_responses (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id  UUID NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    triggers      TEXT[] NOT NULL CHECK (cardinality(triggers) > 0),
    answers       TEXT[] NOT NULL CHECK (cardinality(answers) > 0),
    created_by    UUID REFERENCES users (id) ON DELETE SET NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX custom_responses_workspace ON custom_responses (workspace_id, created_at);

CREATE TABLE custom_response_replies (
    message_id    UUID PRIMARY KEY REFERENCES messages (id) ON DELETE CASCADE,
    response_id   UUID REFERENCES custom_responses (id) ON DELETE SET NULL,
    channel_id    UUID NOT NULL REFERENCES channels (id) ON DELETE CASCADE,
    reply_id      UUID REFERENCES messages (id) ON DELETE SET NULL,
    answered_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX custom_response_replies_recent
    ON custom_response_replies (response_id, channel_id, answered_at DESC);
