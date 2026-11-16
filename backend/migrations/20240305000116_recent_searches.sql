CREATE TYPE search_tab AS ENUM ('messages', 'files');

CREATE TABLE recent_searches (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id  UUID NOT NULL,
    user_id       UUID NOT NULL,
    query         TEXT NOT NULL,
    tab           search_tab NOT NULL,
    filters       JSONB NOT NULL DEFAULT '{}'::jsonb,
    searched_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    FOREIGN KEY (workspace_id, user_id) REFERENCES workspace_members (workspace_id, user_id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX recent_searches_once ON recent_searches (user_id, workspace_id, lower(query), tab, filters);

CREATE INDEX recent_searches_newest ON recent_searches (user_id, workspace_id, searched_at DESC, id DESC);

CREATE VIEW recent_searches_seen AS
SELECT rs.id, rs.workspace_id, rs.user_id, rs.query, rs.tab,
       CASE WHEN rs.filters ? 'in' AND NOT EXISTS (
                SELECT 1
                  FROM channel_members cm
                  JOIN channels c ON c.id = cm.channel_id
                 WHERE cm.channel_id = (rs.filters->>'in')::uuid
                   AND cm.user_id = rs.user_id
                   AND c.workspace_id = rs.workspace_id
            )
            THEN rs.filters - 'in'
            ELSE rs.filters
       END AS filters,
       rs.searched_at
  FROM recent_searches rs;
