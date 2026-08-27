-- A mention that reached nobody. Rows live here for the length of the digest
-- window and are deleted when the email goes out, or when the person comes
-- online and no longer needs one.
--
-- In the database rather than in Redis because a worker restart inside the
-- window would otherwise drop the only remaining notification somebody was
-- going to get.
CREATE TABLE pending_mention_emails (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    channel_id   UUID REFERENCES channels(id) ON DELETE CASCADE,
    message_id   UUID,
    sender_name  VARCHAR(200),
    channel_name VARCHAR(200),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- The sweeper reads by "oldest first, grouped by person": this is the index for
-- that, and there is no other query.
CREATE INDEX idx_pending_mention_emails_due
    ON pending_mention_emails(user_id, workspace_id, created_at);

-- Defaults on: somebody who never grants push permission is exactly who this is
-- for, and they are not going to find a switch to turn it on.
ALTER TABLE users ADD COLUMN mention_emails BOOLEAN NOT NULL DEFAULT TRUE;
