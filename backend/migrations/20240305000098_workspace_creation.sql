ALTER TABLE instance_settings
    ADD COLUMN workspace_creation_restricted BOOLEAN NOT NULL DEFAULT FALSE;

ALTER TABLE users
    ADD COLUMN workspace_creator BOOLEAN NOT NULL DEFAULT FALSE;
