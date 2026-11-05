CREATE TABLE conversation_departures (
    channel_id           UUID NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    user_id              UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role                 channel_role NOT NULL DEFAULT 'member',
    joined_at            TIMESTAMPTZ NOT NULL,
    left_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_read_at         TIMESTAMPTZ,
    last_read_message_id UUID,
    is_starred           BOOLEAN NOT NULL DEFAULT FALSE,
    muted                BOOLEAN NOT NULL DEFAULT FALSE,
    mention_count        INTEGER NOT NULL DEFAULT 0,
    notify_level         notify_level,
    PRIMARY KEY (channel_id, user_id)
);

CREATE INDEX idx_conversation_departures_user ON conversation_departures(user_id);

CREATE TEMP TABLE lost_participants AS
SELECT m.channel_id,
       m.user_id,
       c.channel_type,
       c.workspace_id,
       MIN(m.created_at) AS first_said,
       MAX(m.created_at) AS last_said,
       EXISTS (SELECT 1 FROM workspace_members wm
                WHERE wm.workspace_id = c.workspace_id AND wm.user_id = m.user_id) AS is_back
  FROM messages m
  JOIN channels c ON c.id = m.channel_id
 WHERE c.channel_type IN ('dm', 'group_dm')
   AND NOT EXISTS (SELECT 1 FROM channel_members cm
                    WHERE cm.channel_id = m.channel_id AND cm.user_id = m.user_id)
 GROUP BY m.channel_id, m.user_id, c.channel_type, c.workspace_id;

INSERT INTO conversation_departures (channel_id, user_id, joined_at, left_at, last_read_at)
SELECT channel_id, user_id, first_said, last_said, last_said
  FROM lost_participants
 WHERE NOT is_back;

INSERT INTO channel_members (channel_id, user_id, role, joined_at, last_read_at, unread_count)
SELECT l.channel_id, l.user_id, 'member', l.first_said, l.last_said,
       (SELECT COUNT(*)::int FROM messages m
         WHERE m.channel_id = l.channel_id
           AND m.deleted_at IS NULL
           AND (m.thread_parent_id IS NULL OR m.metadata->>'also_in_channel' = 'true')
           AND m.user_id <> l.user_id
           AND m.created_at > l.last_said)
  FROM lost_participants l
 WHERE l.is_back AND l.channel_type = 'dm'
ON CONFLICT (channel_id, user_id) DO NOTHING;

DROP TABLE lost_participants;
