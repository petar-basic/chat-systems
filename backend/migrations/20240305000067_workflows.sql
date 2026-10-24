DELETE FROM hooks WHERE hook_type IN ('bot', 'scheduled');
DROP INDEX IF EXISTS idx_hooks_incoming_token;
ALTER TYPE hook_type RENAME TO hook_type_old;
CREATE TYPE hook_type AS ENUM ('incoming_webhook', 'outgoing_webhook', 'slash_command');
ALTER TABLE hooks ALTER COLUMN hook_type TYPE hook_type USING hook_type::text::hook_type;
DROP TYPE hook_type_old;
CREATE INDEX idx_hooks_incoming_token ON hooks ((config->>'token'))
WHERE hook_type = 'incoming_webhook' AND is_active = true;

CREATE TABLE workflows (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id  UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name          TEXT NOT NULL,
    description   TEXT,
    created_by    UUID NOT NULL REFERENCES users(id),
    definition    JSONB NOT NULL,
    version       INTEGER NOT NULL DEFAULT 1,
    enabled       BOOLEAN NOT NULL DEFAULT false,
    webhook_token TEXT UNIQUE,
    next_run_at   TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_workflows_workspace ON workflows (workspace_id, created_at);
CREATE INDEX idx_workflows_creator ON workflows (workspace_id, created_by);
CREATE INDEX idx_workflows_due ON workflows (next_run_at) WHERE enabled AND next_run_at IS NOT NULL;

CREATE TYPE workflow_run_status AS ENUM ('pending', 'running', 'succeeded', 'failed');

CREATE TABLE workflow_runs (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workflow_id      UUID NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
    workflow_version INTEGER NOT NULL,
    trigger_key      TEXT NOT NULL,
    status           workflow_run_status NOT NULL DEFAULT 'pending',
    context          JSONB NOT NULL DEFAULT '{}',
    error            TEXT,
    claimed_at       TIMESTAMPTZ,
    started_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    finished_at      TIMESTAMPTZ,
    UNIQUE (workflow_id, trigger_key)
);

CREATE INDEX idx_workflow_runs_recent ON workflow_runs (workflow_id, started_at DESC);
CREATE INDEX idx_workflow_runs_open ON workflow_runs (started_at) WHERE status IN ('pending', 'running');

CREATE TABLE workflow_run_steps (
    run_id      UUID NOT NULL REFERENCES workflow_runs(id) ON DELETE CASCADE,
    position    INTEGER NOT NULL,
    succeeded   BOOLEAN NOT NULL,
    message_id  UUID,
    error       TEXT,
    finished_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (run_id, position)
);

CREATE FUNCTION public.workflow_next_run(days SMALLINT[], at TIME, tz TEXT, after TIMESTAMPTZ)
RETURNS TIMESTAMPTZ
LANGUAGE sql STABLE AS $$
    SELECT MIN(candidate)
      FROM (
          SELECT (((after AT TIME ZONE tz)::date + offs) + at) AT TIME ZONE tz AS candidate,
                 EXTRACT(ISODOW FROM (after AT TIME ZONE tz)::date + offs)::smallint AS dow
            FROM generate_series(0, 7) AS offs
      ) c
     WHERE c.candidate > after AND c.dow = ANY(days)
$$;
