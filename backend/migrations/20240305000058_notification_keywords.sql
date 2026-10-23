ALTER TYPE notification_type ADD VALUE 'keyword';

CREATE TABLE notification_keywords (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    keyword TEXT NOT NULL,
    normalized TEXT NOT NULL,
    position INT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, workspace_id, normalized)
);

CREATE INDEX idx_notification_keywords_workspace ON notification_keywords (workspace_id, user_id);

CREATE FUNCTION public.keyword_pattern(normalized TEXT) RETURNS TEXT
    LANGUAGE sql IMMUTABLE STRICT PARALLEL SAFE AS
$$ SELECT '(^|\W)' || pg_catalog.regexp_replace(normalized, '([.^$*+?()\[\]{}|\\])', '\\\1', 'g') || '(\W|$)' $$;
