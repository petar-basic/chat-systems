CREATE TABLE workflow_versions (
    workflow_id UUID NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
    version     INTEGER NOT NULL,
    definition  JSONB NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (workflow_id, version)
);

INSERT INTO workflow_versions (workflow_id, version, definition)
SELECT id, version, definition FROM workflows;

ALTER TABLE workflow_runs
    ADD COLUMN waiting_at INTEGER,
    ADD COLUMN resume_at  TIMESTAMPTZ;

UPDATE workflow_runs r
   SET waiting_at = f.position
  FROM workflow_forms f
 WHERE f.run_id = r.id AND r.status = 'waiting' AND f.answered_at IS NULL;

CREATE INDEX idx_workflow_runs_resume ON workflow_runs (resume_at)
    WHERE status = 'waiting' AND resume_at IS NOT NULL;

ALTER TABLE workflow_run_steps
    ADD COLUMN matched    BOOLEAN,
    ADD COLUMN channel_id UUID REFERENCES channels(id) ON DELETE SET NULL;
