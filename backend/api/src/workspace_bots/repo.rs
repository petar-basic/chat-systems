use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::models::{WorkspaceBot, WorkspaceBotKind};

pub struct Ensured {
    pub user_id: Uuid,
    pub created: bool,
}

pub struct BotIdentity<'a> {
    pub name: &'a str,
    pub avatar_url: Option<&'a str>,
}

#[derive(Clone)]
pub struct WorkspaceBotRepo {
    pool: PgPool,
}

impl WorkspaceBotRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn find(
        &self,
        workspace_id: Uuid,
        kind: WorkspaceBotKind,
    ) -> sqlx::Result<Option<WorkspaceBot>> {
        sqlx::query_as!(
            WorkspaceBot,
            r#"SELECT b.user_id AS "user_id?", u.display_name AS "name!", u.avatar_url
                 FROM workspace_bots b
                 JOIN users u ON u.id = b.user_id
                WHERE b.workspace_id = $1 AND b.kind = $2"#,
            workspace_id,
            kind as WorkspaceBotKind
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn ensure_in(
        conn: &mut PgConnection,
        workspace_id: Uuid,
        kind: WorkspaceBotKind,
        identity: &BotIdentity<'_>,
    ) -> sqlx::Result<Ensured> {
        sqlx::query!(
            "SELECT pg_advisory_xact_lock(hashtextextended('workspace_bot:' || $1::uuid::text || ':' || $2::text, 0))",
            workspace_id,
            kind.as_str()
        )
        .execute(&mut *conn)
        .await?;
        if let Some(user_id) = sqlx::query_scalar!(
            "SELECT user_id FROM workspace_bots WHERE workspace_id = $1 AND kind = $2",
            workspace_id,
            kind as WorkspaceBotKind
        )
        .fetch_optional(&mut *conn)
        .await?
        {
            return Ok(Ensured {
                user_id,
                created: false,
            });
        }

        let email = format!("workspace-bot-{}@bots.invalid", Uuid::new_v4().simple());
        let user_id = sqlx::query_scalar!(
            r"INSERT INTO users (email, display_name, avatar_url, status, is_bot)
              VALUES ($1, $2, $3, 'active', true)
              RETURNING id",
            email,
            identity.name,
            identity.avatar_url
        )
        .fetch_one(&mut *conn)
        .await?;
        sqlx::query!(
            "INSERT INTO workspace_members (workspace_id, user_id, role) VALUES ($1, $2, 'member')",
            workspace_id,
            user_id
        )
        .execute(&mut *conn)
        .await?;
        sqlx::query!(
            "INSERT INTO workspace_bots (workspace_id, kind, user_id) VALUES ($1, $2, $3)",
            workspace_id,
            kind as WorkspaceBotKind,
            user_id
        )
        .execute(&mut *conn)
        .await?;
        Ok(Ensured {
            user_id,
            created: true,
        })
    }

    pub async fn rename_in(conn: &mut PgConnection, user_id: Uuid, name: &str) -> sqlx::Result<()> {
        sqlx::query!(
            "UPDATE users SET display_name = $2, updated_at = NOW() WHERE id = $1 AND is_bot",
            user_id,
            name
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }
}
