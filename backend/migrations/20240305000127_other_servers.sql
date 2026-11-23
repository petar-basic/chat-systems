ALTER TABLE instance_settings
    ADD COLUMN other_servers_allowed BOOLEAN NOT NULL DEFAULT TRUE;
