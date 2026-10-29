use sqlx::PgPool;
use uuid::Uuid;

pub struct Subscriber {
    pub id: Uuid,
    pub bot_user_id: Uuid,
    pub scopes: Vec<String>,
    pub events: Vec<String>,
    pub channel_type: String,
    pub is_member: bool,
}

pub struct NewDelivery<'a> {
    pub app_id: Uuid,
    pub event_id: Uuid,
    pub channel_id: Uuid,
    pub kind: &'a str,
    pub body: &'a serde_json::Value,
    pub ttl_secs: f64,
}

pub struct Claimed {
    pub id: Uuid,
    pub app_id: Uuid,
    pub body: serde_json::Value,
    pub attempts: i32,
    pub request_url: Option<String>,
    pub signing_secret_encrypted: Option<Vec<u8>>,
    pub signing_secret_nonce: Option<Vec<u8>>,
    pub signing_secret_key_id: Option<String>,
}

#[derive(Clone)]
pub struct AppEventRepo {
    pool: PgPool,
}

impl AppEventRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn subscribers(
        &self,
        workspace_id: Uuid,
        channel_id: Uuid,
    ) -> sqlx::Result<Vec<Subscriber>> {
        sqlx::query_as!(
            Subscriber,
            r#"SELECT b.id, b.bot_user_id, b.scopes, b.events,
                      c.channel_type::text AS "channel_type!",
                      EXISTS (SELECT 1 FROM channel_members m
                               WHERE m.channel_id = c.id AND m.user_id = b.bot_user_id) AS "is_member!"
                 FROM bot_apps b
                 JOIN channels c ON c.id = $2 AND c.workspace_id = b.workspace_id
                WHERE b.workspace_id = $1
                  AND cardinality(b.events) > 0
                  AND (b.socket_mode OR b.request_url IS NOT NULL)"#,
            workspace_id,
            channel_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn enqueue(&self, delivery: NewDelivery<'_>) -> sqlx::Result<bool> {
        let inserted = sqlx::query!(
            "INSERT INTO app_deliveries (app_id, event_id, channel_id, kind, body, expires_at)
             VALUES ($1, $2, $3, $4, $5, NOW() + make_interval(secs => $6))
             ON CONFLICT (app_id, event_id) DO NOTHING",
            delivery.app_id,
            delivery.event_id,
            delivery.channel_id,
            delivery.kind,
            delivery.body,
            delivery.ttl_secs
        )
        .execute(&self.pool)
        .await?;
        Ok(inserted.rows_affected() == 1)
    }

    pub async fn claim_http(&self, lease_secs: f64, limit: i64) -> sqlx::Result<Vec<Claimed>> {
        sqlx::query_as!(
            Claimed,
            r#"WITH heads AS (
                   SELECT DISTINCT ON (d.app_id, d.channel_id)
                          d.id, d.next_attempt_at, d.leased_until, d.expires_at
                     FROM app_deliveries d
                     JOIN bot_apps b ON b.id = d.app_id
                    WHERE d.delivered_at IS NULL AND d.failed_at IS NULL
                      AND d.kind = 'event'
                      AND NOT b.socket_mode AND b.request_url IS NOT NULL
                    ORDER BY d.app_id, d.channel_id, d.seq
               ), due AS (
                   SELECT id FROM heads
                    WHERE next_attempt_at <= NOW() AND expires_at > NOW()
                      AND (leased_until IS NULL OR leased_until < NOW())
                    LIMIT $2
               )
               UPDATE app_deliveries d
                  SET leased_until = NOW() + make_interval(secs => $1),
                      attempts = d.attempts + 1
                 FROM due, bot_apps b
                WHERE d.id = due.id AND b.id = d.app_id
                  AND d.delivered_at IS NULL
                  AND (d.leased_until IS NULL OR d.leased_until < NOW())
               RETURNING d.id, d.app_id, d.body, d.attempts,
                         b.request_url AS "request_url?",
                         b.signing_secret_encrypted AS "signing_secret_encrypted?",
                         b.signing_secret_nonce AS "signing_secret_nonce?",
                         b.signing_secret_key_id AS "signing_secret_key_id?""#,
            lease_secs,
            limit
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn delivered(&self, id: Uuid) -> sqlx::Result<()> {
        sqlx::query!(
            "WITH done AS (
                 UPDATE app_deliveries SET delivered_at = NOW(), leased_until = NULL
                  WHERE id = $1 AND delivered_at IS NULL
                 RETURNING app_id
             )
             UPDATE bot_apps SET failing_since = NULL, last_failure = NULL
              WHERE id IN (SELECT app_id FROM done) AND failing_since IS NOT NULL",
            id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn retry_later(&self, id: Uuid, delay_secs: f64, error: &str) -> sqlx::Result<()> {
        sqlx::query!(
            "UPDATE app_deliveries
                SET leased_until = NULL,
                    next_attempt_at = NOW() + make_interval(secs => $2),
                    last_error = $3
              WHERE id = $1 AND delivered_at IS NULL",
            id,
            delay_secs,
            error
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn give_up_on_expired(&self) -> sqlx::Result<()> {
        sqlx::query!(
            "WITH failed AS (
                 UPDATE app_deliveries
                    SET failed_at = NOW(), leased_until = NULL
                  WHERE delivered_at IS NULL AND failed_at IS NULL
                    AND expires_at <= NOW()
                    AND (leased_until IS NULL OR leased_until < NOW())
                 RETURNING app_id, last_error, seq
             ), latest AS (
                 SELECT DISTINCT ON (app_id) app_id, last_error
                   FROM failed ORDER BY app_id, seq DESC
             )
             UPDATE bot_apps b
                SET failing_since = COALESCE(b.failing_since, NOW()),
                    last_failure = COALESCE(l.last_error, 'Nothing took the event within an hour')
               FROM latest l
              WHERE b.id = l.app_id",
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn purge(&self, keep_secs: f64) -> sqlx::Result<()> {
        sqlx::query!(
            "DELETE FROM app_deliveries
              WHERE (delivered_at IS NOT NULL OR failed_at IS NOT NULL)
                AND created_at < NOW() - make_interval(secs => $1)",
            keep_secs
        )
        .execute(&self.pool)
        .await?;
        sqlx::query!(
            "DELETE FROM app_sockets
              WHERE (connected_at IS NULL AND opened_at < NOW() - INTERVAL '5 minutes')
                 OR (connected_at IS NOT NULL AND seen_at < NOW() - INTERVAL '5 minutes')"
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
