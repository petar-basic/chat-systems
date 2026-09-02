use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::models::{User, UserStatus};

pub struct UserRepo {
    pool: PgPool,
}

impl UserRepo {
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

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
        sqlx::query_as!(
            User,
            r#"
            INSERT INTO users (email, password_hash, display_name, is_instance_admin)
            VALUES ($1, $2, $3, $4)
            RETURNING id, email, password_hash, display_name, avatar_url, bio, timezone AS "timezone!",
                   status AS "status: UserStatus", status_emoji, status_text, status_expires_at,
                   is_instance_admin AS "is_instance_admin!", created_at, updated_at
            "#,
            email,
            password_hash,
            display_name,
            is_instance_admin
        )
        .fetch_one(&self.pool)
        .await
    }

    /// Created active and with no password: the identity provider is the
    /// credential, and an SSO account that also has a password has a second door
    /// nobody is watching.
    pub async fn create_sso_user(&self, email: &str) -> sqlx::Result<User> {
        sqlx::query_as!(
            User,
            r#"
            INSERT INTO users (email, display_name, status)
            VALUES ($1::text, split_part($1::text, '@', 1), 'active')
            RETURNING id, email, password_hash, display_name, avatar_url, bio, timezone AS "timezone!",
                   status AS "status: UserStatus", status_emoji, status_text, status_expires_at,
                   is_instance_admin AS "is_instance_admin!", created_at, updated_at
            "#,
            email
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn find_by_id(&self, id: Uuid) -> sqlx::Result<Option<User>> {
        sqlx::query_as!(
            User,
            r#"SELECT id, email, password_hash, display_name, avatar_url, bio, timezone AS "timezone!",
                   status AS "status: UserStatus", status_emoji, status_text, status_expires_at,
                   is_instance_admin AS "is_instance_admin!", created_at, updated_at
                 FROM users WHERE id = $1"#,
            id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn find_by_email(&self, email: &str) -> sqlx::Result<Option<User>> {
        sqlx::query_as!(
            User,
            r#"SELECT id, email, password_hash, display_name, avatar_url, bio, timezone AS "timezone!",
                   status AS "status: UserStatus", status_emoji, status_text, status_expires_at,
                   is_instance_admin AS "is_instance_admin!", created_at, updated_at
                 FROM users WHERE email = $1"#,
            email
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn list_page(&self, offset: i64, limit: i64) -> sqlx::Result<Vec<User>> {
        sqlx::query_as!(
            User,
            r#"SELECT id, email, password_hash, display_name, avatar_url, bio, timezone AS "timezone!",
                   status AS "status: UserStatus", status_emoji, status_text, status_expires_at,
                   is_instance_admin AS "is_instance_admin!", created_at, updated_at
                 FROM users ORDER BY created_at, id OFFSET $1 LIMIT $2"#,
            offset,
            limit
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn count(&self) -> sqlx::Result<i64> {
        sqlx::query_scalar!(r#"SELECT COUNT(*) AS "count!" FROM users"#)
            .fetch_one(&self.pool)
            .await
    }

    pub async fn activate(
        &self,
        id: Uuid,
        password_hash: &str,
        display_name: &str,
    ) -> sqlx::Result<User> {
        sqlx::query_as!(
            User,
            r#"
            UPDATE users
            SET password_hash = $2, display_name = $3, status = 'active', updated_at = NOW()
            WHERE id = $1
            RETURNING id, email, password_hash, display_name, avatar_url, bio, timezone AS "timezone!",
                   status AS "status: UserStatus", status_emoji, status_text, status_expires_at,
                   is_instance_admin AS "is_instance_admin!", created_at, updated_at
            "#,
            id,
            password_hash,
            display_name
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn set_status(
        &self,
        id: Uuid,
        emoji: Option<&str>,
        text: Option<&str>,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> sqlx::Result<User> {
        sqlx::query_as!(
            User,
            r#"
            UPDATE users
            SET status_emoji = $2,
                status_text = $3,
                status_expires_at = $4,
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, email, password_hash, display_name, avatar_url, bio, timezone AS "timezone!",
                   status AS "status: UserStatus", status_emoji, status_text, status_expires_at,
                   is_instance_admin AS "is_instance_admin!", created_at, updated_at
            "#,
            id,
            emoji,
            text,
            expires_at
        )
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
        sqlx::query_as!(
            User,
            r#"
            UPDATE users
            SET display_name = COALESCE($2, display_name),
                avatar_url = CASE
                    WHEN $3::text IS NULL THEN avatar_url
                    WHEN $3 = '' THEN NULL
                    ELSE $3
                END,
                bio = COALESCE($4, bio),
                timezone = COALESCE($5, timezone),
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, email, password_hash, display_name, avatar_url, bio, timezone AS "timezone!",
                   status AS "status: UserStatus", status_emoji, status_text, status_expires_at,
                   is_instance_admin AS "is_instance_admin!", created_at, updated_at
            "#,
            id,
            display_name,
            avatar_url,
            bio,
            timezone
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn update_password(&self, id: Uuid, password_hash: &str) -> sqlx::Result<()> {
        sqlx::query!(
            "UPDATE users SET password_hash = $2, updated_at = NOW() WHERE id = $1",
            id,
            password_hash
        )
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
        sqlx::query!(
            "INSERT INTO refresh_tokens (user_id, token_hash, expires_at) VALUES ($1, $2, $3)",
            user_id,
            token_hash,
            expires_at
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn find_refresh_token(&self, token_hash: &str) -> sqlx::Result<Option<Uuid>> {
        sqlx::query_scalar!(
            "SELECT user_id FROM refresh_tokens WHERE token_hash = $1 AND expires_at > NOW()",
            token_hash
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn delete_refresh_token(&self, token_hash: &str) -> sqlx::Result<()> {
        sqlx::query!(
            "DELETE FROM refresh_tokens WHERE token_hash = $1",
            token_hash
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_user_refresh_tokens(&self, user_id: Uuid) -> sqlx::Result<()> {
        sqlx::query!("DELETE FROM refresh_tokens WHERE user_id = $1", user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_user_refresh_tokens_except(
        &self,
        user_id: Uuid,
        token_hash: &str,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            "DELETE FROM refresh_tokens WHERE user_id = $1 AND token_hash <> $2",
            user_id,
            token_hash
        )
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
        sqlx::query!(
            "DELETE FROM refresh_tokens WHERE token_hash = $1",
            token_hash
        )
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
        sqlx::query!(
            "INSERT INTO refresh_tokens (user_id, token_hash, expires_at) VALUES ($1, $2, $3)",
            user_id,
            token_hash,
            expires_at
        )
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
        sqlx::query!(
            "INSERT INTO password_reset_tokens (jti, user_id, expires_at) VALUES ($1, $2, $3)",
            jti,
            user_id,
            expires_at
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn consume_reset_jti(&self, jti: Uuid, user_id: Uuid) -> sqlx::Result<bool> {
        let result = sqlx::query!(
            "DELETE FROM password_reset_tokens WHERE jti = $1 AND user_id = $2 AND expires_at > NOW()",
            jti,
            user_id
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }
}
