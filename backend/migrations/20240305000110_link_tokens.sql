DROP TABLE password_reset_tokens;

CREATE TABLE password_reset_tokens (
    token_hash  TEXT PRIMARY KEY,
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at  TIMESTAMPTZ NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_password_reset_tokens_user ON password_reset_tokens (user_id);

CREATE TABLE invite_links (
    token_hash  TEXT PRIMARY KEY,
    invite_id   UUID NOT NULL REFERENCES workspace_invites(id) ON DELETE CASCADE,
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_invite_links_invite ON invite_links (invite_id);
CREATE INDEX idx_invite_links_user ON invite_links (user_id);
