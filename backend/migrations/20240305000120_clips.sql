ALTER TABLE files ADD COLUMN clip JSONB CHECK (clip IS NULL OR jsonb_typeof(clip) = 'object');
