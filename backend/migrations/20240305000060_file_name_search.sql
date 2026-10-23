ALTER TABLE files
    ADD COLUMN search_name TEXT GENERATED ALWAYS AS (public.search_normalize(filename)) STORED;

CREATE INDEX idx_files_search_name_trgm ON files USING GIN (search_name gin_trgm_ops);

DROP INDEX IF EXISTS idx_files_workspace;
CREATE INDEX idx_files_workspace_created ON files (workspace_id, created_at DESC);

CREATE TYPE file_kind AS ENUM (
    'pdf', 'image', 'video', 'audio', 'spreadsheet', 'presentation',
    'document', 'archive', 'text', 'other'
);

CREATE FUNCTION public.file_kind_of(mime TEXT, filename TEXT) RETURNS public.file_kind
    LANGUAGE sql IMMUTABLE PARALLEL SAFE AS
$$
SELECT CASE
    WHEN m = 'application/pdf' OR e = 'pdf' THEN 'pdf'
    WHEN m LIKE 'image/%' THEN 'image'
    WHEN m LIKE 'video/%' THEN 'video'
    WHEN m LIKE 'audio/%' THEN 'audio'
    WHEN strpos(m, 'spreadsheet') > 0 OR strpos(m, 'excel') > 0
         OR e IN ('xls', 'xlsx', 'csv', 'ods', 'numbers') THEN 'spreadsheet'
    WHEN strpos(m, 'presentation') > 0 OR strpos(m, 'powerpoint') > 0
         OR e IN ('ppt', 'pptx', 'odp', 'key') THEN 'presentation'
    WHEN strpos(m, 'word') > 0 OR strpos(m, 'opendocument.text') > 0
         OR e IN ('doc', 'docx', 'odt', 'rtf', 'pages') THEN 'document'
    WHEN strpos(m, 'zip') > 0 OR strpos(m, 'compressed') > 0 OR strpos(m, 'tar') > 0
         OR e IN ('zip', 'rar', '7z', 'gz', 'tgz', 'tar') THEN 'archive'
    WHEN m LIKE 'text/%' OR strpos(m, 'json') > 0 OR strpos(m, 'xml') > 0
         OR e IN ('js', 'ts', 'tsx', 'rs', 'py', 'go', 'java', 'rb', 'sh', 'sql',
                  'yml', 'yaml', 'toml', 'md', 'log') THEN 'text'
    ELSE 'other'
END::public.file_kind
FROM (
    SELECT lower(mime) AS m,
           CASE WHEN strpos(filename, '.') > 0
                THEN lower(regexp_replace(filename, '^.*\.', ''))
                ELSE ''
           END AS e
) parts
$$;

CREATE FUNCTION public.searchable_channels(
    p_workspace UUID,
    p_requester UUID,
    p_only UUID,
    p_is_guest BOOLEAN,
    p_conversations BOOLEAN,
    p_channels BOOLEAN
) RETURNS SETOF UUID
    LANGUAGE sql STABLE PARALLEL SAFE AS
$$
SELECT c.id
  FROM public.channels c
 WHERE c.workspace_id = p_workspace
   AND (p_only IS NULL OR c.id = p_only)
   AND (
       EXISTS (
           SELECT 1 FROM public.channel_members cm
            WHERE cm.channel_id = c.id AND cm.user_id = p_requester
       )
       OR (NOT p_is_guest AND c.channel_type = 'public')
   )
   AND CASE WHEN c.channel_type IN ('dm', 'group_dm') THEN p_conversations ELSE p_channels END
$$;
