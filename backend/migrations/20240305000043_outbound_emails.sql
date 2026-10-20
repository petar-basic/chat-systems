-- Invites and password resets used to be sent inside the request that asked for
-- them: a slow SMTP server held the response open and a dead one lost the email
-- with a log line. They are queued here instead and delivered by the worker,
-- which retries with backoff and parks the row after the last attempt.
CREATE TABLE outbound_emails (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    to_address      TEXT NOT NULL,
    subject         TEXT NOT NULL,
    text_body       TEXT NOT NULL,
    html_body       TEXT,
    attempts        INT NOT NULL DEFAULT 0,
    next_attempt_at TIMESTAMPTZ DEFAULT NOW(),
    sent_at         TIMESTAMPTZ,
    last_error      TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_outbound_emails_due
    ON outbound_emails (next_attempt_at)
    WHERE sent_at IS NULL AND next_attempt_at IS NOT NULL;
