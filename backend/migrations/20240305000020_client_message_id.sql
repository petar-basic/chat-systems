-- The client used to choose the primary key and a unique violation was the
-- idempotency signal, so a retry looked up the row by id alone — across every
-- conversation in the instance. The key the client controls now lives in its own
-- column, unique only within a conversation, and cannot name somebody else's row.
ALTER TABLE conversation_messages ADD COLUMN client_message_id UUID;

CREATE UNIQUE INDEX idx_conversation_messages_client_id
    ON conversation_messages(conversation_id, client_message_id)
    WHERE client_message_id IS NOT NULL;

-- The channel path had the same shape: the client chose the primary key, and the
-- retry branch looked the row up by that key alone — across every channel in the
-- instance, for a caller who only had to be a member of the one they posted to.
ALTER TABLE messages ADD COLUMN client_message_id UUID;

CREATE UNIQUE INDEX idx_messages_client_id
    ON messages(channel_id, client_message_id)
    WHERE client_message_id IS NOT NULL;
