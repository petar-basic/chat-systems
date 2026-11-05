use chrono::{DateTime, Utc};
use sqlx::types::Json;
use sqlx::PgPool;
use uuid::Uuid;
use webauthn_rs::prelude::Passkey;

#[derive(Debug, Clone)]
pub struct StoredPasskey {
    pub id: Uuid,
    pub user_id: Uuid,
    pub credential: Json<Passkey>,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

#[derive(Clone)]
pub struct PasskeyRepo {
    pool: PgPool,
}

impl PasskeyRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list(&self, user_id: Uuid) -> sqlx::Result<Vec<StoredPasskey>> {
        sqlx::query_as!(
            StoredPasskey,
            r#"SELECT id, user_id, credential AS "credential: Json<Passkey>", name, created_at, last_used_at
                 FROM passkeys WHERE user_id = $1 ORDER BY created_at"#,
            user_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn count(&self, user_id: Uuid) -> sqlx::Result<i64> {
        sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "count!" FROM passkeys WHERE user_id = $1"#,
            user_id
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn find_by_credential(
        &self,
        credential_id: &[u8],
    ) -> sqlx::Result<Option<StoredPasskey>> {
        sqlx::query_as!(
            StoredPasskey,
            r#"SELECT id, user_id, credential AS "credential: Json<Passkey>", name, created_at, last_used_at
                 FROM passkeys WHERE credential_id = $1"#,
            credential_id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn insert(
        &self,
        user_id: Uuid,
        passkey: &Passkey,
        name: &str,
        limit: i64,
    ) -> sqlx::Result<Option<StoredPasskey>> {
        let credential_id: &[u8] = passkey.cred_id();
        sqlx::query_as!(
            StoredPasskey,
            r#"INSERT INTO passkeys (user_id, credential_id, credential, name)
               SELECT $1, $2, $3, $4
                WHERE (SELECT COUNT(*) FROM passkeys WHERE user_id = $1) < $5
               RETURNING id, user_id, credential AS "credential: Json<Passkey>", name, created_at, last_used_at"#,
            user_id,
            credential_id,
            Json(passkey) as _,
            name,
            limit
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn rename(&self, id: Uuid, user_id: Uuid, name: &str) -> sqlx::Result<bool> {
        let updated = sqlx::query!(
            "UPDATE passkeys SET name = $3 WHERE id = $1 AND user_id = $2",
            id,
            user_id,
            name
        )
        .execute(&self.pool)
        .await?;
        Ok(updated.rows_affected() == 1)
    }

    pub async fn record_use(&self, id: Uuid, passkey: &Passkey) -> sqlx::Result<()> {
        sqlx::query!(
            "UPDATE passkeys SET credential = $2, last_used_at = NOW() WHERE id = $1",
            id,
            Json(passkey) as _
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn remove(&self, id: Uuid, user_id: Uuid) -> sqlx::Result<bool> {
        let removed = sqlx::query!(
            "DELETE FROM passkeys WHERE id = $1 AND user_id = $2",
            id,
            user_id
        )
        .execute(&self.pool)
        .await?;
        Ok(removed.rows_affected() == 1)
    }
}
