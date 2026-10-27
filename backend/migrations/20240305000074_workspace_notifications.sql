ALTER TABLE workspace_members
    ADD COLUMN dnd_until          TIMESTAMPTZ,
    ADD COLUMN dnd_override_until TIMESTAMPTZ,
    ADD COLUMN mention_emails     BOOLEAN NOT NULL DEFAULT TRUE;

UPDATE workspace_members wm
   SET dnd_until = CASE WHEN u.dnd_until > NOW() THEN u.dnd_until END,
       dnd_override_until = CASE WHEN u.dnd_override_until > NOW() THEN u.dnd_override_until END,
       mention_emails = u.mention_emails
  FROM users u
 WHERE u.id = wm.user_id;

DROP FUNCTION public.notifications_paused_until(UUID, TIMESTAMPTZ);
DROP FUNCTION public.schedule_paused_until(UUID, TIMESTAMPTZ);

CREATE TEMP TABLE account_notification_schedules AS SELECT * FROM notification_schedules;
DROP TABLE notification_schedules;

CREATE TABLE notification_schedules (
    workspace_id UUID NOT NULL,
    user_id UUID NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    days SMALLINT[] NOT NULL,
    start_minute SMALLINT NOT NULL CHECK (start_minute BETWEEN 0 AND 1439),
    end_minute SMALLINT NOT NULL CHECK (end_minute BETWEEN 0 AND 1439),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (workspace_id, user_id),
    FOREIGN KEY (workspace_id, user_id)
        REFERENCES workspace_members(workspace_id, user_id) ON DELETE CASCADE,
    CHECK (start_minute <> end_minute),
    CHECK (cardinality(days) BETWEEN 1 AND 7 AND days <@ ARRAY[1, 2, 3, 4, 5, 6, 7]::SMALLINT[])
);

INSERT INTO notification_schedules (workspace_id, user_id, enabled, days, start_minute, end_minute, updated_at)
SELECT wm.workspace_id, s.user_id, s.enabled, s.days, s.start_minute, s.end_minute, s.updated_at
  FROM account_notification_schedules s
  JOIN workspace_members wm ON wm.user_id = s.user_id;

DROP TABLE account_notification_schedules;

ALTER TABLE users
    DROP COLUMN dnd_until,
    DROP COLUMN dnd_override_until,
    DROP COLUMN mention_emails;

CREATE FUNCTION public.schedule_paused_until(ws UUID, uid UUID, at TIMESTAMPTZ) RETURNS TIMESTAMPTZ
    LANGUAGE plpgsql STABLE AS
$$
DECLARE
    s public.notification_schedules;
    tz TEXT;
    here TIMESTAMP;
    day DATE;
    opens TIMESTAMP;
    closes TIMESTAMP;
BEGIN
    SELECT * INTO s FROM public.notification_schedules
     WHERE workspace_id = ws AND user_id = uid AND enabled;
    IF NOT FOUND THEN
        RETURN NULL;
    END IF;
    SELECT COALESCE(NULLIF(timezone, ''), 'UTC') INTO tz FROM public.users WHERE id = uid;
    here := at AT TIME ZONE tz;

    FOR back IN 0..1 LOOP
        day := here::DATE - back;
        IF EXTRACT(ISODOW FROM day)::SMALLINT = ANY (s.days) THEN
            opens := day + make_interval(mins => s.start_minute);
            closes := day + make_interval(mins => s.end_minute);
            IF closes <= opens THEN
                closes := closes + INTERVAL '1 day';
            END IF;
            IF here >= opens AND here < closes THEN
                RETURN NULL;
            END IF;
        END IF;
    END LOOP;

    FOR ahead IN 0..7 LOOP
        day := here::DATE + ahead;
        IF EXTRACT(ISODOW FROM day)::SMALLINT = ANY (s.days) THEN
            opens := day + make_interval(mins => s.start_minute);
            IF opens > here THEN
                RETURN opens AT TIME ZONE tz;
            END IF;
        END IF;
    END LOOP;
    RETURN NULL;
END
$$;

CREATE FUNCTION public.notifications_paused_until(ws UUID, uid UUID, at TIMESTAMPTZ) RETURNS TIMESTAMPTZ
    LANGUAGE sql STABLE AS
$$
SELECT GREATEST(
           CASE WHEN wm.dnd_until > at THEN wm.dnd_until END,
           CASE WHEN wm.dnd_override_until > at THEN NULL
                ELSE public.schedule_paused_until(ws, uid, at) END
       )
  FROM public.workspace_members wm
 WHERE wm.workspace_id = ws AND wm.user_id = uid
$$;
