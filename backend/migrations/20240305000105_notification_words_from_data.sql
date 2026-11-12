UPDATE notifications
   SET data = data || jsonb_build_object('sender_name', title)
 WHERE notification_type = 'dm';

UPDATE notifications
   SET data = data || jsonb_strip_nulls(jsonb_build_object(
         'keyword', substring(title from '^Matched "(.*)"(?: in #.*)?$'),
         'channel_name', substring(title from '^Matched ".*" in #(.*)$')))
 WHERE notification_type = 'keyword';

UPDATE notifications
   SET data = data || jsonb_strip_nulls(jsonb_build_object(
         'assigned', CASE WHEN title LIKE '% assigned you to %' THEN true END,
         'sender_name', substring(title from '^(.*?) (?:assigned you to|mentioned you on) '),
         'item_title', substring(title from ' (?:assigned you to|mentioned you on) (.*)$'),
         'list_title', CASE WHEN title LIKE '% assigned you to %'
                            THEN substring(body from '^in (.*)$') END)),
       body = CASE WHEN title LIKE '% assigned you to %' THEN NULL ELSE body END
 WHERE notification_type = 'list';

UPDATE notifications
   SET body = NULL
 WHERE notification_type = 'call';

UPDATE notifications
   SET data = data || jsonb_strip_nulls(jsonb_build_object(
         'notice', 'workflow_stopped',
         'workflow_name', substring(body from '^“(.*)” stopped because '),
         'stopped', CASE
                      WHEN body LIKE '%was suspended.' THEN 'inactive'
                      WHEN body LIKE '%left the workspace.' THEN 'left_workspace'
                      WHEN body LIKE '%no longer allowed to.' THEN 'demoted'
                    END)),
       body = NULL
 WHERE notification_type = 'system' AND data ? 'workflow_id';

UPDATE notifications
   SET data = data || jsonb_strip_nulls(jsonb_build_object(
         'notice', 'app_requested',
         'sender_name', substring(title from '^(.*) asked for an app$')))
 WHERE notification_type = 'system' AND data ? 'app_request';

UPDATE notifications
   SET data = data || jsonb_build_object('notice', 'not_delivered'),
       body = NULL
 WHERE notification_type = 'system' AND data ? 'scheduled_message_id';

ALTER TABLE notifications DROP COLUMN title;
