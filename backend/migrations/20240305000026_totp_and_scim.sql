-- Authentication was email and password with no second factor — not even for
-- the instance admin, who can delete any workspace.
--
-- The secret is stored encrypted rather than in the clear: a database dump
-- containing TOTP secrets is the same failure as one containing passwords.
CREATE TABLE user_totp (
    user_id         UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    secret_encrypted BYTEA NOT NULL,
    nonce           BYTEA NOT NULL,
    confirmed_at    TIMESTAMPTZ,
    -- The last window accepted, so a code cannot be replayed inside its own
    -- validity period.
    last_used_step  BIGINT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Hashed like passwords: a recovery code is a password that bypasses the second
-- factor, so it gets the same treatment.
CREATE TABLE totp_recovery_codes (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    code_hash       VARCHAR(255) NOT NULL,
    used_at         TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_totp_recovery_user ON totp_recovery_codes(user_id) WHERE used_at IS NULL;

-- A machine, not a user: the caller is an identity provider, so it authenticates
-- with its own credential rather than a session.
CREATE TABLE scim_tokens (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    token_hash      VARCHAR(255) NOT NULL UNIQUE,
    description     VARCHAR(200),
    created_by      UUID REFERENCES users(id),
    last_used_at    TIMESTAMPTZ,
    revoked_at      TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Linking happens by verified email only, so the address the provider asserted
-- is worth keeping next to the link.
ALTER TABLE oauth_accounts ADD COLUMN email VARCHAR(255);
