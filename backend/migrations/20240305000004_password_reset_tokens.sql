-- Single-use password reset tokens.
-- Each issued reset JWT carries a `jti` that is recorded here on generation and
-- deleted on use, so a reset link cannot be replayed within its 1h validity window.
CREATE TABLE password_reset_tokens (
    jti             UUID PRIMARY KEY,
    user_id         UUID NOT NULL,
    expires_at      TIMESTAMPTZ NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
