use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::models::{AppEvent, BotApp, BotToken, Scope};
use crate::sealed::Sealed;

struct BotAppRow {
    id: Uuid,
    workspace_id: Uuid,
    bot_user_id: Uuid,
    name: String,
    description: Option<String>,
    icon_url: Option<String>,
    scopes: Vec<String>,
    events: Vec<String>,
    request_url: Option<String>,
    socket_mode: bool,
    has_app_token: bool,
    connected_sockets: i64,
    failing_since: Option<DateTime<Utc>>,
    last_failure: Option<String>,
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
            events: parse_events(&row.events),
            request_url: row.request_url,
            socket_mode: row.socket_mode,
            has_app_token: row.has_app_token,
            connected_sockets: row.connected_sockets,
            failing_since: row.failing_since,
            last_failure: row.last_failure,
            created_by: row.created_by,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

fn parse_scopes(stored: &[String]) -> Vec<Scope> {
    stored.iter().filter_map(|s| Scope::parse(s)).collect()
}

pub fn parse_events(stored: &[String]) -> Vec<AppEvent> {
    stored.iter().filter_map(|s| AppEvent::parse(s)).collect()
}

fn store_events(events: &[AppEvent]) -> Vec<String> {
    AppEvent::ALL
        .into_iter()
        .filter(|e| events.contains(e))
        .map(|e| e.as_str().to_string())
        .collect()
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
    pub signing_secret: &'a Sealed,
}

pub struct BotAppChanges<'a> {
    pub name: Option<&'a str>,
    pub description: Option<&'a str>,
    pub icon_url: Option<&'a str>,
    pub scopes: Option<&'a [Scope]>,
    pub events: Option<&'a [AppEvent]>,
    pub request_url: Option<&'a str>,
    pub socket_mode: Option<bool>,
}

