ALTER TABLE instance_settings
    ALTER COLUMN workspace_creation_restricted SET DEFAULT TRUE;

UPDATE instance_settings SET workspace_creation_restricted = TRUE;
