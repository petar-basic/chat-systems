ALTER TABLE users ALTER COLUMN timezone DROP DEFAULT;
UPDATE users SET timezone = NULL WHERE timezone = 'UTC' OR timezone = '';
