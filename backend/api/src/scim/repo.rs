use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct ScimToken {
    pub id: Uuid,
    #[serde(skip)]
    pub token_hash: String,
    pub description: Option<String>,
    pub created_by: Option<Uuid>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Stored as a digest, like the refresh tokens: a database read must not hand
/// somebody a working credential. SHA-256 rather than Argon2 because the token
/// is 192 random bits — there is nothing to guess — and every SCIM request has
/// to look it up by value.
pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[derive(Clone)]
pub struct ScimRepo {
    pool: PgPool,
}

impl ScimRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        token: &str,
        description: Option<&str>,
        created_by: Uuid,
    ) -> sqlx::Result<ScimToken> {
        sqlx::query_as::<_, ScimToken>(
            r"
            INSERT INTO scim_tokens (token_hash, description, created_by)
            VALUES ($1, $2, $3)
            RETURNING *
            ",
        )
        .bind(hash_token(token))
        .bind(description)
        .bind(created_by)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn find_active(&self, token: &str) -> sqlx::Result<Option<ScimToken>> {
        sqlx::query_as::<_, ScimToken>(
            "SELECT * FROM scim_tokens WHERE token_hash = $1 AND revoked_at IS NULL",
        )
        .bind(hash_token(token))
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn list(&self) -> sqlx::Result<Vec<ScimToken>> {
        sqlx::query_as::<_, ScimToken>("SELECT * FROM scim_tokens ORDER BY created_at DESC")
            .fetch_all(&self.pool)
            .await
    }

    pub async fn revoke(&self, id: Uuid) -> sqlx::Result<bool> {
        let updated = sqlx::query(
            "UPDATE scim_tokens SET revoked_at = NOW() WHERE id = $1 AND revoked_at IS NULL",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(updated.rows_affected() == 1)
    }

    /// Best-effort: an unrecorded use is not a reason to fail a provisioning
    /// call, but a token nobody has used in a year is worth seeing in the list.
    pub async fn touch(&self, id: Uuid) {
        let _ = sqlx::query("UPDATE scim_tokens SET last_used_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await;
    }
}
