-- Emoji reactions on direct messages (mirrors the channel `reactions` table, but
-- FKs into direct_messages). `message_id` keeps the same column name as `reactions`
-- so the client can reuse one Reaction shape for channel + DM reactions.
CREATE TABLE IF NOT EXISTS dm_reactions (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    message_id  UUID NOT NULL REFERENCES direct_messages(id) ON DELETE CASCADE,
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    emoji       VARCHAR(50) NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (message_id, user_id, emoji)
);

CREATE INDEX IF NOT EXISTS dm_reactions_message_id_idx ON dm_reactions(message_id);
