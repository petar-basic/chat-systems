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

    pub async fn other_servers_allowed(&self) -> sqlx::Result<bool> {
        let on = sqlx::query_scalar!("SELECT other_servers_allowed FROM instance_settings")
            .fetch_optional(&self.pool)
            .await?;
        Ok(on.unwrap_or(true))
    }

    pub async fn giphy_api_key(&self) -> sqlx::Result<Option<String>> {
        let key = sqlx::query_scalar!("SELECT giphy_api_key FROM instance_settings")
            .fetch_optional(&self.pool)
            .await?;
        Ok(key.flatten())
    }

    pub async fn update(&self, change: &SettingsChange<'_>) -> sqlx::Result<()> {
        sqlx::query!(
            "INSERT INTO instance_settings
                    (id, email_code_sign_in, workspace_creation_restricted, giphy_api_key,
                     other_servers_allowed)
             VALUES (TRUE, COALESCE($1, TRUE), COALESCE($2, TRUE), $4, COALESCE($5, TRUE))
             ON CONFLICT (id) DO UPDATE
                SET email_code_sign_in = COALESCE($1, instance_settings.email_code_sign_in),
                    workspace_creation_restricted =
                        COALESCE($2, instance_settings.workspace_creation_restricted),
                    giphy_api_key = CASE WHEN $3 THEN $4 ELSE instance_settings.giphy_api_key END,
                    other_servers_allowed =
                        COALESCE($5, instance_settings.other_servers_allowed),
                    updated_at = NOW()",
            change.email_code_sign_in,
            change.workspace_creation_restricted,
            change.giphy_api_key.is_some(),
            change.giphy_api_key.flatten(),
            change.other_servers_allowed
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

pub struct SettingsChange<'a> {
    pub email_code_sign_in: Option<bool>,
    pub workspace_creation_restricted: Option<bool>,
    pub giphy_api_key: Option<Option<&'a str>>,
    pub other_servers_allowed: Option<bool>,
}
