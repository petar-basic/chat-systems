CREATE INDEX idx_workflows_listening ON workflows ((definition #>> '{trigger,channel_id}')) WHERE enabled;
