use sqlx::PgPool;
use uuid::Uuid;

use super::models::Claim;

#[derive(Clone)]
pub struct AppUnfurlRepo {
    pool: PgPool,
}

impl AppUnfurlRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn domains(&self, app_id: Uuid) -> sqlx::Result<Vec<String>> {
        sqlx::query_scalar!(
            "SELECT domain FROM app_unfurl_domains WHERE app_id = $1 ORDER BY position",
            app_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn taken(
        &self,
        workspace_id: Uuid,
        app_id: Uuid,
        domains: &[String],
    ) -> sqlx::Result<Vec<String>> {
        sqlx::query_scalar!(
            "SELECT domain FROM app_unfurl_domains
              WHERE workspace_id = $1 AND app_id <> $2 AND domain = ANY($3::text[])
              ORDER BY domain",
            workspace_id,
            app_id,
            domains
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn replace(
        &self,
        app_id: Uuid,
        workspace_id: Uuid,
        domains: &[String],
    ) -> sqlx::Result<Vec<String>> {
        let mut tx = self.pool.begin().await?;
        sqlx::query!(
            "DELETE FROM app_unfurl_domains WHERE app_id = $1 AND NOT (domain = ANY($2::text[]))",
            app_id,
            domains
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "INSERT INTO app_unfurl_domains (app_id, workspace_id, domain, position)
             SELECT $1, $2, d.domain, d.position::smallint
               FROM UNNEST($3::text[]) WITH ORDINALITY AS d(domain, position)
             ON CONFLICT (app_id, domain) DO UPDATE SET position = EXCLUDED.position",
            app_id,
            workspace_id,
            domains
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        self.domains(app_id).await
    }

    pub async fn claims(&self, workspace_id: Uuid, channel_id: Uuid) -> sqlx::Result<Vec<Claim>> {
        sqlx::query_as!(
            Claim,
            r#"SELECT b.id AS app_id, b.bot_user_id, d.domain, b.scopes, b.events,
                      (b.socket_mode OR b.request_url IS NOT NULL) AS "reachable!",
                      EXISTS (SELECT 1 FROM channel_members m
                               WHERE m.channel_id = $2 AND m.user_id = b.bot_user_id) AS "is_member!"
                 FROM app_unfurl_domains d
                 JOIN bot_apps b ON b.id = d.app_id
                WHERE d.workspace_id = $1
                  AND b.disabled_at IS NULL"#,
            workspace_id,
            channel_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn share(
        &self,
        app_id: Uuid,
        message_id: Uuid,
        urls: &[String],
        ttl_secs: f64,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            "INSERT INTO app_link_shares (app_id, message_id, urls, expires_at)
             VALUES ($1, $2, $3, NOW() + make_interval(secs => $4))
             ON CONFLICT (app_id, message_id)
             DO UPDATE SET urls = ARRAY(SELECT DISTINCT u
                                          FROM UNNEST(app_link_shares.urls || EXCLUDED.urls) AS u),
                           expires_at = EXCLUDED.expires_at",
            app_id,
            message_id,
            urls,
            ttl_secs
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn shared(
        &self,
        app_id: Uuid,
        message_id: Uuid,
    ) -> sqlx::Result<Option<Vec<String>>> {
        sqlx::query_scalar!(
            "SELECT urls FROM app_link_shares
              WHERE app_id = $1 AND message_id = $2 AND expires_at > NOW()",
            app_id,
            message_id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn purge_expired(&self) -> sqlx::Result<u64> {
        Ok(
            sqlx::query!("DELETE FROM app_link_shares WHERE expires_at < NOW() - INTERVAL '1 day'")
                .execute(&self.pool)
                .await?
                .rows_affected(),
        )
    }
}
