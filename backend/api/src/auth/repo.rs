use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::models::User;

pub struct UserRepo {
    pool: PgPool,
}

impl UserRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        email: &str,
        password_hash: Option<&str>,
        display_name: Option<&str>,
        is_instance_admin: bool,
    ) -> sqlx::Result<User> {
        sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (email, password_hash, display_name, is_instance_admin)
            VALUES ($1, $2, $3, $4)
            RETURNING *
            "#,
        )
        .bind(email)
        .bind(password_hash)
        .bind(display_name)
        .bind(is_instance_admin)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn find_by_id(&self, id: Uuid) -> sqlx::Result<Option<User>> {
        sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn find_by_email(&self, email: &str) -> sqlx::Result<Option<User>> {
        sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
            .bind(email)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn activate(
        &self,
        id: Uuid,
        password_hash: &str,
        display_name: &str,
    ) -> sqlx::Result<User> {
        sqlx::query_as::<_, User>(
            r#"
            UPDATE users
            SET password_hash = $2, display_name = $3, status = 'active', updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(password_hash)
        .bind(display_name)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn update_profile(
        &self,
        id: Uuid,
        display_name: Option<&str>,
        avatar_url: Option<&str>,
        bio: Option<&str>,
        timezone: Option<&str>,
    ) -> sqlx::Result<User> {
        sqlx::query_as::<_, User>(
            r#"
            UPDATE users
            SET display_name = COALESCE($2, display_name),
                avatar_url = COALESCE($3, avatar_url),
                bio = COALESCE($4, bio),
                timezone = COALESCE($5, timezone),
                updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(display_name)
        .bind(avatar_url)
        .bind(bio)
        .bind(timezone)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn update_password(&self, id: Uuid, password_hash: &str) -> sqlx::Result<()> {
        sqlx::query("UPDATE users SET password_hash = $2, updated_at = NOW() WHERE id = $1")
            .bind(id)
            .bind(password_hash)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn store_refresh_token(
        &self,
        user_id: Uuid,
        token_hash: &str,
        expires_at: DateTime<Utc>,
    ) -> sqlx::Result<()> {
        sqlx::query(
            "INSERT INTO refresh_tokens (user_id, token_hash, expires_at) VALUES ($1, $2, $3)",
        )
        .bind(user_id)
        .bind(token_hash)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn find_refresh_token(&self, token_hash: &str) -> sqlx::Result<Option<Uuid>> {
        sqlx::query_scalar::<_, Uuid>(
            "SELECT user_id FROM refresh_tokens WHERE token_hash = $1 AND expires_at > NOW()",
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn delete_refresh_token(&self, token_hash: &str) -> sqlx::Result<()> {
        sqlx::query("DELETE FROM refresh_tokens WHERE token_hash = $1")
            .bind(token_hash)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_user_refresh_tokens(&self, user_id: Uuid) -> sqlx::Result<()> {
        sqlx::query("DELETE FROM refresh_tokens WHERE user_id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn begin(&self) -> sqlx::Result<sqlx::Transaction<'_, sqlx::Postgres>> {
        self.pool.begin().await
    }

    pub async fn delete_refresh_token_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        token_hash: &str,
    ) -> sqlx::Result<()> {
        sqlx::query("DELETE FROM refresh_tokens WHERE token_hash = $1")
            .bind(token_hash)
            .execute(&mut **tx)
            .await?;
        Ok(())
    }

    pub async fn store_refresh_token_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        user_id: Uuid,
        token_hash: &str,
        expires_at: DateTime<Utc>,
    ) -> sqlx::Result<()> {
        sqlx::query(
            "INSERT INTO refresh_tokens (user_id, token_hash, expires_at) VALUES ($1, $2, $3)",
        )
        .bind(user_id)
        .bind(token_hash)
        .bind(expires_at)
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    pub async fn store_reset_jti(
        &self,
        jti: Uuid,
        user_id: Uuid,
        expires_at: DateTime<Utc>,
    ) -> sqlx::Result<()> {
        sqlx::query(
            "INSERT INTO password_reset_tokens (jti, user_id, expires_at) VALUES ($1, $2, $3)",
        )
        .bind(jti)
        .bind(user_id)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn consume_reset_jti(&self, jti: Uuid, user_id: Uuid) -> sqlx::Result<bool> {
        let result = sqlx::query(
            "DELETE FROM password_reset_tokens WHERE jti = $1 AND user_id = $2 AND expires_at > NOW()",
        )
        .bind(jti)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }
}
