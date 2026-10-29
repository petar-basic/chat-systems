CREATE TABLE command_invocations (
    id UUID PRIMARY KEY,
    hook_id UUID NOT NULL REFERENCES hooks(id) ON DELETE CASCADE,
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    channel_id UUID NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    command TEXT NOT NULL,
    payload JSONB NOT NULL,
    uses_left INT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    attempts INT NOT NULL DEFAULT 1,
    retry_at TIMESTAMPTZ,
    answer_message_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX command_invocations_retry_at ON command_invocations (retry_at)
    WHERE retry_at IS NOT NULL;
CREATE INDEX command_invocations_expires_at ON command_invocations (expires_at);
