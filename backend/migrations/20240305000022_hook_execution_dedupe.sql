-- Worker consumers read the event log through a consumer group now, which
-- redelivers anything that was not acknowledged — so the same event can reach a
-- worker twice after one dies mid-dispatch. Calling somebody's webhook twice is
-- not a harmless retry, so the pair is claimed before the request goes out and
-- the unique index is what makes the claim mean something.
ALTER TABLE hook_executions ADD COLUMN event_id UUID;

CREATE UNIQUE INDEX idx_hook_executions_event
    ON hook_executions(hook_id, event_id)
    WHERE event_id IS NOT NULL;
