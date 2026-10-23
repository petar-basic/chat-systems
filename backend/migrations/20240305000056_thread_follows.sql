CREATE TABLE thread_follows (
    user_id         UUID NOT NULL,
    workspace_id    UUID NOT NULL,
    root_message_id UUID NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    following       BOOLEAN NOT NULL DEFAULT TRUE,
    last_read_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, root_message_id),
    FOREIGN KEY (workspace_id, user_id)
        REFERENCES workspace_members (workspace_id, user_id) ON DELETE CASCADE
);

CREATE INDEX thread_follows_followers ON thread_follows (root_message_id) WHERE following;
CREATE INDEX thread_follows_followed ON thread_follows (user_id, workspace_id) WHERE following;

INSERT INTO thread_follows (user_id, workspace_id, root_message_id, last_read_at)
SELECT p.user_id, c.workspace_id, r.id, NOW()
  FROM messages r
  JOIN channels c ON c.id = r.channel_id
 CROSS JOIN LATERAL (
        SELECT r.user_id
        UNION
        SELECT m.user_id FROM messages m WHERE m.thread_parent_id = r.id AND m.deleted_at IS NULL
       ) p
  JOIN workspace_members wm ON wm.workspace_id = c.workspace_id AND wm.user_id = p.user_id
 WHERE r.thread_parent_id IS NULL
   AND r.reply_count > 0
   AND r.deleted_at IS NULL;
