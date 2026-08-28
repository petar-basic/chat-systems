WITH ranked AS (
    SELECT
        id,
        name,
        ROW_NUMBER() OVER (
            PARTITION BY workspace_id, lower(name)
            ORDER BY created_at, id
        ) AS position
    FROM channels
    WHERE is_archived IS NOT TRUE
      AND name IS NOT NULL
)
UPDATE channels AS c
SET name = left(r.name || '-' || r.position::text, 100)
FROM ranked AS r
WHERE c.id = r.id
  AND r.position > 1;

CREATE UNIQUE INDEX idx_channels_workspace_name_unique
    ON channels (workspace_id, lower(name))
    WHERE is_archived IS NOT TRUE AND name IS NOT NULL;
