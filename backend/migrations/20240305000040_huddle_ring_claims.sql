-- A ring is ephemeral, so it stays on pub/sub — which delivers to every
-- subscriber. With more than one worker replica that means the same person is
-- rung twice. The claim is what makes the duplicate harmless: whichever replica
-- inserts the row first sends, the others lose the race and do nothing.
--
-- The same shape as `hook_executions`' `(hook_id, event_id)` claim from CS-028,
-- for the same reason.
CREATE TABLE huddle_ring_claims (
    huddle_id  UUID NOT NULL,
    to_user_id UUID NOT NULL,
    claimed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (huddle_id, to_user_id)
);

-- Rows are worthless once the call is over; retention sweeps them with the rest
-- of the short-lived tables.
CREATE INDEX idx_huddle_ring_claims_claimed_at ON huddle_ring_claims(claimed_at);
