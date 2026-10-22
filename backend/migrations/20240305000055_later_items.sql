CREATE TABLE later_items (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL,
    user_id      UUID NOT NULL,
    created_by   UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    message_id   UUID REFERENCES messages(id) ON DELETE CASCADE,
    channel_id   UUID REFERENCES channels(id) ON DELETE CASCADE,
    text         TEXT,
    note         TEXT,
    status       TEXT NOT NULL DEFAULT 'in_progress'
                 CHECK (status IN ('in_progress', 'completed', 'archived')),
    remind_at    TIMESTAMPTZ,
    reminded_at  TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (message_id IS NOT NULL OR text IS NOT NULL),
    FOREIGN KEY (workspace_id, user_id)
        REFERENCES workspace_members (workspace_id, user_id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX later_items_one_per_message ON later_items (user_id, message_id)
    WHERE message_id IS NOT NULL;
CREATE INDEX later_items_owner ON later_items (user_id, workspace_id, status);
CREATE INDEX later_items_due ON later_items (remind_at)
    WHERE remind_at IS NOT NULL AND reminded_at IS NULL;

INSERT INTO later_items (workspace_id, user_id, created_by, message_id, channel_id, note, created_at, updated_at)
SELECT s.workspace_id, s.user_id, s.user_id, s.message_id, m.channel_id, s.note, s.created_at, s.created_at
  FROM saved_messages s
  JOIN messages m ON m.id = s.message_id
  JOIN workspace_members wm ON wm.workspace_id = s.workspace_id AND wm.user_id = s.user_id;

INSERT INTO later_items (workspace_id, user_id, created_by, message_id, channel_id, text, remind_at, reminded_at,
                         created_at, updated_at)
SELECT r.workspace_id, r.target_user_id, r.created_by, r.message_id, r.channel_id, r.content, r.remind_at,
       CASE WHEN r.is_delivered THEN r.remind_at END, r.created_at, r.created_at
  FROM reminders r
  JOIN workspace_members wm ON wm.workspace_id = r.workspace_id AND wm.user_id = r.target_user_id
ON CONFLICT (user_id, message_id) WHERE message_id IS NOT NULL
DO UPDATE SET text = EXCLUDED.text,
              remind_at = EXCLUDED.remind_at,
              reminded_at = EXCLUDED.reminded_at;

DROP TABLE saved_messages;
DROP TABLE reminders;
