-- Durable events used to be written to Redis after the database transaction
-- had committed, so a Redis error or a crash between the two lost the event
-- while the row it described stayed. The event is now staged here inside the
-- transaction that writes the row, published immediately after commit, and
-- swept by the worker if that immediate publish did not happen.
CREATE TABLE event_outbox (
    id           BIGSERIAL PRIMARY KEY,
    event_id     UUID NOT NULL,
    event_type   TEXT NOT NULL,
    workspace_id UUID NOT NULL,
    payload      JSONB NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    claimed_at   TIMESTAMPTZ,
    published_at TIMESTAMPTZ
);

CREATE INDEX idx_event_outbox_unpublished ON event_outbox (id) WHERE published_at IS NULL;
CREATE INDEX idx_event_outbox_published ON event_outbox (published_at) WHERE published_at IS NOT NULL;
