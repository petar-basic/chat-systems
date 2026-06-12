CREATE INDEX idx_hooks_incoming_token ON hooks ((config->>'token'))
WHERE hook_type = 'incoming_webhook' AND is_active = true;
