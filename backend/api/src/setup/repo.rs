use sqlx::PgPool;

#[derive(Clone)]
pub struct SetupRepo {
    pool: PgPool,
}

impl SetupRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn has_instance_admin(&self) -> sqlx::Result<bool> {
        sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM users WHERE is_instance_admin) AS "exists!""#
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn ensure_nonce(&self, candidate: &str) -> sqlx::Result<String> {
        sqlx::query_scalar!(
            r#"INSERT INTO instance_setup (nonce) VALUES ($1)
               ON CONFLICT (id) DO UPDATE SET id = instance_setup.id
               RETURNING nonce"#,
            candidate
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn current_nonce(&self) -> sqlx::Result<Option<String>> {
        sqlx::query_scalar!("SELECT nonce FROM instance_setup")
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn claim(&self, nonce: &str) -> sqlx::Result<bool> {
        let claimed = sqlx::query_scalar!(
            r#"DELETE FROM instance_setup
               WHERE nonce = $1 AND NOT EXISTS (SELECT 1 FROM users WHERE is_instance_admin)
               RETURNING nonce"#,
            nonce
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(claimed.is_some())
    }

    pub async fn restore(&self, nonce: &str) -> sqlx::Result<()> {
        sqlx::query!(
            "INSERT INTO instance_setup (nonce) VALUES ($1) ON CONFLICT (id) DO NOTHING",
            nonce
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn clear(&self) -> sqlx::Result<()> {
        sqlx::query!("DELETE FROM instance_setup")
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
