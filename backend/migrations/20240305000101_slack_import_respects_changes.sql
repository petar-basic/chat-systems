ALTER TABLE slack_channels ADD COLUMN created_by_run UUID REFERENCES slack_imports(id) ON DELETE SET NULL;
ALTER TABLE slack_channels DROP COLUMN created_by_import;

ALTER TABLE slack_imports ADD COLUMN archive_deleted_at TIMESTAMPTZ;

UPDATE slack_imports
   SET storage_key = NULL, archive_deleted_at = COALESCE(finished_at, NOW())
 WHERE status = 'complete' AND storage_key IS NOT NULL;

CREATE INDEX slack_imports_kept_archives ON slack_imports (finished_at)
 WHERE storage_key IS NOT NULL;

CREATE TABLE analytics_uncounted_imports (
    message_id UUID PRIMARY KEY REFERENCES messages(id) ON DELETE CASCADE
);
