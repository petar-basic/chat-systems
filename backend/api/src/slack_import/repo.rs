use sqlx::PgPool;
use uuid::Uuid;

pub struct SlackImportRepo {
    pool: PgPool,
}

impl SlackImportRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn start_run(
        &self,
        workspace_id: Uuid,
        source: &str,
        dry_run: bool,
    ) -> sqlx::Result<Uuid> {
        sqlx::query_scalar(
            r"
            INSERT INTO slack_imports (workspace_id, source, dry_run)
            VALUES ($1, $2, $3)
            RETURNING id
            ",
        )
        .bind(workspace_id)
        .bind(source)
        .bind(dry_run)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn finish_run(&self, id: Uuid, report: &serde_json::Value) -> sqlx::Result<()> {
        sqlx::query("UPDATE slack_imports SET report = $2, finished_at = NOW() WHERE id = $1")
            .bind(id)
            .bind(report)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn user_mappings(
        &self,
        workspace_id: Uuid,
    ) -> sqlx::Result<Vec<(String, Uuid, String)>> {
        sqlx::query_as(
            r"
            SELECT su.slack_user_id, su.user_id, COALESCE(u.display_name, u.email)
            FROM slack_users su
            JOIN users u ON u.id = su.user_id
            WHERE su.workspace_id = $1
            ",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn map_user(
        &self,
        workspace_id: Uuid,
        slack_user_id: &str,
        user_id: Uuid,
    ) -> sqlx::Result<()> {
        sqlx::query(
            r"
            INSERT INTO slack_users (workspace_id, slack_user_id, user_id)
            VALUES ($1, $2, $3)
            ON CONFLICT (workspace_id, slack_user_id) DO UPDATE SET user_id = $3
            ",
        )
        .bind(workspace_id)
        .bind(slack_user_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn channel_mappings(&self, workspace_id: Uuid) -> sqlx::Result<Vec<(String, Uuid)>> {
        sqlx::query_as(
            "SELECT slack_channel_id, channel_id FROM slack_channels WHERE workspace_id = $1",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn map_channel(
        &self,
        workspace_id: Uuid,
        slack_channel_id: &str,
        channel_id: Uuid,
    ) -> sqlx::Result<()> {
        sqlx::query(
            r"
            INSERT INTO slack_channels (workspace_id, slack_channel_id, channel_id)
            VALUES ($1, $2, $3)
            ON CONFLICT (workspace_id, slack_channel_id) DO UPDATE SET channel_id = $3
            ",
        )
        .bind(workspace_id)
        .bind(slack_channel_id)
        .bind(channel_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Everything this channel already holds from a previous run, keyed by the
    /// Slack timestamp it came from. One query per channel rather than one per
    /// message: a resumed import of 200k messages should not be 200k lookups.
    pub async fn imported_message_ids(
        &self,
        channel_id: Uuid,
    ) -> sqlx::Result<Vec<(String, Uuid)>> {
        sqlx::query_as(
            "SELECT slack_ts, id FROM messages WHERE channel_id = $1 AND slack_ts IS NOT NULL",
        )
        .bind(channel_id)
        .fetch_all(&self.pool)
        .await
    }
}
