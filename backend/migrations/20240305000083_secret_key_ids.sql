ALTER TABLE workspace_apps ADD COLUMN client_secret_key_id TEXT;
ALTER TABLE app_connections
    ADD COLUMN access_token_key_id TEXT,
    ADD COLUMN refresh_token_key_id TEXT;
ALTER TABLE user_totp ADD COLUMN key_id TEXT;
