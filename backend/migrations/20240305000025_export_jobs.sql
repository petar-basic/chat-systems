-- There was no way to get data out. Not for a legal hold, not for a subject
-- access request, not for leaving the product — `pg_dump` is a backup, not an
-- export: it cannot be scoped to a workspace or a person and is not something
-- you hand to a regulator.
--
-- Jobs, never synchronous: an export of a busy workspace is minutes of work and
-- hundreds of megabytes, which is not a request handler's job.
CREATE TYPE export_scope AS ENUM ('workspace', 'user');
CREATE TYPE export_status AS ENUM ('pending', 'running', 'complete', 'failed');

CREATE TABLE export_jobs (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    scope           export_scope NOT NULL,
    workspace_id    UUID REFERENCES workspaces(id) ON DELETE CASCADE,
    subject_user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    requested_by    UUID NOT NULL REFERENCES users(id),
    -- Private conversations are in a workspace export only when asked for, and
    -- the asking is what gets audited.
    include_dms     BOOLEAN NOT NULL DEFAULT FALSE,
    since           TIMESTAMPTZ,
    until           TIMESTAMPTZ,
    status          export_status NOT NULL DEFAULT 'pending',
    storage_key     TEXT,
    manifest        JSONB,
    error           TEXT,
    -- Single-use: consumed on the first successful download.
    download_token  TEXT UNIQUE,
    token_expires_at TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at    TIMESTAMPTZ,
    CONSTRAINT export_jobs_scope_target CHECK (
        (scope = 'workspace' AND workspace_id IS NOT NULL)
        OR (scope = 'user' AND subject_user_id IS NOT NULL)
    )
);

CREATE INDEX idx_export_jobs_pending ON export_jobs(created_at) WHERE status = 'pending';
CREATE INDEX idx_export_jobs_requester ON export_jobs(requested_by, created_at DESC);
