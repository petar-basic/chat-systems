use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct TotpEnrolment {
    pub user_id: Uuid,
    pub secret_encrypted: Vec<u8>,
    pub nonce: Vec<u8>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub last_used_step: Option<i64>,
    pub created_at: DateTime<Utc>,
}

impl TotpEnrolment {
    pub fn is_active(&self) -> bool {
        self.confirmed_at.is_some()
    }
}

#[derive(Clone)]
pub struct TotpRepo {
    pool: PgPool,
}

impl TotpRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn find(&self, user_id: Uuid) -> sqlx::Result<Option<TotpEnrolment>> {
        sqlx::query_as!(
            TotpEnrolment,
            "SELECT user_id, secret_encrypted, nonce, confirmed_at, last_used_step, created_at
               FROM user_totp WHERE user_id = $1",
            user_id
        )
        .fetch_optional(&self.pool)
        .await
    }

    /// Enrolment replaces any half-finished attempt: somebody who abandoned the
    /// QR code and started again should not be blocked by their own first try.
    pub async fn start_enrolment(
        &self,
        user_id: Uuid,
        secret_encrypted: &[u8],
        nonce: &[u8],
    ) -> sqlx::Result<()> {
        sqlx::query!(
            r"
            INSERT INTO user_totp (user_id, secret_encrypted, nonce)
            VALUES ($1, $2, $3)
            ON CONFLICT (user_id) DO UPDATE SET
                secret_encrypted = EXCLUDED.secret_encrypted,
                nonce = EXCLUDED.nonce,
                confirmed_at = NULL,
                last_used_step = NULL,
                created_at = NOW()
            ",
            user_id,
            secret_encrypted,
            nonce
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn confirm(&self, user_id: Uuid, step: i64) -> sqlx::Result<()> {
        sqlx::query!(
            "UPDATE user_totp SET confirmed_at = NOW(), last_used_step = $2 WHERE user_id = $1",
            user_id,
            step
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Records the step a code came from, and refuses one that is not newer.
    /// Verifying the digits alone accepts the same code for its whole thirty
    /// seconds, which is exactly long enough for somebody reading over a
    /// shoulder.
    pub async fn claim_step(&self, user_id: Uuid, step: i64) -> sqlx::Result<bool> {
        let updated = sqlx::query!(
            r"
            UPDATE user_totp
               SET last_used_step = $2
             WHERE user_id = $1
               AND (last_used_step IS NULL OR last_used_step < $2)
            ",
            user_id,
            step
        )
        .execute(&self.pool)
        .await?;
        Ok(updated.rows_affected() == 1)
    }

    pub async fn disable(&self, user_id: Uuid) -> sqlx::Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query!("DELETE FROM user_totp WHERE user_id = $1", user_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query!(
            "DELETE FROM totp_recovery_codes WHERE user_id = $1",
            user_id
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn replace_recovery_codes(
        &self,
        user_id: Uuid,
        hashes: &[String],
    ) -> sqlx::Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query!(
            "DELETE FROM totp_recovery_codes WHERE user_id = $1",
            user_id
        )
        .execute(&mut *tx)
        .await?;
        for hash in hashes {
            sqlx::query!(
                "INSERT INTO totp_recovery_codes (user_id, code_hash) VALUES ($1, $2)",
                user_id,
                hash
            )
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn unused_recovery_hashes(&self, user_id: Uuid) -> sqlx::Result<Vec<(Uuid, String)>> {
        let rows = sqlx::query!(
            "SELECT id, code_hash FROM totp_recovery_codes WHERE user_id = $1 AND used_at IS NULL",
            user_id
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| (r.id, r.code_hash)).collect())
    }

    /// Marks it used only if it was not: a recovery code is single-use, and two
    /// concurrent attempts must not both succeed.
    pub async fn consume_recovery_code(&self, id: Uuid) -> sqlx::Result<bool> {
        let updated = sqlx::query!(
            "UPDATE totp_recovery_codes SET used_at = NOW() WHERE id = $1 AND used_at IS NULL",
            id
        )
        .execute(&self.pool)
        .await?;
        Ok(updated.rows_affected() == 1)
    }
}
