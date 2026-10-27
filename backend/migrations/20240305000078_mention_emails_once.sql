DELETE FROM pending_mention_emails p
 USING pending_mention_emails q
 WHERE p.user_id = q.user_id
   AND p.message_id = q.message_id
   AND p.id > q.id;

CREATE UNIQUE INDEX idx_pending_mention_emails_once
    ON pending_mention_emails(user_id, message_id);
