CREATE TYPE notify_level AS ENUM ('all', 'mentions', 'nothing');

ALTER TABLE workspace_members
    ADD COLUMN notify_default notify_level NOT NULL DEFAULT 'mentions';

ALTER TABLE channel_members
    DROP COLUMN notifications,
    DROP COLUMN is_muted,
    ADD COLUMN notify_level notify_level;

ALTER TABLE pending_mention_emails
    ADD COLUMN direct BOOLEAN NOT NULL DEFAULT false;

CREATE FUNCTION public.effective_notify_level(
    stored public.notify_level,
    workspace_default public.notify_level,
    kind public.channel_type
) RETURNS public.notify_level
LANGUAGE sql IMMUTABLE AS $$
    SELECT COALESCE(
        stored,
        CASE WHEN kind IN ('dm', 'group_dm') THEN 'all'::public.notify_level ELSE workspace_default END
    )
$$;
