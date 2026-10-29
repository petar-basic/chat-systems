ALTER TABLE bot_apps
    ADD COLUMN events                   TEXT[] NOT NULL DEFAULT '{}',
    ADD COLUMN request_url              VARCHAR(2048),
    ADD COLUMN socket_mode              BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN app_token_hash           VARCHAR(64) UNIQUE,
    ADD COLUMN signing_secret_encrypted BYTEA,
    ADD COLUMN signing_secret_nonce     BYTEA,
    ADD COLUMN signing_secret_key_id    VARCHAR(64),
    ADD COLUMN failing_since            TIMESTAMPTZ,
    ADD COLUMN last_failure             TEXT;

CREATE TABLE app_deliveries (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    seq             BIGSERIAL NOT NULL,
    app_id          UUID NOT NULL REFERENCES bot_apps(id) ON DELETE CASCADE,
    event_id        UUID NOT NULL,
    channel_id      UUID NOT NULL,
    kind            VARCHAR(20) NOT NULL,
    body            JSONB NOT NULL,
    attempts        INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    leased_until    TIMESTAMPTZ,
    last_error      TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at      TIMESTAMPTZ NOT NULL,
    delivered_at    TIMESTAMPTZ,
    failed_at       TIMESTAMPTZ,
    UNIQUE (app_id, event_id)
);

CREATE INDEX app_deliveries_pending ON app_deliveries (app_id, channel_id, seq)
    WHERE delivered_at IS NULL AND failed_at IS NULL;
CREATE INDEX app_deliveries_settled ON app_deliveries (created_at)
    WHERE delivered_at IS NOT NULL OR failed_at IS NOT NULL;

CREATE TABLE app_sockets (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    app_id       UUID NOT NULL REFERENCES bot_apps(id) ON DELETE CASCADE,
    ticket_hash  VARCHAR(64) UNIQUE,
    opened_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    connected_at TIMESTAMPTZ,
    seen_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX app_sockets_app ON app_sockets (app_id);
