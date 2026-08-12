-- The channel list ran an EXISTS subquery per channel to decide whether it had
-- anything unread, so the cost grew with message volume rather than with channel
-- count and could only ever answer "any", never "how many".
--
-- Mentions are counted separately: a mention badge and an unread badge are
-- different things in the UI, and deriving one from the other needs exactly the
-- subquery this replaces.
ALTER TABLE channel_members
    ADD COLUMN unread_count INT NOT NULL DEFAULT 0,
    ADD COLUMN mention_count INT NOT NULL DEFAULT 0,
    ADD COLUMN last_read_message_id UUID;

UPDATE channel_members cm
   SET unread_count = counted.total,
       last_read_message_id = cm.last_read_msg
  FROM (
      SELECT member.channel_id,
             member.user_id,
             COUNT(*) AS total
        FROM channel_members member
        JOIN messages msg ON msg.channel_id = member.channel_id
       WHERE msg.deleted_at IS NULL
         AND msg.user_id <> member.user_id
         AND (member.last_read_at IS NULL OR msg.created_at > member.last_read_at)
       GROUP BY member.channel_id, member.user_id
  ) AS counted
 WHERE counted.channel_id = cm.channel_id
   AND counted.user_id = cm.user_id;

CREATE INDEX idx_channel_members_unread
    ON channel_members(user_id, channel_id)
    WHERE unread_count > 0;
