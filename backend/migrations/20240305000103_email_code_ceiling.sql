ALTER TABLE email_sign_in_codes
    ADD COLUMN failures INT NOT NULL DEFAULT 0,
    ADD COLUMN failures_since TIMESTAMPTZ;
