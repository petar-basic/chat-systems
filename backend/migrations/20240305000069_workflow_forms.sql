ALTER TYPE workflow_run_status ADD VALUE 'waiting';
ALTER TYPE workflow_run_status ADD VALUE 'expired';

CREATE TABLE workflow_forms (
    run_id       UUID NOT NULL REFERENCES workflow_runs(id) ON DELETE CASCADE,
    position     INTEGER NOT NULL,
    recipient_id UUID NOT NULL REFERENCES users(id),
    message_id   UUID NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    fields       JSONB NOT NULL,
    answers      JSONB,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at   TIMESTAMPTZ NOT NULL,
    answered_at  TIMESTAMPTZ,
    PRIMARY KEY (run_id, position)
);

CREATE INDEX idx_workflow_forms_open ON workflow_forms (expires_at) WHERE answered_at IS NULL;
