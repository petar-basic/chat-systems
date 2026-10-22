CREATE INDEX files_message_id ON files (message_id, created_at) WHERE message_id IS NOT NULL;
CREATE INDEX files_storage_key ON files (storage_key);
CREATE INDEX files_thumbnail_key ON files (thumbnail_key) WHERE thumbnail_key IS NOT NULL;
