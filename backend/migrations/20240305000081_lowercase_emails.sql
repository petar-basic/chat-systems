DO $$
DECLARE
    clashes TEXT;
BEGIN
    SELECT string_agg(spellings, '; ')
      INTO clashes
      FROM (
          SELECT string_agg(email || ' (' || id || ')', ', ' ORDER BY created_at) AS spellings
            FROM users
           GROUP BY lower(email)
          HAVING COUNT(*) > 1
      ) same_address;
    IF clashes IS NOT NULL THEN
        RAISE EXCEPTION 'accounts whose addresses differ only in case: %. Merge or rename one of each first, see RUNBOOK "Upgrade note: addresses are lowercase"', clashes;
    END IF;
END $$;

UPDATE users SET email = lower(email) WHERE email <> lower(email);
ALTER TABLE users ADD CONSTRAINT users_email_lowercase CHECK (email = lower(email));

UPDATE workspace_invites SET email = lower(email) WHERE email <> lower(email);
ALTER TABLE workspace_invites ADD CONSTRAINT workspace_invites_email_lowercase CHECK (email = lower(email));
