use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::models::*;

#[derive(Clone)]
pub struct NotificationRepo {
    pool: PgPool,
}

impl NotificationRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        notification_type: &NotificationType,
        title: &str,
        body: Option<&str>,
        data: &serde_json::Value,
    ) -> sqlx::Result<Option<Notification>> {
        sqlx::query_as!(
            Notification,
            r#"
            INSERT INTO notifications (user_id, workspace_id, notification_type, title, body, data)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT DO NOTHING
            RETURNING id, user_id, workspace_id, notification_type AS "notification_type: NotificationType",
                   title, body, data AS "data!", is_read AS "is_read!", created_at
            "#,
            user_id,
            workspace_id,
            notification_type.clone() as NotificationType,
            title,
            body,
            data
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn list_for_user(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> sqlx::Result<Vec<Notification>> {
        sqlx::query_as!(
            Notification,
            r#"
            SELECT id, user_id, workspace_id, notification_type AS "notification_type: NotificationType",
                   title, body, data AS "data!", is_read AS "is_read!", created_at
              FROM notifications
             WHERE user_id = $1 AND workspace_id = $2
             ORDER BY created_at DESC
             LIMIT $3 OFFSET $4
            "#,
            user_id,
            workspace_id,
            limit,
            offset
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn mark_read(&self, notification_ids: &[Uuid], user_id: Uuid) -> sqlx::Result<u64> {
        let result = sqlx::query!(
            "UPDATE notifications SET is_read = true WHERE id = ANY($1) AND user_id = $2",
            notification_ids,
            user_id
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn mark_all_read(&self, user_id: Uuid, workspace_id: Uuid) -> sqlx::Result<u64> {
        let result = sqlx::query!(
            "UPDATE notifications SET is_read = true WHERE user_id = $1 AND workspace_id = $2 AND is_read = false",
            user_id,
            workspace_id
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn mark_channel_read(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        channel_id: Uuid,
    ) -> sqlx::Result<u64> {
        let result = sqlx::query!(
            "UPDATE notifications SET is_read = true WHERE user_id = $1 AND workspace_id = $2 AND is_read = false AND data->>'channel_id' = $3",
            user_id,
            workspace_id,
            channel_id.to_string()
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn get_dnd(&self, user_id: Uuid) -> sqlx::Result<Option<DateTime<Utc>>> {
        let row = sqlx::query_scalar!("SELECT dnd_until FROM users WHERE id = $1", user_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.flatten())
    }

    pub async fn set_dnd(&self, user_id: Uuid, until: Option<DateTime<Utc>>) -> sqlx::Result<()> {
        sqlx::query!(
            "UPDATE users SET dnd_until = $2 WHERE id = $1",
            user_id,
            until
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn is_dnd_active(&self, user_id: Uuid) -> sqlx::Result<bool> {
        let row = sqlx::query_scalar!(
            r#"SELECT (dnd_until IS NOT NULL AND dnd_until > NOW()) AS "active!" FROM users WHERE id = $1"#,
            user_id
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.unwrap_or(false))
    }

    pub async fn is_channel_muted(&self, channel_id: Uuid, user_id: Uuid) -> sqlx::Result<bool> {
        let row = sqlx::query_scalar!(
            "SELECT muted FROM channel_members WHERE channel_id = $1 AND user_id = $2",
            channel_id,
            user_id
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.unwrap_or(false))
    }

    pub async fn unread_count(&self, user_id: Uuid, workspace_id: Uuid) -> sqlx::Result<i64> {
        sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "count!" FROM notifications WHERE user_id = $1 AND workspace_id = $2 AND is_read = false"#,
            user_id,
            workspace_id
        )
        .fetch_one(&self.pool)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::PgPool;

    async fn insert_user(pool: &PgPool, email: &str) -> Uuid {
        let row: (Uuid,) =
            sqlx::query_as("INSERT INTO users (email, status) VALUES ($1, 'active') RETURNING id")
                .bind(email)
                .fetch_one(pool)
                .await
                .expect("insert user");
        row.0
    }

    async fn insert_workspace(pool: &PgPool, owner_id: Uuid, slug: &str) -> Uuid {
        let row: (Uuid,) = sqlx::query_as(
            "INSERT INTO workspaces (name, slug, owner_id) VALUES ($1, $2, $3) RETURNING id",
        )
        .bind("Test Workspace")
        .bind(slug)
        .bind(owner_id)
        .fetch_one(pool)
        .await
        .expect("insert workspace");
        row.0
    }

    #[test_macros::db_test(migrations = "../migrations")]
    async fn create_persists_list_counts_and_mark_read_clears_unread(pool: PgPool) {
        let user_id = insert_user(&pool, "notif-user@test.local").await;
        let workspace_id = insert_workspace(&pool, user_id, "notif-ws").await;
        let repo = NotificationRepo::new(pool.clone());

        let data = serde_json::json!({ "message_id": "abc" });
        let created = repo
            .create(
                user_id,
                workspace_id,
                &NotificationType::Dm,
                "New DM",
                Some("hello there"),
                &data,
            )
            .await
            .expect("create should persist the notification")
            .expect("a first insert must not be deduplicated away");

        assert_eq!(created.user_id, user_id);
        assert_eq!(created.workspace_id, workspace_id);
        assert_eq!(created.notification_type, NotificationType::Dm);
        assert_eq!(created.title, "New DM");
        assert_eq!(created.body.as_deref(), Some("hello there"));
        assert!(
            !created.is_read,
            "a freshly created notification must be unread"
        );

        let db_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM notifications WHERE id = $1")
            .bind(created.id)
            .fetch_one(&pool)
            .await
            .expect("count by id");
        assert_eq!(db_count.0, 1, "create() must write a row to notifications");

        let listed = repo
            .list_for_user(user_id, workspace_id, 50, 0)
            .await
            .expect("list_for_user");
        assert_eq!(listed.len(), 1, "the created notification must be listed");
        assert_eq!(listed[0].id, created.id);

        let before = repo
            .unread_count(user_id, workspace_id)
            .await
            .expect("unread_count before");
        assert_eq!(before, 1, "the single unread notification must be counted");

        let affected = repo
            .mark_read(&[created.id], user_id)
            .await
            .expect("mark_read");
        assert_eq!(
            affected, 1,
            "exactly one notification should be marked read"
        );

        let after = repo
            .unread_count(user_id, workspace_id)
            .await
            .expect("unread_count after");
        assert_eq!(
            after, 0,
            "marking the only notification read must drop the unread count to 0"
        );

        let is_read: (bool,) = sqlx::query_as("SELECT is_read FROM notifications WHERE id = $1")
            .bind(created.id)
            .fetch_one(&pool)
            .await
            .expect("fetch is_read");
        assert!(is_read.0, "mark_read must persist is_read = true");
    }

    #[test_macros::db_test(migrations = "../migrations")]
    async fn mark_channel_read_clears_only_that_channels_notifications(pool: PgPool) {
        let user_id = insert_user(&pool, "chan-notif-user@test.local").await;
        let workspace_id = insert_workspace(&pool, user_id, "chan-notif-ws").await;
        let repo = NotificationRepo::new(pool.clone());

        let channel_a = Uuid::new_v4();
        let channel_b = Uuid::new_v4();

        repo.create(
            user_id,
            workspace_id,
            &NotificationType::Mention,
            "You were mentioned",
            Some("in A"),
            &serde_json::json!({ "channel_id": channel_a.to_string() }),
        )
        .await
        .expect("create A");
        repo.create(
            user_id,
            workspace_id,
            &NotificationType::Mention,
            "You were mentioned",
            Some("in B"),
            &serde_json::json!({ "channel_id": channel_b.to_string() }),
        )
        .await
        .expect("create B");

        assert_eq!(
            repo.unread_count(user_id, workspace_id)
                .await
                .expect("count before"),
            2,
        );

        let updated = repo
            .mark_channel_read(user_id, workspace_id, channel_a)
            .await
            .expect("mark_channel_read");
        assert_eq!(
            updated, 1,
            "only channel A's notification should be marked read"
        );

        assert_eq!(
            repo.unread_count(user_id, workspace_id)
                .await
                .expect("count after"),
            1,
            "channel B's notification must remain unread",
        );
    }
}
