-- Messages queued for later delivery. Exactly one target: a channel or a conversation.
CREATE TABLE scheduled_messages (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id    UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    channel_id      UUID REFERENCES channels(id) ON DELETE CASCADE,
    conversation_id UUID REFERENCES conversations(id) ON DELETE CASCADE,
    content         TEXT NOT NULL,
    send_at         TIMESTAMPTZ NOT NULL,
    sent_at         TIMESTAMPTZ,
    canceled_at     TIMESTAMPTZ,
    failure         TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT scheduled_messages_one_target CHECK (
        (channel_id IS NOT NULL AND conversation_id IS NULL)
        OR (channel_id IS NULL AND conversation_id IS NOT NULL)
    )
);

CREATE INDEX idx_scheduled_messages_pending
    ON scheduled_messages(send_at)
    WHERE sent_at IS NULL AND canceled_at IS NULL;

CREATE INDEX idx_scheduled_messages_author
    ON scheduled_messages(user_id, workspace_id, send_at);
