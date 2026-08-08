-- Invites were created with no expiry and no use limit, and `max_uses IS NULL`
-- means unlimited. Every link ever issued was therefore a permanent, unlimited
-- key to the workspace at whatever role it carried.
--
-- BREAKING: this expires every outstanding invite. Admins must re-send. That is
-- deliberate — there is no way to tell a legitimate outstanding link from a
-- leaked one, and the whole point of the change is that neither should work.

ALTER TABLE workspace_invites
    ALTER COLUMN expires_at SET DEFAULT NOW() + INTERVAL '7 days';

UPDATE workspace_invites
   SET expires_at = COALESCE(expires_at, created_at),
       max_uses = COALESCE(max_uses, 1)
 WHERE expires_at IS NULL
    OR max_uses IS NULL;

CREATE INDEX idx_workspace_invites_expiry ON workspace_invites (workspace_id, expires_at);
