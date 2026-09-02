use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize)]
pub struct PushSubscription {
    pub id: Uuid,
    pub user_id: Uuid,
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
    pub user_agent: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

#[derive(Clone)]
pub struct PushRepo {
    pool: PgPool,
}

impl PushRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Keyed on the endpoint, because that is what identifies a browser to the
    /// push service. A browser that rotates its keys and re-subscribes keeps one
    /// row; inserting a second would mean every notification going out twice.
    pub async fn upsert(
        &self,
        user_id: Uuid,
        endpoint: &str,
        p256dh: &str,
        auth: &str,
        user_agent: Option<&str>,
    ) -> sqlx::Result<PushSubscription> {
        sqlx::query_as!(
            PushSubscription,
            r"
            INSERT INTO push_subscriptions (user_id, endpoint, p256dh, auth, user_agent)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (endpoint) DO UPDATE SET
                user_id = EXCLUDED.user_id,
                p256dh = EXCLUDED.p256dh,
                auth = EXCLUDED.auth,
                user_agent = EXCLUDED.user_agent
            RETURNING id, user_id, endpoint, p256dh, auth, user_agent, created_at, last_used_at
            ",
            user_id,
            endpoint,
            p256dh,
            auth,
            user_agent
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn list_for_user(&self, user_id: Uuid) -> sqlx::Result<Vec<PushSubscription>> {
        sqlx::query_as!(
            PushSubscription,
            "SELECT id, user_id, endpoint, p256dh, auth, user_agent, created_at, last_used_at
               FROM push_subscriptions WHERE user_id = $1 ORDER BY created_at",
            user_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn delete(&self, user_id: Uuid, endpoint: &str) -> sqlx::Result<bool> {
        let deleted = sqlx::query!(
            "DELETE FROM push_subscriptions WHERE user_id = $1 AND endpoint = $2",
            user_id,
            endpoint
        )
        .execute(&self.pool)
        .await?;
        Ok(deleted.rows_affected() == 1)
    }

    /// A subscription the push service has declared dead. `410 Gone` is the only
    /// reliable signal there is -- a browser that is merely closed looks
    /// identical to one that has been uninstalled.
    pub async fn delete_by_endpoint(&self, endpoint: &str) -> sqlx::Result<()> {
        sqlx::query!(
            "DELETE FROM push_subscriptions WHERE endpoint = $1",
            endpoint
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn touch(&self, id: Uuid) {
        let _ = sqlx::query!(
            "UPDATE push_subscriptions SET last_used_at = NOW() WHERE id = $1",
            id
        )
        .execute(&self.pool)
        .await;
    }
}
