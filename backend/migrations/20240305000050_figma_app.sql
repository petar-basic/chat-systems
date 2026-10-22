ALTER TABLE workspace_apps DROP CONSTRAINT workspace_apps_provider_check;
ALTER TABLE workspace_apps ADD CONSTRAINT workspace_apps_provider_check
    CHECK (provider IN ('jira', 'github', 'google_drive', 'figma'));
