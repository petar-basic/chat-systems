CREATE TABLE app_workflow_steps (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    app_id      UUID NOT NULL REFERENCES bot_apps(id) ON DELETE CASCADE,
    callback_id VARCHAR(255) NOT NULL,
    title       VARCHAR(80) NOT NULL,
    description VARCHAR(300) NOT NULL DEFAULT '',
    inputs      JSONB NOT NULL DEFAULT '[]',
    outputs     JSONB NOT NULL DEFAULT '[]',
    position    SMALLINT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (app_id, callback_id)
);

CREATE TABLE workflow_app_calls (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_id      UUID NOT NULL REFERENCES workflow_runs(id) ON DELETE CASCADE,
    position    INTEGER NOT NULL,
    app_id      UUID NOT NULL,
    title       VARCHAR(80) NOT NULL,
    inputs      JSONB NOT NULL,
    outputs     JSONB NOT NULL,
    expires_at  TIMESTAMPTZ NOT NULL,
    finished_at TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (run_id, position)
);

CREATE INDEX idx_workflow_app_calls_open ON workflow_app_calls (expires_at)
    WHERE finished_at IS NULL;

ALTER TABLE workflow_run_steps
    ADD COLUMN inputs  JSONB,
    ADD COLUMN outputs JSONB;
