use sqlx::PgPool;
use uuid::Uuid;

use super::Sealed;

pub struct Stored {
    pub id: String,
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
    pub key_id: Option<String>,
}

pub struct StoredApp {
    pub workspace_id: Uuid,
    pub provider: String,
    pub stored: Stored,
}

pub struct StoredConnection {
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub provider: String,
    pub stored: Stored,
}

pub struct StoredTotp {
    pub user_id: Uuid,
    pub stored: Stored,
}

pub async fn app_secrets_off(pool: &PgPool, current: &str) -> sqlx::Result<Vec<StoredApp>> {
    let rows = sqlx::query!(
        "SELECT workspace_id, provider, client_secret_encrypted, client_secret_nonce, client_secret_key_id
           FROM workspace_apps
          WHERE client_secret_key_id IS DISTINCT FROM $1",
        current
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| StoredApp {
            stored: Stored {
                id: format!("workspace app {} {}", r.workspace_id, r.provider),
                ciphertext: r.client_secret_encrypted,
                nonce: r.client_secret_nonce,
                key_id: r.client_secret_key_id,
            },
            workspace_id: r.workspace_id,
            provider: r.provider,
        })
        .collect())
}

pub async fn reseal_app_secret(
    pool: &PgPool,
    app: &StoredApp,
    sealed: &Sealed,
) -> sqlx::Result<bool> {
    let result = sqlx::query!(
        "UPDATE workspace_apps
            SET client_secret_encrypted = $4, client_secret_nonce = $5, client_secret_key_id = $6
          WHERE workspace_id = $1 AND provider = $2
            AND client_secret_key_id IS NOT DISTINCT FROM $3",
        app.workspace_id,
        app.provider,
        app.stored.key_id,
        sealed.ciphertext,
        sealed.nonce,
        sealed.key_id
    )
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn access_tokens_off(
    pool: &PgPool,
    current: &str,
) -> sqlx::Result<Vec<StoredConnection>> {
    let rows = sqlx::query!(
        "SELECT workspace_id, user_id, provider, access_token_encrypted, access_token_nonce, access_token_key_id
           FROM app_connections
          WHERE access_token_key_id IS DISTINCT FROM $1",
        current
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| StoredConnection {
            stored: Stored {
                id: format!(
                    "access token {} {} {}",
                    r.workspace_id, r.user_id, r.provider
                ),
                ciphertext: r.access_token_encrypted,
                nonce: r.access_token_nonce,
                key_id: r.access_token_key_id,
            },
            workspace_id: r.workspace_id,
            user_id: r.user_id,
            provider: r.provider,
        })
        .collect())
}

pub async fn reseal_access_token(
    pool: &PgPool,
    connection: &StoredConnection,
    sealed: &Sealed,
) -> sqlx::Result<bool> {
    let result = sqlx::query!(
        "UPDATE app_connections
            SET access_token_encrypted = $5, access_token_nonce = $6, access_token_key_id = $7
          WHERE workspace_id = $1 AND user_id = $2 AND provider = $3
            AND access_token_key_id IS NOT DISTINCT FROM $4",
        connection.workspace_id,
        connection.user_id,
        connection.provider,
        connection.stored.key_id,
        sealed.ciphertext,
        sealed.nonce,
        sealed.key_id
    )
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn refresh_tokens_off(
    pool: &PgPool,
    current: &str,
) -> sqlx::Result<Vec<StoredConnection>> {
    let rows = sqlx::query!(
        r#"SELECT workspace_id, user_id, provider,
                  refresh_token_encrypted AS "refresh_token_encrypted!",
                  refresh_token_nonce AS "refresh_token_nonce!",
                  refresh_token_key_id
             FROM app_connections
            WHERE refresh_token_encrypted IS NOT NULL
              AND refresh_token_nonce IS NOT NULL
              AND refresh_token_key_id IS DISTINCT FROM $1"#,
        current
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| StoredConnection {
            stored: Stored {
                id: format!(
                    "refresh token {} {} {}",
                    r.workspace_id, r.user_id, r.provider
                ),
                ciphertext: r.refresh_token_encrypted,
                nonce: r.refresh_token_nonce,
                key_id: r.refresh_token_key_id,
            },
            workspace_id: r.workspace_id,
            user_id: r.user_id,
            provider: r.provider,
        })
        .collect())
}

pub async fn reseal_refresh_token(
    pool: &PgPool,
    connection: &StoredConnection,
    sealed: &Sealed,
) -> sqlx::Result<bool> {
    let result = sqlx::query!(
        "UPDATE app_connections
            SET refresh_token_encrypted = $5, refresh_token_nonce = $6, refresh_token_key_id = $7
          WHERE workspace_id = $1 AND user_id = $2 AND provider = $3
            AND refresh_token_key_id IS NOT DISTINCT FROM $4",
        connection.workspace_id,
        connection.user_id,
        connection.provider,
        connection.stored.key_id,
        sealed.ciphertext,
        sealed.nonce,
        sealed.key_id
    )
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn totp_secrets_off(pool: &PgPool, current: &str) -> sqlx::Result<Vec<StoredTotp>> {
    let rows = sqlx::query!(
        "SELECT user_id, secret_encrypted, nonce, key_id
           FROM user_totp
          WHERE key_id IS DISTINCT FROM $1",
        current
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| StoredTotp {
            stored: Stored {
                id: format!("two-factor secret of {}", r.user_id),
                ciphertext: r.secret_encrypted,
                nonce: r.nonce,
                key_id: r.key_id,
            },
            user_id: r.user_id,
        })
        .collect())
}

pub async fn reseal_totp_secret(
    pool: &PgPool,
    totp: &StoredTotp,
    sealed: &Sealed,
) -> sqlx::Result<bool> {
    let result = sqlx::query!(
        "UPDATE user_totp
            SET secret_encrypted = $3, nonce = $4, key_id = $5
          WHERE user_id = $1 AND key_id IS NOT DISTINCT FROM $2",
        totp.user_id,
        totp.stored.key_id,
        sealed.ciphertext,
        sealed.nonce,
        sealed.key_id
    )
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}
