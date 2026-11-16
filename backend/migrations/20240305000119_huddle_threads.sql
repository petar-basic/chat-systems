ALTER TABLE huddle_sessions ADD COLUMN message_id UUID REFERENCES messages(id) ON DELETE SET NULL;

CREATE UNIQUE INDEX idx_huddle_sessions_message ON huddle_sessions (message_id) WHERE message_id IS NOT NULL;

ALTER TABLE huddle_participants ADD COLUMN followed_on_join BOOLEAN NOT NULL DEFAULT FALSE;
