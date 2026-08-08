-- Attachments were only ever linked to channel messages, so every file sent in a
-- DM kept `message_id IS NULL` — and a null owner meant `require_file_access`
-- asked for nothing but workspace membership. Any member could read any DM
-- attachment; the unguessable storage key was the only thing in the way.

ALTER TABLE files
    ADD COLUMN conversation_message_id UUID REFERENCES conversation_messages(id) ON DELETE SET NULL;

CREATE INDEX idx_files_conversation_message ON files (conversation_message_id);

-- Attribute the existing DM attachments rather than orphaning them: the storage
-- key appears verbatim in the message that posted it, and only the uploader's
-- own files are claimed — the same guard `link_to_message` already applies.
-- `position` rather than LIKE: a sanitized filename can contain `_`, which LIKE
-- would treat as a wildcard.
UPDATE files f
   SET conversation_message_id = cm.id
  FROM conversation_messages cm
 WHERE f.message_id IS NULL
   AND f.conversation_message_id IS NULL
   AND f.user_id = cm.user_id
   AND position('/api/files/download/' || f.storage_key in cm.content) > 0;

ALTER TABLE files
    ADD CONSTRAINT files_single_owner
    CHECK (message_id IS NULL OR conversation_message_id IS NULL);
