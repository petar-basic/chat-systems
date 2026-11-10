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

    pub async fn workspace_creation_restricted(&self) -> sqlx::Result<bool> {
        let on = sqlx::query_scalar!("SELECT workspace_creation_restricted FROM instance_settings")
            .fetch_optional(&self.pool)
            .await?;
        Ok(on.unwrap_or(true))
    }

    pub async fn update(
        &self,
        email_code_sign_in: Option<bool>,
        workspace_creation_restricted: Option<bool>,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            "INSERT INTO instance_settings (id, email_code_sign_in, workspace_creation_restricted)
             VALUES (TRUE, COALESCE($1, TRUE), COALESCE($2, TRUE))
             ON CONFLICT (id) DO UPDATE
                SET email_code_sign_in = COALESCE($1, instance_settings.email_code_sign_in),
                    workspace_creation_restricted =
                        COALESCE($2, instance_settings.workspace_creation_restricted),
                    updated_at = NOW()",
            email_code_sign_in,
            workspace_creation_restricted
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
