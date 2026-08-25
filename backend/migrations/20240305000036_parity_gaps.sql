-- Six small gaps against Slack, all of them a missing surface rather than a
-- missing idea: threads in conversations, saved messages, channel bookmarks and
-- a status somebody sets by hand.

-- Threads reach conversations. Channels have had them since the beginning; the
-- column and counter mirror `messages` exactly so the client can reuse its shape.
ALTER TABLE conversation_messages
    ADD COLUMN thread_parent_id UUID REFERENCES conversation_messages(id) ON DELETE CASCADE,
    ADD COLUMN reply_count INT NOT NULL DEFAULT 0;

CREATE INDEX idx_conversation_messages_thread
    ON conversation_messages(thread_parent_id, created_at)
    WHERE thread_parent_id IS NOT NULL;

-- The conversation feed shows roots only, the way the channel feed does.
DROP INDEX IF EXISTS idx_conversation_messages_feed;
CREATE INDEX idx_conversation_messages_feed
    ON conversation_messages(conversation_id, created_at DESC, id DESC)
    WHERE deleted_at IS NULL AND thread_parent_id IS NULL;

-- Saved messages are per person, and point at either kind of message. Exactly
-- one target, the same shape `scheduled_messages` uses.
CREATE TABLE saved_messages (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id                 UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    workspace_id            UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    message_id              UUID REFERENCES messages(id) ON DELETE CASCADE,
    conversation_message_id UUID REFERENCES conversation_messages(id) ON DELETE CASCADE,
    note                    TEXT,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT saved_messages_one_target CHECK (
        (message_id IS NOT NULL AND conversation_message_id IS NULL)
        OR (message_id IS NULL AND conversation_message_id IS NOT NULL)
    )
);

CREATE UNIQUE INDEX idx_saved_messages_channel_once
    ON saved_messages(user_id, message_id)
    WHERE message_id IS NOT NULL;

CREATE UNIQUE INDEX idx_saved_messages_conversation_once
    ON saved_messages(user_id, conversation_message_id)
    WHERE conversation_message_id IS NOT NULL;

CREATE INDEX idx_saved_messages_owner ON saved_messages(user_id, workspace_id, created_at DESC);

-- Channel bookmarks are shared, not personal: they are the pinned links a
-- channel keeps at the top, so they follow channel moderation rights.
CREATE TABLE channel_bookmarks (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    channel_id  UUID NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    created_by  UUID REFERENCES users(id) ON DELETE SET NULL,
    label       VARCHAR(80) NOT NULL,
    url         TEXT NOT NULL,
    emoji       VARCHAR(50),
    position    INT NOT NULL DEFAULT 0,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_channel_bookmarks_channel ON channel_bookmarks(channel_id, position, created_at);

-- A status somebody sets by hand, which is a different thing from presence:
-- presence says whether a socket is open, this says what the person wants read
-- next to their name. It expires on its own so nobody is "out for lunch" at 6pm.
ALTER TABLE users
    ADD COLUMN status_emoji      VARCHAR(50),
    ADD COLUMN status_text       VARCHAR(100),
    ADD COLUMN status_expires_at TIMESTAMPTZ;
