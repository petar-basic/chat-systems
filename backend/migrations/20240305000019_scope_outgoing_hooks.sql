-- Outgoing webhooks used to fire on every message in the workspace. They now
-- carry an explicit `channel_ids` allow-list, and an existing hook has no way to
-- say which channels it was ever meant to see — so every one of them is turned
-- off and has to be re-created with a scope. A silent no-op would look like a
-- working integration that stopped delivering.
UPDATE hooks
   SET is_active = false,
       updated_at = NOW()
 WHERE hook_type = 'outgoing_webhook'
   AND is_active = true;
