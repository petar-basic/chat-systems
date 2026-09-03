-- The ring now reaches the worker through the workspace stream's consumer
-- group, so exactly one replica is handed each one and a redelivery after a
-- crash is the only way the same ring arrives twice. That case is absorbed by
-- the notification row itself: one per person per call.
DELETE FROM notifications a
 USING notifications b
 WHERE a.data ? 'huddle_id'
   AND b.data ? 'huddle_id'
   AND a.user_id = b.user_id
   AND a.notification_type = b.notification_type
   AND a.data ->> 'huddle_id' = b.data ->> 'huddle_id'
   AND a.ctid > b.ctid;

CREATE UNIQUE INDEX idx_notifications_call_dedup
    ON notifications (user_id, notification_type, (data ->> 'huddle_id'))
    WHERE data ? 'huddle_id';

DROP TABLE huddle_ring_claims;
