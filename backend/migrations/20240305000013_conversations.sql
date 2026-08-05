-- Conversations subsume direct messages: a `direct` conversation is the 1:1 case of the
-- same model that backs group DMs, so read state, reactions and fan-out have one shape.

CREATE TYPE conversation_kind AS ENUM ('direct', 'group');

CREATE TABLE conversations (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id    UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    kind            conversation_kind NOT NULL,
    created_by      UUID REFERENCES users(id),
    last_message_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    legacy_pair_key TEXT
);

CREATE INDEX idx_conversations_workspace ON conversations(workspace_id, last_message_at DESC);

CREATE TABLE conversation_participants (
    conversation_id UUID NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    last_read_at    TIMESTAMPTZ,
    joined_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (conversation_id, user_id)
);

CREATE INDEX idx_conversation_participants_user ON conversation_participants(user_id);

CREATE TABLE conversation_messages (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    conversation_id UUID NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    content         TEXT NOT NULL,
    edited_at       TIMESTAMPTZ,
    deleted_at      TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_conversation_messages_feed
    ON conversation_messages(conversation_id, created_at DESC, id DESC)
    WHERE deleted_at IS NULL;

CREATE TABLE conversation_message_reactions (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    message_id  UUID NOT NULL REFERENCES conversation_messages(id) ON DELETE CASCADE,
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    emoji       VARCHAR(50) NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (message_id, user_id, emoji)
);

CREATE INDEX idx_conversation_message_reactions_message
    ON conversation_message_reactions(message_id);

INSERT INTO conversations (workspace_id, kind, created_by, created_at, last_message_at, legacy_pair_key)
SELECT workspace_id,
       'direct',
       LEAST(from_user_id, to_user_id),
       MIN(created_at),
       MAX(created_at),
       workspace_id || ':' || LEAST(from_user_id, to_user_id) || ':' || GREATEST(from_user_id, to_user_id)
FROM direct_messages
GROUP BY workspace_id, LEAST(from_user_id, to_user_id), GREATEST(from_user_id, to_user_id);

INSERT INTO conversation_participants (conversation_id, user_id, joined_at)
SELECT c.id, split_part(c.legacy_pair_key, ':', 2)::uuid, c.created_at
FROM conversations c
WHERE c.legacy_pair_key IS NOT NULL;

INSERT INTO conversation_participants (conversation_id, user_id, joined_at)
SELECT c.id, split_part(c.legacy_pair_key, ':', 3)::uuid, c.created_at
FROM conversations c
WHERE c.legacy_pair_key IS NOT NULL;

INSERT INTO conversation_messages (id, conversation_id, user_id, content, edited_at, deleted_at, created_at, updated_at)
SELECT dm.id,
       c.id,
       dm.from_user_id,
       dm.content,
       dm.edited_at,
       dm.deleted_at,
       dm.created_at,
       dm.updated_at
FROM direct_messages dm
JOIN conversations c
  ON c.legacy_pair_key = dm.workspace_id || ':' || LEAST(dm.from_user_id, dm.to_user_id) || ':' || GREATEST(dm.from_user_id, dm.to_user_id);

INSERT INTO conversation_message_reactions (id, message_id, user_id, emoji, created_at)
SELECT id, message_id, user_id, emoji, created_at FROM dm_reactions;

UPDATE conversation_participants cp
SET last_read_at = r.last_read_at
FROM dm_reads r
JOIN conversations c
  ON c.legacy_pair_key = r.workspace_id || ':' || LEAST(r.user_id, r.partner_id) || ':' || GREATEST(r.user_id, r.partner_id)
WHERE cp.conversation_id = c.id AND cp.user_id = r.user_id;

ALTER TABLE conversations DROP COLUMN legacy_pair_key;

DROP TABLE dm_reactions;
DROP TABLE dm_reads;
DROP TABLE direct_messages;
