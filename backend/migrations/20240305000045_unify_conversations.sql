-- A direct or group message is a message in a channel nobody can browse. Two
-- parallel sets of tables (conversation_messages, conversation_message_reactions,
-- conversation_message_edits, plus a nullable second foreign key on files,
-- saved_messages and scheduled_messages) carried the same shape as the channel
-- ones, and every feature had to be written twice. Conversations become channels
-- of type `dm` / `group_dm`; ids are preserved so links and clients keep working.

INSERT INTO channels (id, workspace_id, name, channel_type, created_by, is_default, is_archived, settings, created_at, updated_at)
SELECT c.id,
       c.workspace_id,
       NULL,
       CASE c.kind WHEN 'direct' THEN 'dm'::channel_type ELSE 'group_dm'::channel_type END,
       c.created_by,
       FALSE,
       FALSE,
       '{}'::jsonb,
       c.created_at,
       c.last_message_at
  FROM conversations c;

INSERT INTO channel_members (channel_id, user_id, role, last_read_at, joined_at)
SELECT p.conversation_id, p.user_id, 'member', p.last_read_at, p.joined_at
  FROM conversation_participants p;

INSERT INTO messages (id, channel_id, user_id, content, metadata, thread_parent_id, reply_count, is_pinned,
                      created_at, updated_at, deleted_at, client_message_id, slack_ts, search_vector)
SELECT cm.id, cm.conversation_id, cm.user_id, cm.content, '{}'::jsonb, cm.thread_parent_id, cm.reply_count, FALSE,
       cm.created_at, cm.updated_at, cm.deleted_at, cm.client_message_id, cm.slack_ts, cm.search_vector
  FROM conversation_messages cm;

INSERT INTO reactions (id, message_id, user_id, emoji, created_at)
SELECT id, message_id, user_id, emoji, created_at FROM conversation_message_reactions;

INSERT INTO message_edits (id, message_id, previous_content, edited_by, edited_at)
SELECT id, message_id, previous_content, edited_by, edited_at FROM conversation_message_edits;

UPDATE channel_members cm
   SET unread_count = counts.n
  FROM (
       SELECT m.channel_id, p.user_id, COUNT(*) AS n
         FROM conversation_participants p
         JOIN messages m ON m.channel_id = p.conversation_id
        WHERE m.deleted_at IS NULL
          AND m.thread_parent_id IS NULL
          AND m.user_id <> p.user_id
          AND (p.last_read_at IS NULL OR m.created_at > p.last_read_at)
        GROUP BY m.channel_id, p.user_id
       ) counts
 WHERE cm.channel_id = counts.channel_id AND cm.user_id = counts.user_id;

ALTER TABLE files DROP CONSTRAINT files_single_owner;
UPDATE files SET message_id = conversation_message_id WHERE conversation_message_id IS NOT NULL;
DROP INDEX IF EXISTS idx_files_conversation_message;
ALTER TABLE files DROP COLUMN conversation_message_id;

ALTER TABLE saved_messages DROP CONSTRAINT saved_messages_one_target;
UPDATE saved_messages SET message_id = conversation_message_id WHERE conversation_message_id IS NOT NULL;
DROP INDEX IF EXISTS idx_saved_messages_conversation_once;
DROP INDEX IF EXISTS idx_saved_messages_channel_once;
ALTER TABLE saved_messages DROP COLUMN conversation_message_id;
ALTER TABLE saved_messages ALTER COLUMN message_id SET NOT NULL;
CREATE UNIQUE INDEX idx_saved_messages_once ON saved_messages(user_id, message_id);

ALTER TABLE scheduled_messages DROP CONSTRAINT scheduled_messages_one_target;
UPDATE scheduled_messages SET channel_id = conversation_id WHERE conversation_id IS NOT NULL;
ALTER TABLE scheduled_messages DROP COLUMN conversation_id;
ALTER TABLE scheduled_messages ALTER COLUMN channel_id SET NOT NULL;

INSERT INTO slack_channels (workspace_id, slack_channel_id, channel_id, created_at)
SELECT workspace_id, slack_channel_id, conversation_id, created_at FROM slack_conversations
ON CONFLICT (workspace_id, slack_channel_id) DO NOTHING;
DROP TABLE slack_conversations;

DROP TABLE conversation_message_edits;
DROP TABLE conversation_message_reactions;
DROP TABLE conversation_messages;
DROP TABLE conversation_participants;
DROP TABLE conversations;
DROP TYPE conversation_kind;
