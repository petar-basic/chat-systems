ALTER TYPE notification_type ADD VALUE 'list';

ALTER TABLE list_items ADD COLUMN comment_count INTEGER NOT NULL DEFAULT 0;

CREATE TABLE list_item_comments (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    item_id     UUID NOT NULL REFERENCES list_items(id) ON DELETE CASCADE,
    list_id     UUID NOT NULL REFERENCES lists(id) ON DELETE CASCADE,
    user_id     UUID NOT NULL REFERENCES users(id),
    content     TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    edited_at   TIMESTAMPTZ
);

CREATE INDEX idx_list_item_comments_item ON list_item_comments (item_id, created_at, id);
CREATE INDEX idx_list_item_comments_list ON list_item_comments (list_id, created_at);

ALTER TABLE list_columns ADD COLUMN is_due BOOLEAN NOT NULL DEFAULT false;
CREATE UNIQUE INDEX list_columns_one_due ON list_columns (list_id) WHERE is_due;

ALTER TABLE later_items ADD COLUMN list_item_id UUID REFERENCES list_items(id) ON DELETE CASCADE;
CREATE UNIQUE INDEX later_items_one_per_list_item ON later_items (user_id, list_item_id)
    WHERE list_item_id IS NOT NULL;

CREATE FUNCTION public.list_due_assignments(only_list UUID DEFAULT NULL) RETURNS TABLE (
    workspace_id UUID,
    channel_id   UUID,
    item_id      UUID,
    user_id      UUID,
    title        TEXT,
    due          DATE,
    today        DATE,
    remind_at    TIMESTAMPTZ
)
    LANGUAGE sql STABLE AS
$$
    SELECT l.workspace_id, l.channel_id, i.id, u.id,
           COALESCE(NULLIF(btrim(i."values" ->> t.id::text), ''), 'Untitled'),
           d.due,
           (NOW() AT TIME ZONE z.tz)::DATE,
           (d.due + TIME '09:00') AT TIME ZONE z.tz
      FROM public.list_columns dc
      JOIN public.lists l ON l.id = dc.list_id AND l.deleted_at IS NULL
      JOIN public.list_items i ON i.list_id = l.id AND i.deleted_at IS NULL
      CROSS JOIN LATERAL (SELECT (i."values" ->> dc.id::text)::DATE AS due) d
      JOIN public.list_columns pc ON pc.list_id = l.id AND pc.kind = 'person'
      JOIN public.users u ON u.id = (i."values" ->> pc.id::text)::UUID
      CROSS JOIN LATERAL (SELECT COALESCE(NULLIF(u.timezone, ''), 'UTC') AS tz) z
      LEFT JOIN LATERAL (
          SELECT c.id FROM public.list_columns c
           WHERE c.list_id = l.id AND c.kind = 'text'
           ORDER BY c.position, c.id LIMIT 1
      ) t ON TRUE
      LEFT JOIN LATERAL (
          SELECT c.id FROM public.list_columns c
           WHERE c.list_id = l.id AND c.kind = 'checkbox'
           ORDER BY c.position, c.id LIMIT 1
      ) k ON TRUE
     WHERE dc.is_due
       AND (only_list IS NULL OR dc.list_id = only_list)
       AND d.due IS NOT NULL
       AND d.due <= (NOW() AT TIME ZONE z.tz)::DATE + 1
       AND (k.id IS NULL OR (i."values" ->> k.id::text) IS DISTINCT FROM 'true')
       AND (
           (l.channel_id IS NULL AND l.created_by = u.id)
           OR EXISTS (
               SELECT 1 FROM public.channel_members cm
                WHERE cm.channel_id = l.channel_id AND cm.user_id = u.id
           )
       )
$$;
