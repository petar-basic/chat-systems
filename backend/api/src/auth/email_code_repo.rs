use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct CodeTry {
    pub code_hash: String,
    pub attempts: i32,
}

#[derive(Clone)]
pub struct EmailCodeRepo {
    pool: PgPool,
}

impl EmailCodeRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn replace(
        &self,
        user_id: Uuid,
        code_hash: &str,
        lifetime_mins: i32,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            "INSERT INTO email_sign_in_codes (user_id, code_hash, expires_at)
             VALUES ($1, $2, NOW() + make_interval(mins => $3))
             ON CONFLICT (user_id) DO UPDATE
                SET code_hash = EXCLUDED.code_hash,
                    attempts = 0,
                    created_at = NOW(),
                    expires_at = EXCLUDED.expires_at,
                    used_at = NULL",
            user_id,
            code_hash,
            lifetime_mins
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn claim_try(&self, user_id: Uuid, max_tries: i32) -> sqlx::Result<Option<CodeTry>> {
        sqlx::query_as!(
            CodeTry,
            "UPDATE email_sign_in_codes
                SET attempts = attempts + 1
              WHERE user_id = $1
                AND used_at IS NULL
                AND expires_at > NOW()
                AND attempts < $2
          RETURNING code_hash, attempts",
            user_id,
            max_tries
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn record_failure(&self, user_id: Uuid) -> sqlx::Result<i32> {
        sqlx::query_scalar!(
            "UPDATE email_sign_in_codes
                SET failures = CASE WHEN failures_since > NOW() - INTERVAL '1 day'
                                    THEN failures + 1 ELSE 1 END,
                    failures_since = CASE WHEN failures_since > NOW() - INTERVAL '1 day'
                                          THEN failures_since ELSE NOW() END
              WHERE user_id = $1
          RETURNING failures",
            user_id
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn locked_out(&self, user_id: Uuid, ceiling: i32) -> sqlx::Result<bool> {
        sqlx::query_scalar!(
            r#"SELECT EXISTS(
                   SELECT 1 FROM email_sign_in_codes
                    WHERE user_id = $1
                      AND failures >= $2
                      AND failures_since > NOW() - INTERVAL '1 day'
               ) AS "locked!""#,
            user_id,
            ceiling
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn consume(&self, user_id: Uuid, code_hash: &str) -> sqlx::Result<bool> {
        let result = sqlx::query!(
            "UPDATE email_sign_in_codes
                SET used_at = NOW()
              WHERE user_id = $1
                AND code_hash = $2
                AND used_at IS NULL
                AND expires_at > NOW()",
            user_id,
            code_hash
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }
}
