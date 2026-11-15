CREATE TABLE drafts (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id       UUID NOT NULL,
    user_id            UUID NOT NULL,
    channel_id         UUID NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    thread_parent_id   UUID REFERENCES messages(id) ON DELETE CASCADE,
    content            TEXT NOT NULL,
    files_left_behind  BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at         TIMESTAMPTZ NOT NULL,
    FOREIGN KEY (workspace_id, user_id) REFERENCES workspace_members (workspace_id, user_id) ON DELETE CASCADE,
    CONSTRAINT drafts_place UNIQUE NULLS NOT DISTINCT (user_id, channel_id, thread_parent_id)
);

CREATE INDEX drafts_written ON drafts (user_id, workspace_id, updated_at DESC) WHERE content <> '';

CREATE INDEX idx_messages_sent ON messages (user_id, created_at DESC, id DESC) WHERE deleted_at IS NULL;
