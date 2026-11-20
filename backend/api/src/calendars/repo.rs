use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::models::{
    AgendaEvent, Change, ConnectionKey, CurrentEvent, DueAgenda, DueReminder, DueSync,
    MemberStatus, SyncSettings, SyncedEvent, When, KEEP_PAST_HOURS,
};

#[derive(Clone)]
pub struct CalendarRepo {
    pool: PgPool,
}

pub struct CalendarStatus<'a> {
    pub provider: &'a str,
    pub emoji: &'a str,
    pub text: &'a str,
    pub until: DateTime<Utc>,
}

fn split(when: When) -> (Option<DateTime<Utc>>, Option<NaiveDate>) {
    match when {
        When::At(at) => (Some(at), None),
        When::Day(day) => (None, Some(day)),
    }
}

impl CalendarRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn start_sync(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        provider: &str,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            "INSERT INTO calendar_syncs (workspace_id, user_id, provider)
             VALUES ($1, $2, $3)
             ON CONFLICT (workspace_id, user_id, provider) DO UPDATE
                SET sync_token = NULL, window_end = NULL, next_sync_at = NOW(),
                    leased_until = NULL, last_error = NULL",
            workspace_id,
            user_id,
            provider
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn settings(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        provider: &str,
    ) -> sqlx::Result<Option<SyncSettings>> {
        sqlx::query_as!(
            SyncSettings,
            "SELECT status_on, reminders_on, agenda_on, agenda_at, last_synced_at, last_error
               FROM calendar_syncs WHERE workspace_id = $1 AND user_id = $2 AND provider = $3",
            workspace_id,
            user_id,
            provider
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn update_settings(
        &self,
        key: &ConnectionKey,
        status_on: Option<bool>,
        reminders_on: Option<bool>,
        agenda_on: Option<bool>,
        agenda_at: Option<NaiveTime>,
    ) -> sqlx::Result<bool> {
        let result = sqlx::query!(
            "UPDATE calendar_syncs
                SET status_on = COALESCE($4, status_on),
                    reminders_on = COALESCE($5, reminders_on),
                    agenda_on = COALESCE($6, agenda_on),
                    agenda_at = COALESCE($7, agenda_at)
              WHERE workspace_id = $1 AND user_id = $2 AND provider = $3",
            key.workspace_id,
            key.user_id,
            key.provider,
            status_on,
            reminders_on,
            agenda_on,
            agenda_at
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn claim_due(&self, now: DateTime<Utc>, limit: i64) -> sqlx::Result<Vec<DueSync>> {
        sqlx::query_as!(
            DueSync,
            r#"UPDATE calendar_syncs s
                  SET leased_until = $1::timestamptz + interval '2 minutes'
                 FROM (SELECT d.workspace_id, d.user_id, d.provider
                         FROM calendar_syncs d
                         JOIN workspace_apps a
                           ON a.workspace_id = d.workspace_id AND a.provider = d.provider AND a.enabled
                        WHERE d.next_sync_at <= $1
                          AND (d.leased_until IS NULL OR d.leased_until < $1)
                        ORDER BY d.next_sync_at
                        LIMIT $2
                        FOR UPDATE OF d SKIP LOCKED) due,
                      users u
                WHERE s.workspace_id = due.workspace_id AND s.user_id = due.user_id
                  AND s.provider = due.provider AND u.id = s.user_id
            RETURNING s.workspace_id, s.user_id, s.provider, s.sync_token, s.window_end,
                      COALESCE(u.timezone, 'UTC') AS "timezone!""#,
            now,
            limit
        )
        .fetch_all(&self.pool)
        .await
    }

    async fn upsert_in(
        conn: &mut PgConnection,
        key: &ConnectionKey,
        event: &SyncedEvent,
        timezone: &str,
    ) -> sqlx::Result<()> {
        let (starts_at, starts_day) = split(event.starts);
        let (ends_at, ends_day) = split(event.ends);
        sqlx::query!(
            r#"INSERT INTO calendar_events
                   (workspace_id, user_id, provider, event_id, title, starts_at, ends_at, all_day,
                    kind, attending, private, join_url)
               VALUES ($1, $2, $3, $4, $5,
                       COALESCE($6::timestamptz, $7::date::timestamp AT TIME ZONE $10::text),
                       COALESCE($8::timestamptz, $9::date::timestamp AT TIME ZONE $10::text),
                       $6::timestamptz IS NULL, $11, $12, $13, $14)
               ON CONFLICT (workspace_id, user_id, provider, event_id) DO UPDATE
                  SET title = EXCLUDED.title,
                      reminded_at = CASE WHEN calendar_events.starts_at = EXCLUDED.starts_at
                                         THEN calendar_events.reminded_at END,
                      starts_at = EXCLUDED.starts_at,
                      ends_at = EXCLUDED.ends_at,
                      all_day = EXCLUDED.all_day,
                      kind = EXCLUDED.kind,
                      attending = EXCLUDED.attending,
                      private = EXCLUDED.private,
                      join_url = EXCLUDED.join_url"#,
            key.workspace_id,
            key.user_id,
            key.provider,
            event.id,
            event.title,
            starts_at,
            starts_day,
            ends_at,
            ends_day,
            timezone,
            event.kind.as_str(),
            event.attending,
            event.private,
            event.join_url
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    pub async fn apply(
        &self,
        sync: &DueSync,
        changes: &[Change],
        full: bool,
        token: &str,
        window: (DateTime<Utc>, DateTime<Utc>),
        now: DateTime<Utc>,
    ) -> sqlx::Result<()> {
        let key = sync.key();
        let mut tx = self.pool.begin().await?;
        let mut kept = Vec::new();
        for change in changes {
            match change {
                Change::Upsert(event) => {
                    Self::upsert_in(&mut tx, &key, event, &sync.timezone).await?;
                    kept.push(event.id.clone());
                }
                Change::Remove(id) => {
                    sqlx::query!(
                        "DELETE FROM calendar_events
                          WHERE workspace_id = $1 AND user_id = $2 AND provider = $3 AND event_id = $4",
                        key.workspace_id,
                        key.user_id,
                        key.provider,
                        id
                    )
                    .execute(&mut *tx)
                    .await?;
                }
            }
        }
        if full {
            sqlx::query!(
                "DELETE FROM calendar_events
                  WHERE workspace_id = $1 AND user_id = $2 AND provider = $3
                    AND event_id <> ALL($4)",
                key.workspace_id,
                key.user_id,
                key.provider,
                &kept
            )
            .execute(&mut *tx)
            .await?;
        }
        sqlx::query!(
            "DELETE FROM calendar_events
              WHERE workspace_id = $1 AND user_id = $2 AND provider = $3
                AND (starts_at >= $4 OR ends_at <= $5::timestamptz - make_interval(hours => $6))",
            key.workspace_id,
            key.user_id,
            key.provider,
            window.1,
            now,
            KEEP_PAST_HOURS as i32
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "UPDATE calendar_syncs
                SET sync_token = $4, window_end = $5, last_synced_at = $6, last_error = NULL,
                    next_sync_at = $6::timestamptz + make_interval(secs => $7), leased_until = NULL
              WHERE workspace_id = $1 AND user_id = $2 AND provider = $3",
            key.workspace_id,
            key.user_id,
            key.provider,
            token,
            window.1,
            now,
            super::models::SYNC_EVERY_SECS as f64
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await
    }

    pub async fn sync_failed(
        &self,
        key: &ConnectionKey,
        error: &str,
        now: DateTime<Utc>,
        forget_token: bool,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            "UPDATE calendar_syncs
                SET last_error = $4, leased_until = NULL,
                    sync_token = CASE WHEN $6 THEN NULL ELSE sync_token END,
                    window_end = CASE WHEN $6 THEN NULL ELSE window_end END,
                    next_sync_at = $5::timestamptz + make_interval(secs => $7)
              WHERE workspace_id = $1 AND user_id = $2 AND provider = $3",
            key.workspace_id,
            key.user_id,
            key.provider,
            error,
            now,
            forget_token,
            super::models::SYNC_EVERY_SECS as f64
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn purge_past(&self, now: DateTime<Utc>) -> sqlx::Result<u64> {
        let result = sqlx::query!(
            "DELETE FROM calendar_events WHERE ends_at < $1::timestamptz - make_interval(hours => $2)",
            now,
            KEEP_PAST_HOURS as i32
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn current_events(&self, now: DateTime<Utc>) -> sqlx::Result<Vec<CurrentEvent>> {
        sqlx::query_as!(
            CurrentEvent,
            r#"SELECT e.workspace_id, e.user_id, e.provider, e.event_id, e.kind, e.private, e.ends_at,
                      s.status_event_id
                 FROM calendar_events e
                 JOIN calendar_syncs s USING (workspace_id, user_id, provider)
                 JOIN workspace_apps a
                   ON a.workspace_id = e.workspace_id AND a.provider = e.provider AND a.enabled
                WHERE s.status_on AND e.starts_at <= $1 AND e.ends_at > $1
                  AND (e.kind = 'out_of_office'
                       OR (e.kind = 'meeting' AND e.attending AND NOT e.all_day))"#,
            now
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn member_statuses(
        &self,
        workspace_ids: &[Uuid],
        user_ids: &[Uuid],
        now: DateTime<Utc>,
    ) -> sqlx::Result<Vec<MemberStatus>> {
        sqlx::query_as!(
            MemberStatus,
            r#"SELECT m.workspace_id, m.user_id,
                      CASE WHEN m.status_expires_at IS NULL OR m.status_expires_at > $3
                           THEN m.status_emoji END AS "status_emoji?",
                      CASE WHEN m.status_expires_at IS NULL OR m.status_expires_at > $3
                           THEN m.status_text END AS "status_text?",
                      m.status_expires_at,
                      m.status_source,
                      u.preferences->>'language' AS language
                 FROM workspace_members m
                 JOIN users u ON u.id = m.user_id
                WHERE (m.workspace_id, m.user_id) IN (SELECT * FROM UNNEST($1::uuid[], $2::uuid[]))
                   OR (m.status_source IS NOT NULL
                       AND (m.status_expires_at IS NULL OR m.status_expires_at > $3))"#,
            workspace_ids,
            user_ids,
            now
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn set_status(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        status: &CalendarStatus<'_>,
        event_id: &str,
        now: DateTime<Utc>,
    ) -> sqlx::Result<bool> {
        let mut tx = self.pool.begin().await?;
        let set = sqlx::query!(
            "UPDATE workspace_members
                SET status_emoji = $3, status_text = $4, status_expires_at = $5, status_source = $6
              WHERE workspace_id = $1 AND user_id = $2
                AND (status_source IS NOT NULL
                     OR (status_emoji IS NULL AND status_text IS NULL)
                     OR status_expires_at <= $7)",
            workspace_id,
            user_id,
            status.emoji,
            status.text,
            status.until,
            status.provider,
            now
        )
        .execute(&mut *tx)
        .await?
        .rows_affected()
            > 0;
        if set {
            sqlx::query!(
                "UPDATE calendar_syncs SET status_event_id = $4
                  WHERE workspace_id = $1 AND user_id = $2 AND provider = $3",
                workspace_id,
                user_id,
                status.provider,
                event_id
            )
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(set)
    }

    pub async fn clear_status(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        provider: Option<&str>,
    ) -> sqlx::Result<bool> {
        let mut tx = self.pool.begin().await?;
        let source = sqlx::query_scalar!(
            r#"SELECT status_source AS "status_source!" FROM workspace_members
                WHERE workspace_id = $1 AND user_id = $2 AND status_source IS NOT NULL
                  AND ($3::text IS NULL OR status_source = $3)
                  FOR UPDATE"#,
            workspace_id,
            user_id,
            provider
        )
        .fetch_optional(&mut *tx)
        .await?;
        let Some(source) = source else {
            return Ok(false);
        };
        sqlx::query!(
            "UPDATE workspace_members
                SET status_emoji = NULL, status_text = NULL, status_expires_at = NULL,
                    status_source = NULL
              WHERE workspace_id = $1 AND user_id = $2",
            workspace_id,
            user_id
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "UPDATE calendar_syncs SET status_event_id = NULL
              WHERE workspace_id = $1 AND user_id = $2 AND provider = $3",
            workspace_id,
            user_id,
            source
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(true)
    }

    pub async fn clear_provider_statuses(
        &self,
        workspace_id: Uuid,
        provider: &str,
    ) -> sqlx::Result<u64> {
        let result = sqlx::query!(
            "UPDATE workspace_members
                SET status_emoji = NULL, status_text = NULL, status_expires_at = NULL,
                    status_source = NULL
              WHERE workspace_id = $1 AND status_source = $2",
            workspace_id,
            provider
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn mark_away(&self, now: DateTime<Utc>) -> sqlx::Result<Vec<Uuid>> {
        sqlx::query_scalar!(
            r#"WITH away AS (
                   SELECT e.user_id, MAX(e.ends_at) AS until
                     FROM calendar_events e
                     JOIN calendar_syncs s USING (workspace_id, user_id, provider)
                     JOIN workspace_apps a
                       ON a.workspace_id = e.workspace_id AND a.provider = e.provider AND a.enabled
                    WHERE s.status_on AND e.kind = 'out_of_office'
                      AND e.starts_at <= $1 AND e.ends_at > $1
                    GROUP BY e.user_id
               ),
               marked AS (
                   UPDATE users u SET calendar_away_until = away.until
                     FROM away
                    WHERE u.id = away.user_id
                      AND u.calendar_away_until IS DISTINCT FROM away.until
                   RETURNING u.id
               ),
               cleared AS (
                   UPDATE users u SET calendar_away_until = NULL
                    WHERE u.calendar_away_until IS NOT NULL
                      AND NOT EXISTS (SELECT 1 FROM away WHERE away.user_id = u.id)
                   RETURNING u.id
               )
               SELECT id AS "id!" FROM marked UNION SELECT id FROM cleared"#,
            now
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn clear_away(&self, user_id: Uuid) -> sqlx::Result<bool> {
        let result = sqlx::query!(
            "UPDATE users SET calendar_away_until = NULL
              WHERE id = $1 AND calendar_away_until IS NOT NULL",
            user_id
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn claim_reminders(&self, now: DateTime<Utc>) -> sqlx::Result<Vec<DueReminder>> {
        sqlx::query_as!(
            DueReminder,
            r#"UPDATE calendar_events e SET reminded_at = $1
                 FROM calendar_syncs s, workspace_apps a, users u
                WHERE s.workspace_id = e.workspace_id AND s.user_id = e.user_id
                  AND s.provider = e.provider
                  AND a.workspace_id = e.workspace_id AND a.provider = e.provider AND a.enabled
                  AND u.id = e.user_id
                  AND s.reminders_on AND e.attending AND NOT e.all_day
                  AND e.kind <> 'out_of_office' AND e.reminded_at IS NULL
                  AND e.starts_at > $1::timestamptz - interval '30 seconds'
                  AND e.starts_at <= $1::timestamptz + interval '60 seconds'
            RETURNING e.workspace_id, e.user_id, e.provider, e.event_id, e.title, e.private,
                      e.join_url,
                      e.starts_at AT TIME ZONE COALESCE(u.timezone, 'UTC') AS "starts_local!",
                      e.ends_at AT TIME ZONE COALESCE(u.timezone, 'UTC') AS "ends_local!",
                      u.preferences->>'language' AS language,
                      u.preferences->>'clock' AS clock"#,
            now
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn release_reminder(&self, reminder: &DueReminder) -> sqlx::Result<()> {
        sqlx::query!(
            "UPDATE calendar_events SET reminded_at = NULL
              WHERE workspace_id = $1 AND user_id = $2 AND provider = $3 AND event_id = $4",
            reminder.workspace_id,
            reminder.user_id,
            reminder.provider,
            reminder.event_id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn claim_agendas(&self, now: DateTime<Utc>) -> sqlx::Result<Vec<DueAgenda>> {
        sqlx::query_as!(
            DueAgenda,
            r#"WITH due AS (
                   SELECT s.workspace_id, s.user_id, s.provider,
                          COALESCE(u.timezone, 'UTC') AS timezone,
                          ($1 AT TIME ZONE COALESCE(u.timezone, 'UTC')) AS local_now,
                          s.agenda_at,
                          u.preferences->>'language' AS language,
                          u.preferences->>'clock' AS clock
                     FROM calendar_syncs s
                     JOIN users u ON u.id = s.user_id
                     JOIN workspace_apps a
                       ON a.workspace_id = s.workspace_id AND a.provider = s.provider AND a.enabled
                    WHERE s.agenda_on AND s.last_synced_at IS NOT NULL
               )
               UPDATE calendar_syncs s SET agenda_sent_on = due.local_now::date
                 FROM due
                WHERE s.workspace_id = due.workspace_id AND s.user_id = due.user_id
                  AND s.provider = due.provider
                  AND s.agenda_sent_on IS DISTINCT FROM due.local_now::date
                  AND due.local_now >= due.local_now::date + due.agenda_at
                  AND due.local_now < due.local_now::date + due.agenda_at + interval '2 hours'
            RETURNING s.workspace_id, s.user_id, s.provider, due.local_now::date AS "day!",
                      due.timezone AS "timezone!", due.language, due.clock"#,
            now
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn day_events(&self, agenda: &DueAgenda) -> sqlx::Result<Vec<AgendaEvent>> {
        sqlx::query_as!(
            AgendaEvent,
            r#"SELECT title, private, all_day, join_url,
                      starts_at AT TIME ZONE $5 AS "starts_local!",
                      ends_at AT TIME ZONE $5 AS "ends_local!"
                 FROM calendar_events
                WHERE workspace_id = $1 AND user_id = $2 AND provider = $3
                  AND starts_at < ($4::date + 1)::timestamp AT TIME ZONE $5
                  AND ends_at > $4::date::timestamp AT TIME ZONE $5
                ORDER BY all_day DESC, starts_at, ends_at"#,
            agenda.workspace_id,
            agenda.user_id,
            agenda.provider,
            agenda.day,
            agenda.timezone
        )
        .fetch_all(&self.pool)
        .await
    }
}
