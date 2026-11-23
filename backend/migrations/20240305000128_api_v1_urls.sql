UPDATE users
   SET avatar_url = replace(avatar_url, '/api/files/download/', '/api/v1/files/download/')
 WHERE avatar_url LIKE '%/api/files/download/%';

UPDATE workspace_members
   SET avatar_url = replace(avatar_url, '/api/files/download/', '/api/v1/files/download/')
 WHERE avatar_url LIKE '%/api/files/download/%';

UPDATE workspace_departures
   SET avatar_url = replace(avatar_url, '/api/files/download/', '/api/v1/files/download/')
 WHERE avatar_url LIKE '%/api/files/download/%';

UPDATE workspaces
   SET icon_url = replace(icon_url, '/api/files/download/', '/api/v1/files/download/')
 WHERE icon_url LIKE '%/api/files/download/%';

UPDATE messages
   SET metadata = replace(metadata::text, '/api/files/download/', '/api/v1/files/download/')::jsonb
 WHERE metadata::text LIKE '%/api/files/download/%';

UPDATE outbound_emails
   SET text_body = replace(replace(text_body, '/api/email/unsubscribe/', '/api/v1/email/unsubscribe/'),
                           '/api/exports/download/', '/api/v1/exports/download/'),
       html_body = replace(replace(html_body, '/api/email/unsubscribe/', '/api/v1/email/unsubscribe/'),
                           '/api/exports/download/', '/api/v1/exports/download/'),
       list_unsubscribe = replace(list_unsubscribe, '/api/email/unsubscribe/', '/api/v1/email/unsubscribe/')
 WHERE sent_at IS NULL;
