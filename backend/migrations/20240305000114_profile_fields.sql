CREATE TYPE profile_field_kind AS ENUM ('text', 'link', 'date', 'options', 'person');

CREATE TABLE profile_fields (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    label        TEXT NOT NULL,
    hint         TEXT,
    kind         profile_field_kind NOT NULL,
    options      JSONB NOT NULL DEFAULT '[]'::jsonb,
    admins_only  BOOLEAN NOT NULL DEFAULT FALSE,
    on_card      BOOLEAN NOT NULL DEFAULT FALSE,
    position     INT NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (workspace_id, id)
);

CREATE UNIQUE INDEX profile_fields_label ON profile_fields (workspace_id, lower(label));
CREATE INDEX profile_fields_position ON profile_fields (workspace_id, position);

CREATE TABLE profile_field_values (
    workspace_id UUID NOT NULL,
    user_id      UUID NOT NULL,
    field_id     UUID NOT NULL,
    value        TEXT NOT NULL,
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (workspace_id, user_id, field_id),
    FOREIGN KEY (workspace_id, field_id) REFERENCES profile_fields (workspace_id, id) ON DELETE CASCADE,
    FOREIGN KEY (workspace_id, user_id) REFERENCES workspace_members (workspace_id, user_id) ON DELETE CASCADE
);

CREATE INDEX profile_field_values_field ON profile_field_values (field_id, value);
CREATE INDEX profile_field_values_user ON profile_field_values (user_id);
