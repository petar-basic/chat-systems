CREATE TYPE list_view_kind AS ENUM ('table', 'board');

CREATE TABLE list_views (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    list_id     UUID NOT NULL REFERENCES lists(id) ON DELETE CASCADE,
    position    INTEGER NOT NULL,
    name        TEXT NOT NULL,
    kind        list_view_kind NOT NULL,
    config      JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_by  UUID NOT NULL REFERENCES users(id),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT list_views_position UNIQUE (list_id, position) DEFERRABLE INITIALLY DEFERRED
);

INSERT INTO list_views (list_id, position, name, kind, created_by)
SELECT id, 0, 'Table', 'table', created_by FROM lists;
