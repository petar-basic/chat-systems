ALTER TABLE files ADD COLUMN draft_until TIMESTAMPTZ;
CREATE INDEX files_draft_until ON files (draft_until) WHERE draft_until IS NOT NULL;

ALTER TABLE scheduled_messages ADD COLUMN attachment_ids UUID[] NOT NULL DEFAULT '{}';

CREATE TEMPORARY TABLE legacy_file_links ON COMMIT DROP AS
SELECT m.id AS message_id, link[1] AS url, link[2] AS storage_key
  FROM messages m,
       regexp_matches(m.content, '([^\s()\[\]"]*/api/files/download/([^)\]\s"]+))', 'g') AS link
 WHERE m.content LIKE '%/api/files/download/%';

UPDATE files f
   SET message_id = l.message_id
  FROM legacy_file_links l
 WHERE f.storage_key = l.storage_key
   AND f.message_id IS NULL
   AND f.workspace_id = (SELECT c.workspace_id
                           FROM messages m JOIN channels c ON c.id = m.channel_id
                          WHERE m.id = l.message_id);

DO $$
DECLARE
    owned RECORD;
BEGIN
    FOR owned IN
        SELECT DISTINCT l.message_id, l.url
          FROM legacy_file_links l
          JOIN files f ON f.storage_key = l.storage_key AND f.message_id = l.message_id
    LOOP
        UPDATE messages
           SET content = replace(
                   regexp_replace(content, '\[file:[^\]]*\]\(' || regexp_replace(owned.url, '([.?*+^$()\[\]{}|\\])', '\\\1', 'g') || '\)', '', 'g'),
                   owned.url, '')
         WHERE id = owned.message_id;
    END LOOP;
END $$;

UPDATE messages m
   SET metadata = m.metadata || jsonb_build_object('attachments', a.list),
       content = btrim(m.content, E' \n\t\r')
  FROM (
        SELECT f.message_id,
               jsonb_agg(
                   jsonb_build_object(
                       'id', f.id,
                       'filename', f.filename,
                       'mime_type', f.mime_type,
                       'size_bytes', f.size_bytes,
                       'url', COALESCE(
                           (SELECT l.url FROM legacy_file_links l
                             WHERE l.message_id = f.message_id AND l.storage_key = f.storage_key
                             LIMIT 1),
                           '/api/files/download/' || f.storage_key)
                   )
                   ORDER BY f.created_at, f.id
               ) AS list
          FROM files f
         WHERE f.message_id IS NOT NULL
         GROUP BY f.message_id
       ) a
 WHERE m.id = a.message_id;
