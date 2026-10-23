CREATE TYPE list_column_kind AS ENUM (
    'text', 'number', 'select', 'multi_select', 'person', 'date', 'checkbox', 'link'
);

CREATE TABLE lists (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id  UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    channel_id    UUID REFERENCES channels(id) ON DELETE CASCADE,
    title         TEXT NOT NULL,
    description   TEXT NOT NULL DEFAULT '',
    created_by    UUID NOT NULL REFERENCES users(id),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at    TIMESTAMPTZ
);

CREATE INDEX idx_lists_channel ON lists (channel_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_lists_personal ON lists (workspace_id, created_by)
    WHERE channel_id IS NULL AND deleted_at IS NULL;
CREATE INDEX idx_lists_deleted ON lists (deleted_at) WHERE deleted_at IS NOT NULL;

CREATE TABLE list_columns (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    list_id     UUID NOT NULL REFERENCES lists(id) ON DELETE CASCADE,
    position    INTEGER NOT NULL,
    name        TEXT NOT NULL,
    kind        list_column_kind NOT NULL,
    options     JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT list_columns_position UNIQUE (list_id, position) DEFERRABLE INITIALLY DEFERRED
);

CREATE TABLE list_items (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    list_id     UUID NOT NULL REFERENCES lists(id) ON DELETE CASCADE,
    rank        TEXT COLLATE "C" NOT NULL,
    "values"    JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_by  UUID NOT NULL REFERENCES users(id),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at  TIMESTAMPTZ
);

CREATE INDEX idx_list_items_rank ON list_items (list_id, rank, id) WHERE deleted_at IS NULL;
CREATE INDEX idx_list_items_updated ON list_items (list_id, updated_at);
