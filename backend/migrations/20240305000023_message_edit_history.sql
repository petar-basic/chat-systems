-- Editing a message overwrote it: only `updated_at` changed and the previous
-- text was gone. The product asserted that an edit had happened and could not
-- say what it was.
--
-- Two tables with one shape rather than one table with a nullable pair of
-- foreign keys: that shape is the ownership-modelling problem CS-009 had to
-- solve for attachments, and there is no reason to reintroduce it here.
CREATE TABLE message_edits (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    message_id       UUID NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    previous_content TEXT NOT NULL,
    edited_by        UUID NOT NULL REFERENCES users(id),
    edited_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_message_edits_message ON message_edits(message_id, edited_at DESC);

CREATE TABLE conversation_message_edits (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    message_id       UUID NOT NULL REFERENCES conversation_messages(id) ON DELETE CASCADE,
    previous_content TEXT NOT NULL,
    edited_by        UUID NOT NULL REFERENCES users(id),
    edited_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_conversation_message_edits_message
    ON conversation_message_edits(message_id, edited_at DESC);
