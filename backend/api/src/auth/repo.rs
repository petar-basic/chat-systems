use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::models::{RegistrationInvite, UpdateProfileRequest, User, UserStatus};

#[derive(Clone)]
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
            VALUES (lower($1), $2, $3, $4)
            RETURNING id, email, password_hash, display_name, avatar_url, bio, title, pronouns, timezone,
                   status AS "status: UserStatus",
                   is_instance_admin AS "is_instance_admin!", workspace_creator AS "workspace_creator!",
                   created_at, updated_at
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
    pub async fn create_suspended(&self, email: &str, display_name: &str) -> sqlx::Result<User> {
        sqlx::query_as!(
            User,
            r#"
            INSERT INTO users (email, display_name, status)
            VALUES (lower($1), $2, 'suspended')
            RETURNING id, email, password_hash, display_name, avatar_url, bio, title, pronouns, timezone,
                   status AS "status: UserStatus",
                   is_instance_admin AS "is_instance_admin!", workspace_creator AS "workspace_creator!",
                   created_at, updated_at
            "#,
            email,
            display_name
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn is_instance_admin(&self, id: Uuid) -> sqlx::Result<bool> {
        sqlx::query_scalar!(
            r#"SELECT EXISTS(
                   SELECT 1 FROM users WHERE id = $1 AND is_instance_admin AND status = 'active'
               ) AS "admin!""#,
            id
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn reactivate(&self, id: Uuid) -> sqlx::Result<()> {
        sqlx::query!(
            r"
            UPDATE users
               SET status = CASE
                       WHEN password_hash IS NULL
                        AND NOT EXISTS (SELECT 1 FROM oauth_accounts o WHERE o.user_id = users.id)
                       THEN 'pending'::user_status
                       ELSE 'active'::user_status
                   END,
                   updated_at = NOW()
             WHERE id = $1
            ",
            id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn create_sso_user(&self, email: &str) -> sqlx::Result<User> {
        sqlx::query_as!(
            User,
            r#"
            INSERT INTO users (email, display_name, status)
            VALUES (lower($1::text), split_part($1::text, '@', 1), 'active')
            RETURNING id, email, password_hash, display_name, avatar_url, bio, title, pronouns, timezone,
                   status AS "status: UserStatus",
                   is_instance_admin AS "is_instance_admin!", workspace_creator AS "workspace_creator!",
                   created_at, updated_at
            "#,
            email
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn find_by_id(&self, id: Uuid) -> sqlx::Result<Option<User>> {
        sqlx::query_as!(
            User,
            r#"SELECT id, email, password_hash, display_name, avatar_url, bio, title, pronouns, timezone,
                   status AS "status: UserStatus",
                   is_instance_admin AS "is_instance_admin!", workspace_creator AS "workspace_creator!",
                   created_at, updated_at
                 FROM users WHERE id = $1"#,
            id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn find_by_email(&self, email: &str) -> sqlx::Result<Option<User>> {
        sqlx::query_as!(
            User,
            r#"SELECT id, email, password_hash, display_name, avatar_url, bio, title, pronouns, timezone,
                   status AS "status: UserStatus",
                   is_instance_admin AS "is_instance_admin!", workspace_creator AS "workspace_creator!",
                   created_at, updated_at
                 FROM users WHERE email = lower($1) AND NOT is_bot"#,
            email
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn list_page(&self, offset: i64, limit: i64) -> sqlx::Result<Vec<User>> {
        sqlx::query_as!(
            User,
            r#"SELECT id, email, password_hash, display_name, avatar_url, bio, title, pronouns, timezone,
                   status AS "status: UserStatus",
                   is_instance_admin AS "is_instance_admin!", workspace_creator AS "workspace_creator!",
                   created_at, updated_at
                 FROM users WHERE NOT is_bot ORDER BY created_at, id OFFSET $1 LIMIT $2"#,
            offset,
            limit
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn count(&self) -> sqlx::Result<i64> {
        sqlx::query_scalar!(r#"SELECT COUNT(*) AS "count!" FROM users WHERE NOT is_bot"#)
            .fetch_one(&self.pool)
            .await
    }

    pub async fn unlink_sso(&self, id: Uuid) -> sqlx::Result<()> {
        sqlx::query!("DELETE FROM oauth_accounts WHERE user_id = $1", id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn activate_sso(&self, id: Uuid) -> sqlx::Result<()> {
        sqlx::query!(
            r"
            UPDATE users
            SET status = 'active',
                display_name = COALESCE(display_name, split_part(email, '@', 1)),
                updated_at = NOW()
            WHERE id = $1 AND status = 'pending'
            ",
            id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
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
            RETURNING id, email, password_hash, display_name, avatar_url, bio, title, pronouns, timezone,
                   status AS "status: UserStatus",
                   is_instance_admin AS "is_instance_admin!", workspace_creator AS "workspace_creator!",
                   created_at, updated_at
            "#,
            id,
            password_hash,
            display_name
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn update_profile(
        &self,
        id: Uuid,
        changes: &UpdateProfileRequest,
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
                title = CASE WHEN $5::text IS NULL THEN title
                             ELSE NULLIF(BTRIM($5), '') END,
                pronouns = CASE WHEN $6::text IS NULL THEN pronouns
                                ELSE NULLIF(BTRIM($6), '') END,
                timezone = COALESCE($7, timezone),
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, email, password_hash, display_name, avatar_url, bio, title, pronouns, timezone,
                   status AS "status: UserStatus",
                   is_instance_admin AS "is_instance_admin!", workspace_creator AS "workspace_creator!",
                   created_at, updated_at
            "#,
            id,
            changes.display_name.as_deref(),
            changes.avatar_url.as_deref(),
            changes.bio.as_deref(),
            changes.title.as_deref(),
            changes.pronouns.as_deref(),
            changes.timezone.as_deref()
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn timezone_exists(&self, timezone: &str) -> sqlx::Result<bool> {
        sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM pg_timezone_names WHERE name = $1) AS "exists!""#,
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

    pub async fn language(&self, user_id: Uuid) -> sqlx::Result<Option<String>> {
        sqlx::query_scalar!(
            r#"SELECT preferences->>'language' AS "language?" FROM users WHERE id = $1"#,
            user_id
        )
        .fetch_optional(&self.pool)
        .await
        .map(Option::flatten)
    }

    pub async fn store_reset_token(
        &self,
        token_hash: &str,
        user_id: Uuid,
        expires_at: DateTime<Utc>,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            "INSERT INTO password_reset_tokens (token_hash, user_id, expires_at) VALUES ($1, $2, $3)",
            token_hash,
            user_id,
            expires_at
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn consume_reset_token(&self, token_hash: &str) -> sqlx::Result<Option<Uuid>> {
        sqlx::query_scalar!(
            "DELETE FROM password_reset_tokens WHERE token_hash = $1 AND expires_at > NOW() RETURNING user_id",
            token_hash
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn store_invite_link(
        &self,
        token_hash: &str,
        invite_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            "INSERT INTO invite_links (token_hash, invite_id, user_id) VALUES ($1, $2, $3)",
            token_hash,
            invite_id,
            user_id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn find_invite_link(
        &self,
        token_hash: &str,
    ) -> sqlx::Result<Option<RegistrationInvite>> {
        sqlx::query_as!(
            RegistrationInvite,
            r#"SELECT l.user_id, l.invite_id, i.email AS "email!"
                 FROM invite_links l
                 JOIN workspace_invites i ON i.id = l.invite_id
                WHERE l.token_hash = $1 AND i.email IS NOT NULL"#,
            token_hash
        )
        .fetch_optional(&self.pool)
        .await
    }
}
