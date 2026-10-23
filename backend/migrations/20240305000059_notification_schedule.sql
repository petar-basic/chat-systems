CREATE TABLE notification_schedules (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    days SMALLINT[] NOT NULL,
    start_minute SMALLINT NOT NULL CHECK (start_minute BETWEEN 0 AND 1439),
    end_minute SMALLINT NOT NULL CHECK (end_minute BETWEEN 0 AND 1439),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (start_minute <> end_minute),
    CHECK (cardinality(days) BETWEEN 1 AND 7 AND days <@ ARRAY[1, 2, 3, 4, 5, 6, 7]::SMALLINT[])
);

ALTER TABLE users ADD COLUMN dnd_override_until TIMESTAMPTZ;

CREATE FUNCTION public.schedule_paused_until(uid UUID, at TIMESTAMPTZ) RETURNS TIMESTAMPTZ
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
    SELECT * INTO s FROM public.notification_schedules WHERE user_id = uid AND enabled;
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

CREATE FUNCTION public.notifications_paused_until(uid UUID, at TIMESTAMPTZ) RETURNS TIMESTAMPTZ
    LANGUAGE sql STABLE AS
$$
SELECT GREATEST(
           CASE WHEN u.dnd_until > at THEN u.dnd_until END,
           CASE WHEN u.dnd_override_until > at THEN NULL ELSE public.schedule_paused_until(u.id, at) END
       )
  FROM public.users u
 WHERE u.id = uid
$$;
