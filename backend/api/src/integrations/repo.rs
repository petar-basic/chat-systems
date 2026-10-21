use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::models::*;

#[derive(Clone)]
pub struct IntegrationRepo {
    pool: PgPool,
}

pub struct NewConnection<'a> {
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub provider: &'a str,
    pub external_account: &'a str,
    pub access_token: &'a crate::sealed::Sealed,
    pub refresh_token: Option<&'a crate::sealed::Sealed>,
    pub expires_at: Option<DateTime<Utc>>,
    pub resources: &'a serde_json::Value,
}

impl IntegrationRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn apps(&self, workspace_id: Uuid) -> sqlx::Result<Vec<WorkspaceApp>> {
        sqlx::query_as!(
            WorkspaceApp,
            "SELECT workspace_id, provider, client_id, client_secret_encrypted, client_secret_nonce, enabled
               FROM workspace_apps WHERE workspace_id = $1",
            workspace_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn app(
        &self,
        workspace_id: Uuid,
        provider: &str,
    ) -> sqlx::Result<Option<WorkspaceApp>> {
        sqlx::query_as!(
            WorkspaceApp,
            "SELECT workspace_id, provider, client_id, client_secret_encrypted, client_secret_nonce, enabled
               FROM workspace_apps WHERE workspace_id = $1 AND provider = $2",
            workspace_id,
            provider
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn save_app(
        &self,
        workspace_id: Uuid,
        provider: &str,
        client_id: &str,
        secret: &crate::sealed::Sealed,
        enabled: bool,
        configured_by: Uuid,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            "INSERT INTO workspace_apps
                 (workspace_id, provider, client_id, client_secret_encrypted, client_secret_nonce, enabled, configured_by)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (workspace_id, provider) DO UPDATE
                SET client_id = EXCLUDED.client_id,
                    client_secret_encrypted = EXCLUDED.client_secret_encrypted,
                    client_secret_nonce = EXCLUDED.client_secret_nonce,
                    enabled = EXCLUDED.enabled,
                    configured_by = EXCLUDED.configured_by,
                    configured_at = NOW()",
            workspace_id,
            provider,
            client_id,
            secret.ciphertext,
            secret.nonce,
            enabled,
            configured_by
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_app_settings(
        &self,
        workspace_id: Uuid,
        provider: &str,
        client_id: &str,
        enabled: bool,
        configured_by: Uuid,
    ) -> sqlx::Result<bool> {
        let result = sqlx::query!(
            "UPDATE workspace_apps
                SET client_id = $3, enabled = $4, configured_by = $5, configured_at = NOW()
              WHERE workspace_id = $1 AND provider = $2",
            workspace_id,
            provider,
            client_id,
            enabled,
            configured_by
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_app(&self, workspace_id: Uuid, provider: &str) -> sqlx::Result<bool> {
        let result = sqlx::query!(
            "DELETE FROM workspace_apps WHERE workspace_id = $1 AND provider = $2",
            workspace_id,
            provider
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn connected_accounts(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<Vec<(String, String)>> {
        let rows = sqlx::query!(
            "SELECT provider, external_account FROM app_connections WHERE workspace_id = $1 AND user_id = $2",
            workspace_id,
            user_id
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| (r.provider, r.external_account))
            .collect())
    }

    pub async fn connection(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        provider: &str,
    ) -> sqlx::Result<Option<AppConnection>> {
        sqlx::query_as!(
            AppConnection,
            "SELECT external_account, access_token_encrypted, access_token_nonce,
                    refresh_token_encrypted, refresh_token_nonce, expires_at, resources
               FROM app_connections WHERE workspace_id = $1 AND user_id = $2 AND provider = $3",
            workspace_id,
            user_id,
            provider
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn upsert_connection(&self, new: NewConnection<'_>) -> sqlx::Result<()> {
        sqlx::query!(
            "INSERT INTO app_connections
                 (workspace_id, user_id, provider, external_account, access_token_encrypted, access_token_nonce,
                  refresh_token_encrypted, refresh_token_nonce, expires_at, resources)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
             ON CONFLICT (workspace_id, user_id, provider) DO UPDATE
                SET external_account = EXCLUDED.external_account,
                    access_token_encrypted = EXCLUDED.access_token_encrypted,
                    access_token_nonce = EXCLUDED.access_token_nonce,
                    refresh_token_encrypted = EXCLUDED.refresh_token_encrypted,
                    refresh_token_nonce = EXCLUDED.refresh_token_nonce,
                    expires_at = EXCLUDED.expires_at,
                    resources = EXCLUDED.resources,
                    connected_at = NOW()",
            new.workspace_id,
            new.user_id,
            new.provider,
            new.external_account,
            new.access_token.ciphertext,
            new.access_token.nonce,
            new.refresh_token.map(|s| s.ciphertext.clone()),
            new.refresh_token.map(|s| s.nonce.clone()),
            new.expires_at,
            new.resources
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn store_refreshed_tokens(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        provider: &str,
        access_token: &crate::sealed::Sealed,
        refresh_token: Option<&crate::sealed::Sealed>,
        expires_at: Option<DateTime<Utc>>,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            "UPDATE app_connections
                SET access_token_encrypted = $4,
                    access_token_nonce = $5,
                    refresh_token_encrypted = COALESCE($6, refresh_token_encrypted),
                    refresh_token_nonce = COALESCE($7, refresh_token_nonce),
                    expires_at = $8
              WHERE workspace_id = $1 AND user_id = $2 AND provider = $3",
            workspace_id,
            user_id,
            provider,
            access_token.ciphertext,
            access_token.nonce,
            refresh_token.map(|s| s.ciphertext.clone()),
            refresh_token.map(|s| s.nonce.clone()),
            expires_at
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_connection(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        provider: &str,
    ) -> sqlx::Result<bool> {
        let result = sqlx::query!(
            "DELETE FROM app_connections WHERE workspace_id = $1 AND user_id = $2 AND provider = $3",
            workspace_id,
            user_id,
            provider
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_user_connections(&self, user_id: Uuid) -> sqlx::Result<u64> {
        let result = sqlx::query!("DELETE FROM app_connections WHERE user_id = $1", user_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }
}
