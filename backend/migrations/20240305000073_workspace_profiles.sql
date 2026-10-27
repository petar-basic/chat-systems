ALTER TABLE workspace_members
    ADD COLUMN display_name      VARCHAR(100),
    ADD COLUMN avatar_url        VARCHAR(500),
    ADD COLUMN bio               TEXT,
    ADD COLUMN status_emoji      VARCHAR(50),
    ADD COLUMN status_text       VARCHAR(100),
    ADD COLUMN status_expires_at TIMESTAMPTZ;

UPDATE workspace_members wm
   SET status_emoji = CASE
           WHEN u.status_emoji NOT LIKE ':%:'
             OR EXISTS (SELECT 1 FROM workspace_emojis e
                         WHERE e.workspace_id = wm.workspace_id
                           AND ':' || e.name || ':' = u.status_emoji)
           THEN u.status_emoji
       END,
       status_text = u.status_text,
       status_expires_at = u.status_expires_at
  FROM users u
 WHERE u.id = wm.user_id
   AND (u.status_emoji IS NOT NULL OR u.status_text IS NOT NULL)
   AND (u.status_expires_at IS NULL OR u.status_expires_at > NOW());

ALTER TABLE users
    DROP COLUMN status_emoji,
    DROP COLUMN status_text,
    DROP COLUMN status_expires_at;

DROP TABLE user_preferences;
DROP TABLE bookmarked_messages;
