use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::models::{BotApp, BotToken, Scope};

struct BotAppRow {
    id: Uuid,
    workspace_id: Uuid,
    bot_user_id: Uuid,
    name: String,
    description: Option<String>,
    icon_url: Option<String>,
    scopes: Vec<String>,
    created_by: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<BotAppRow> for BotApp {
    fn from(row: BotAppRow) -> Self {
        Self {
            id: row.id,
            workspace_id: row.workspace_id,
            bot_user_id: row.bot_user_id,
            name: row.name,
            description: row.description,
            icon_url: row.icon_url,
            scopes: parse_scopes(&row.scopes),
            created_by: row.created_by,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

fn parse_scopes(stored: &[String]) -> Vec<Scope> {
    stored.iter().filter_map(|s| Scope::parse(s)).collect()
}

fn store_scopes(scopes: &[Scope]) -> Vec<String> {
    Scope::ALL
        .into_iter()
        .filter(|s| scopes.contains(s))
        .map(|s| s.as_str().to_string())
        .collect()
}

pub struct NewBotApp<'a> {
    pub workspace_id: Uuid,
    pub created_by: Uuid,
    pub name: &'a str,
    pub description: Option<&'a str>,
    pub icon_url: Option<&'a str>,
    pub scopes: &'a [Scope],
    pub token_hash: &'a str,
}

pub struct BotAppChanges<'a> {
    pub name: Option<&'a str>,
    pub description: Option<&'a str>,
    pub icon_url: Option<&'a str>,
    pub scopes: Option<&'a [Scope]>,
}

#[derive(Clone)]
pub struct BotRepo {
    pool: PgPool,
}

impl BotRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_in(
        &self,
        conn: &mut PgConnection,
        app: NewBotApp<'_>,
    ) -> sqlx::Result<BotApp> {
        let email = format!("app-{}@bots.invalid", Uuid::new_v4().simple());
        let bot_user_id = sqlx::query_scalar!(
            r"INSERT INTO users (email, display_name, avatar_url, status, is_bot)
              VALUES ($1, $2, $3, 'active', true)
              RETURNING id",
            email,
            app.name,
            app.icon_url
        )
        .fetch_one(&mut *conn)
        .await?;

        sqlx::query!(
            "INSERT INTO workspace_members (workspace_id, user_id, role) VALUES ($1, $2, 'member')",
            app.workspace_id,
            bot_user_id
        )
        .execute(&mut *conn)
        .await?;

        let row = sqlx::query_as!(
            BotAppRow,
            r"INSERT INTO bot_apps (workspace_id, bot_user_id, name, description, icon_url, scopes, token_hash, created_by)
              VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
              RETURNING id, workspace_id, bot_user_id, name, description, icon_url, scopes,
                        created_by, created_at, updated_at",
            app.workspace_id,
            bot_user_id,
            app.name,
            app.description,
            app.icon_url,
            &store_scopes(app.scopes),
            app.token_hash,
            app.created_by
        )
        .fetch_one(&mut *conn)
        .await?;
        Ok(row.into())
    }

    pub async fn list(&self, workspace_id: Uuid) -> sqlx::Result<Vec<BotApp>> {
        let rows = sqlx::query_as!(
            BotAppRow,
            r"SELECT id, workspace_id, bot_user_id, name, description, icon_url, scopes,
                     created_by, created_at, updated_at
                FROM bot_apps WHERE workspace_id = $1 ORDER BY lower(name), id",
            workspace_id
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn find(&self, id: Uuid) -> sqlx::Result<Option<BotApp>> {
        let row = sqlx::query_as!(
            BotAppRow,
            r"SELECT id, workspace_id, bot_user_id, name, description, icon_url, scopes,
                     created_by, created_at, updated_at
                FROM bot_apps WHERE id = $1",
            id
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(Into::into))
    }

    pub async fn update(
        &self,
        id: Uuid,
        changes: BotAppChanges<'_>,
    ) -> sqlx::Result<Option<BotApp>> {
        let mut tx = self.pool.begin().await?;
        let scopes = changes.scopes.map(store_scopes);
        let row = sqlx::query_as!(
            BotAppRow,
            r"UPDATE bot_apps
                 SET name = COALESCE($2, name),
                     description = CASE WHEN $3::text IS NULL THEN description ELSE NULLIF($3, '') END,
                     icon_url = CASE WHEN $4::text IS NULL THEN icon_url ELSE NULLIF($4, '') END,
                     scopes = COALESCE($5, scopes),
                     updated_at = NOW()
               WHERE id = $1
              RETURNING id, workspace_id, bot_user_id, name, description, icon_url, scopes,
                        created_by, created_at, updated_at",
            id,
            changes.name,
            changes.description,
            changes.icon_url,
            scopes.as_deref()
        )
        .fetch_optional(&mut *tx)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        sqlx::query!(
            "UPDATE users SET display_name = $2, avatar_url = $3, updated_at = NOW() WHERE id = $1",
            row.bot_user_id,
            row.name,
            row.icon_url
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(Some(row.into()))
    }

    pub async fn rotate(&self, id: Uuid, token_hash: &str) -> sqlx::Result<bool> {
        let updated = sqlx::query!(
            "UPDATE bot_apps SET token_hash = $2, updated_at = NOW() WHERE id = $1",
            id,
            token_hash
        )
        .execute(&self.pool)
        .await?;
        Ok(updated.rows_affected() == 1)
    }

    pub async fn remove_in(&self, conn: &mut PgConnection, id: Uuid) -> sqlx::Result<bool> {
        let bot_user_id = sqlx::query_scalar!(
            "DELETE FROM bot_apps WHERE id = $1 RETURNING bot_user_id",
            id
        )
        .fetch_optional(&mut *conn)
        .await?;
        let Some(bot_user_id) = bot_user_id else {
            return Ok(false);
        };
        sqlx::query!(
            "UPDATE users SET status = 'suspended', updated_at = NOW() WHERE id = $1",
            bot_user_id
        )
        .execute(&mut *conn)
        .await?;
        Ok(true)
    }

    pub async fn authenticate(&self, token_hash: &str) -> sqlx::Result<Option<(Uuid, BotToken)>> {
        let row = sqlx::query!(
            r"SELECT b.id, b.workspace_id, b.bot_user_id, b.scopes
                FROM bot_apps b
                JOIN workspaces w ON w.id = b.workspace_id
               WHERE b.token_hash = $1 AND w.deleted_at IS NULL",
            token_hash
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|row| {
            (
                row.bot_user_id,
                BotToken {
                    app_id: row.id,
                    workspace_id: row.workspace_id,
                    scopes: parse_scopes(&row.scopes),
                },
            )
        }))
    }
}
