CREATE TABLE workspace_departures (
    workspace_id    UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    display_name    VARCHAR(100),
    avatar_url      VARCHAR(500),
    left_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (workspace_id, user_id)
);

CREATE INDEX idx_workspace_departures_user ON workspace_departures(user_id);

INSERT INTO workspace_departures (workspace_id, user_id)
SELECT c.workspace_id, m.user_id
  FROM messages m
  JOIN channels c ON c.id = m.channel_id
 WHERE NOT EXISTS (
         SELECT 1 FROM workspace_members wm
          WHERE wm.workspace_id = c.workspace_id AND wm.user_id = m.user_id
       )
UNION
SELECT c.workspace_id, d.user_id
  FROM conversation_departures d
  JOIN channels c ON c.id = d.channel_id
 WHERE NOT EXISTS (
         SELECT 1 FROM workspace_members wm
          WHERE wm.workspace_id = c.workspace_id AND wm.user_id = d.user_id
       )
ON CONFLICT DO NOTHING;
