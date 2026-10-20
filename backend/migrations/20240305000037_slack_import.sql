-- Slack import (CS-036).
--
-- An import is long, restartable and operator-supervised. What makes a re-run
-- safe is written down rather than held in memory: which Slack id became which
-- row, and which Slack message a row came from.

CREATE TABLE slack_imports (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id    UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    source          VARCHAR(500) NOT NULL,
    dry_run         BOOLEAN NOT NULL DEFAULT FALSE,
    report          JSONB NOT NULL DEFAULT '{}',
    started_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    finished_at     TIMESTAMPTZ
);

CREATE INDEX idx_slack_imports_workspace ON slack_imports(workspace_id, started_at DESC);

-- The mapping is per workspace: the same export can be imported into two
-- workspaces, and a Slack id means nothing outside the workspace it landed in.
CREATE TABLE slack_users (
    workspace_id    UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    slack_user_id   VARCHAR(64) NOT NULL,
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (workspace_id, slack_user_id)
);

CREATE TABLE slack_channels (
    workspace_id     UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    slack_channel_id VARCHAR(64) NOT NULL,
    channel_id       UUID NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (workspace_id, slack_channel_id)
);

-- Direct and group messages are conversations here, not channels, so they need
-- their own mapping: `dms.json` and `mpims.json` in the export.
CREATE TABLE slack_conversations (
    workspace_id     UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    slack_channel_id VARCHAR(64) NOT NULL,
    conversation_id  UUID NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (workspace_id, slack_channel_id)
);

-- Slack's `ts` is unique within a conversation, so it is the natural idempotency
-- key: a second run finds the row and moves on instead of writing it again.
ALTER TABLE messages ADD COLUMN slack_ts VARCHAR(32);
ALTER TABLE conversation_messages ADD COLUMN slack_ts VARCHAR(32);

CREATE UNIQUE INDEX idx_messages_slack_ts ON messages(channel_id, slack_ts)
    WHERE slack_ts IS NOT NULL;

CREATE UNIQUE INDEX idx_conversation_messages_slack_ts
    ON conversation_messages(conversation_id, slack_ts)
    WHERE slack_ts IS NOT NULL;
