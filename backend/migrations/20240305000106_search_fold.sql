DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_catalog.pg_available_extensions WHERE name = 'pg_bigm') THEN
        RAISE EXCEPTION 'pg_bigm is not installed on this database server'
            USING HINT = 'Run the chat-systems-postgres image, or install pg_bigm and add it to shared_preload_libraries: see "Postgres and pg_bigm" in docs/RUNBOOK.md.';
    END IF;
END
$$;

CREATE EXTENSION IF NOT EXISTS pg_bigm;

CREATE FUNCTION public.search_fold(input TEXT) RETURNS TEXT
    LANGUAGE sql IMMUTABLE STRICT PARALLEL SAFE AS
$$ SELECT pg_catalog.lower(pg_catalog.normalize(public.search_normalize(input), 'NFKC')) $$;

CREATE FUNCTION public.search_like_patterns(query TEXT) RETURNS TEXT[]
    LANGUAGE sql IMMUTABLE STRICT PARALLEL SAFE AS
$$ SELECT pg_catalog.array_agg(
              '%' || pg_catalog.replace(pg_catalog.replace(pg_catalog.replace(t, '\', '\\'), '%', '\%'), '_', '\_') || '%'
              ORDER BY pg_catalog.length(t) DESC, t)
     FROM pg_catalog.regexp_split_to_table(public.search_fold(query), '\s+') AS t
    WHERE t <> '' $$;

CREATE FUNCTION public.search_passage(content TEXT, query TEXT) RETURNS TEXT
    LANGUAGE sql IMMUTABLE STRICT PARALLEL SAFE AS
$$ SELECT CASE WHEN h.at > 41 THEN '… ' ELSE '' END
       || pg_catalog.substr(content, GREATEST(1, h.at - 40), LEAST(h.at - 1, 40))
       || pg_catalog.chr(2) || pg_catalog.substr(content, h.at, h.width) || pg_catalog.chr(3)
       || pg_catalog.substr(content, h.at + h.width, 80)
       || CASE WHEN h.at + h.width + 80 <= pg_catalog.length(content) THEN ' …' ELSE '' END
     FROM (
       SELECT pg_catalog.regexp_instr(
                content, pg_catalog.regexp_replace(t, '([.^$*+?()\[\]{}|\\])', '\\\1', 'g'), 1, 1, 0, 'i') AS at,
              pg_catalog.length(t) AS width
         FROM pg_catalog.regexp_split_to_table(pg_catalog.btrim(query), '\s+') AS t
        WHERE t <> ''
     ) h
    WHERE h.at > 0
    ORDER BY h.at
    LIMIT 1 $$;

CREATE OR REPLACE FUNCTION public.keyword_pattern(normalized TEXT) RETURNS TEXT
    LANGUAGE sql IMMUTABLE STRICT PARALLEL SAFE AS
$$ SELECT CASE WHEN normalized ~ ('^' || s.class) THEN '' ELSE '(^|\W|' || s.class || ')' END
       || pg_catalog.regexp_replace(normalized, '([.^$*+?()\[\]{}|\\])', '\\\1', 'g')
       || CASE WHEN normalized ~ (s.class || '$') THEN '' ELSE '(\W|' || s.class || '|$)' END
     FROM (SELECT '[\u0e00-\u0eff\u1000-\u109f\u1780-\u17ff\u3005-\u3007\u3040-\u30ff\u31f0-\u31ff\u3400-\u4dbf\u4e00-\u9fff\uf900-\ufaff\uff66-\uff9f\U00020000-\U0003134f]'::text AS class) s $$;

DELETE FROM notification_keywords k
 USING notification_keywords o
 WHERE o.user_id = k.user_id
   AND o.workspace_id = k.workspace_id
   AND public.search_fold(o.keyword) = public.search_fold(k.keyword)
   AND (o.position, o.normalized) < (k.position, k.normalized);

UPDATE notification_keywords SET normalized = public.search_fold(keyword);