pub struct SealedSecret {
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
    pub key_id: Option<String>,
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
            r#"INSERT INTO bot_apps (workspace_id, bot_user_id, name, description, icon_url, scopes, token_hash, created_by,
                                     signing_secret_encrypted, signing_secret_nonce, signing_secret_key_id)
              VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
              RETURNING id, workspace_id, bot_user_id, name, description, icon_url, scopes, events,
                        request_url, socket_mode,
                        (app_token_hash IS NOT NULL) AS "has_app_token!",
                        (SELECT COUNT(*) FROM app_sockets s
                          WHERE s.app_id = bot_apps.id AND s.connected_at IS NOT NULL
                            AND s.seen_at > NOW() - INTERVAL '30 seconds') AS "connected_sockets!",
                        failing_since, last_failure,
                        created_by, created_at, updated_at"#,
            app.workspace_id,
            bot_user_id,
            app.name,
            app.description,
            app.icon_url,
            &store_scopes(app.scopes),
            app.token_hash,
            app.created_by,
            app.signing_secret.ciphertext,
            app.signing_secret.nonce,
            app.signing_secret.key_id
        )
        .fetch_one(&mut *conn)
        .await?;
        Ok(row.into())
    }

    pub async fn list(&self, workspace_id: Uuid) -> sqlx::Result<Vec<BotApp>> {
        let rows = sqlx::query_as!(
            BotAppRow,
            r#"SELECT id, workspace_id, bot_user_id, name, description, icon_url, scopes, events,
                     request_url, socket_mode,
                     (app_token_hash IS NOT NULL) AS "has_app_token!",
                     (SELECT COUNT(*) FROM app_sockets s
                       WHERE s.app_id = bot_apps.id AND s.connected_at IS NOT NULL
                         AND s.seen_at > NOW() - INTERVAL '30 seconds') AS "connected_sockets!",
                     failing_since, last_failure,
                     created_by, created_at, updated_at
                FROM bot_apps WHERE workspace_id = $1 ORDER BY lower(name), id"#,
            workspace_id
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn find(&self, id: Uuid) -> sqlx::Result<Option<BotApp>> {
        let row = sqlx::query_as!(
            BotAppRow,
            r#"SELECT id, workspace_id, bot_user_id, name, description, icon_url, scopes, events,
                     request_url, socket_mode,
                     (app_token_hash IS NOT NULL) AS "has_app_token!",
                     (SELECT COUNT(*) FROM app_sockets s
                       WHERE s.app_id = bot_apps.id AND s.connected_at IS NOT NULL
                         AND s.seen_at > NOW() - INTERVAL '30 seconds') AS "connected_sockets!",
                     failing_since, last_failure,
                     created_by, created_at, updated_at
                FROM bot_apps WHERE id = $1"#,
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
        let events = changes.events.map(store_events);
        let row = sqlx::query_as!(
            BotAppRow,
            r#"UPDATE bot_apps
                 SET name = COALESCE($2, name),
                     description = CASE WHEN $3::text IS NULL THEN description ELSE NULLIF($3, '') END,
                     icon_url = CASE WHEN $4::text IS NULL THEN icon_url ELSE NULLIF($4, '') END,
                     scopes = COALESCE($5, scopes),
                     events = COALESCE($6, events),
                     request_url = CASE WHEN $7::text IS NULL THEN request_url ELSE NULLIF($7, '') END,
                     socket_mode = COALESCE($8, socket_mode),
                     failing_since = CASE WHEN $7::text IS NULL AND $8::bool IS NULL THEN failing_since END,
                     last_failure = CASE WHEN $7::text IS NULL AND $8::bool IS NULL THEN last_failure END,
                     updated_at = NOW()
               WHERE id = $1
              RETURNING id, workspace_id, bot_user_id, name, description, icon_url, scopes, events,
                        request_url, socket_mode,
                        (app_token_hash IS NOT NULL) AS "has_app_token!",
                        (SELECT COUNT(*) FROM app_sockets s
                          WHERE s.app_id = bot_apps.id AND s.connected_at IS NOT NULL
                            AND s.seen_at > NOW() - INTERVAL '30 seconds') AS "connected_sockets!",
                        failing_since, last_failure,
                        created_by, created_at, updated_at"#,
            id,
            changes.name,
            changes.description,
            changes.icon_url,
            scopes.as_deref(),
            events.as_deref(),
            changes.request_url,
            changes.socket_mode
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

    pub async fn rotate_app_token(&self, id: Uuid, token_hash: Option<&str>) -> sqlx::Result<bool> {
        let mut tx = self.pool.begin().await?;
        let updated = sqlx::query!(
            "UPDATE bot_apps SET app_token_hash = $2, updated_at = NOW() WHERE id = $1",
            id,
            token_hash
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!("DELETE FROM app_sockets WHERE app_id = $1", id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(updated.rows_affected() == 1)
    }

    pub async fn close_sockets(&self, id: Uuid) -> sqlx::Result<()> {
        sqlx::query!("DELETE FROM app_sockets WHERE app_id = $1", id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn signing_secret(&self, id: Uuid) -> sqlx::Result<Option<SealedSecret>> {
        let row = sqlx::query!(
            "SELECT signing_secret_encrypted, signing_secret_nonce, signing_secret_key_id
               FROM bot_apps WHERE id = $1",
            id
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.and_then(|r| {
            Some(SealedSecret {
                ciphertext: r.signing_secret_encrypted?,
                nonce: r.signing_secret_nonce?,
                key_id: r.signing_secret_key_id,
            })
        }))
    }

    pub async fn set_signing_secret(&self, id: Uuid, sealed: &Sealed) -> sqlx::Result<bool> {
        let updated = sqlx::query!(
            "UPDATE bot_apps
                SET signing_secret_encrypted = $2, signing_secret_nonce = $3, signing_secret_key_id = $4
              WHERE id = $1",
            id,
            sealed.ciphertext,
            sealed.nonce,
            sealed.key_id
        )
        .execute(&self.pool)
        .await?;
        Ok(updated.rows_affected() == 1)
    }

    pub async fn by_app_token(&self, token_hash: &str) -> sqlx::Result<Option<(Uuid, bool)>> {
        let row = sqlx::query!(
            r"SELECT b.id, b.socket_mode
                FROM bot_apps b
                JOIN workspaces w ON w.id = b.workspace_id
               WHERE b.app_token_hash = $1 AND w.deleted_at IS NULL",
            token_hash
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| (r.id, r.socket_mode)))
    }

    pub async fn open_socket(&self, app_id: Uuid, ticket_hash: &str) -> sqlx::Result<()> {
        sqlx::query!(
            "INSERT INTO app_sockets (app_id, ticket_hash) VALUES ($1, $2)",
            app_id,
            ticket_hash
        )
        .execute(&self.pool)
        .await?;
        Ok(())
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
