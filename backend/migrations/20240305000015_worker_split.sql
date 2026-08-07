-- Background consumers move out of chat-api into a single chat-worker process.
-- Belt and braces: a worker restart mid-batch can still redeliver, so make the
-- duplicate impossible at the schema level rather than merely unlikely.

DELETE FROM notifications a
USING notifications b
WHERE a.data ? 'message_id'
  AND b.data ? 'message_id'
  AND a.user_id = b.user_id
  AND a.notification_type = b.notification_type
  AND a.data ->> 'message_id' = b.data ->> 'message_id'
  AND a.ctid > b.ctid;

CREATE UNIQUE INDEX idx_notifications_dedup
    ON notifications (user_id, notification_type, (data ->> 'message_id'))
    WHERE data ? 'message_id';

-- Reminders were read with a plain SELECT and marked delivered afterwards, so
-- two readers delivered the same reminder twice. Claim them the way scheduled
-- messages already do.
CREATE INDEX idx_reminders_claim ON reminders (remind_at, id) WHERE is_delivered = FALSE;
