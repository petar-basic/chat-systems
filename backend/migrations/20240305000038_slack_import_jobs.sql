-- An import you can start from the app rather than from a shell.
--
-- The CLI stays for the exports that are too large to go through a browser, but
-- for the ones that are not, "upload the zip and watch it" is the whole job. The
-- shape is the one the export feature already uses: a row per run, claimed by
-- the worker, with progress written as it goes rather than only at the end.

CREATE TYPE slack_import_status AS ENUM ('pending', 'running', 'complete', 'failed');

ALTER TABLE slack_imports
    ADD COLUMN status       slack_import_status NOT NULL DEFAULT 'complete',
    ADD COLUMN storage_key  TEXT,
    ADD COLUMN requested_by UUID REFERENCES users(id) ON DELETE SET NULL,
    ADD COLUMN error        TEXT;

-- Rows written by the CLI predate the queue and were never pending; only the
-- ones with an uploaded archive are the worker's to claim.
CREATE INDEX idx_slack_imports_pending ON slack_imports(started_at)
    WHERE status = 'pending';
