use sqlx::PgPool;

#[derive(Clone)]
pub struct InstanceSettingsRepo {
    pool: PgPool,
}

impl InstanceSettingsRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn email_code_sign_in(&self) -> sqlx::Result<bool> {
        let on = sqlx::query_scalar!("SELECT email_code_sign_in FROM instance_settings")
            .fetch_optional(&self.pool)
            .await?;
        Ok(on.unwrap_or(true))
    }

    pub async fn set_email_code_sign_in(&self, on: bool) -> sqlx::Result<()> {
        sqlx::query!(
            "INSERT INTO instance_settings (id, email_code_sign_in) VALUES (TRUE, $1)
             ON CONFLICT (id) DO UPDATE
                SET email_code_sign_in = EXCLUDED.email_code_sign_in, updated_at = NOW()",
            on
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
