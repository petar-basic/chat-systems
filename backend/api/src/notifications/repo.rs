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
            r#"UPDATE notifications SET is_read = true WHERE user_id = $1 AND workspace_id = $2 AND is_read = false AND data->>'channel_id' = $3 AND (data->>'thread_parent_id' IS NULL OR data @> '{"in_channel": true}')"#,
            user_id,
            workspace_id,
            channel_id.to_string()
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn mark_channels_read(&self, user_id: Uuid, workspace_id: Uuid) -> sqlx::Result<u64> {
        let result = sqlx::query!(
            r#"UPDATE notifications SET is_read = true WHERE user_id = $1 AND workspace_id = $2 AND is_read = false AND data->>'channel_id' IS NOT NULL AND (data->>'thread_parent_id' IS NULL OR data @> '{"in_channel": true}')"#,
            user_id,
            workspace_id
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn mark_thread_read(&self, user_id: Uuid, root_id: Uuid) -> sqlx::Result<u64> {
        let result = sqlx::query!(
            "UPDATE notifications SET is_read = true WHERE user_id = $1 AND is_read = false AND data->>'thread_parent_id' = $2",
            user_id,
            root_id.to_string()
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn dnd_status(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        at: DateTime<Utc>,
    ) -> sqlx::Result<DndStatus> {
        let row = sqlx::query_as!(
            DndStatus,
            r#"SELECT CASE WHEN dnd_until > $3 THEN dnd_until END AS dnd_until,
                      CASE WHEN dnd_override_until > $3 THEN NULL
                           ELSE public.schedule_paused_until(workspace_id, user_id, $3) END
                          AS schedule_paused_until
                 FROM workspace_members WHERE workspace_id = $1 AND user_id = $2"#,
            workspace_id,
            user_id,
            at
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.unwrap_or(DndStatus {
            dnd_until: None,
            schedule_paused_until: None,
        }))
    }

    pub async fn set_dnd(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        until: Option<DateTime<Utc>>,
    ) -> sqlx::Result<()> {
        self.set_dnd_at(workspace_id, user_id, until, Utc::now())
            .await
    }

    pub async fn set_dnd_at(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        until: Option<DateTime<Utc>>,
        at: DateTime<Utc>,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            "UPDATE workspace_members
                SET dnd_until = $3,
                    dnd_override_until = CASE WHEN $3::timestamptz IS NULL
                                              THEN public.schedule_paused_until(workspace_id, user_id, $4) END
              WHERE workspace_id = $1 AND user_id = $2",
            workspace_id,
            user_id,
            until,
            at
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn paused_until(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        at: DateTime<Utc>,
    ) -> sqlx::Result<Option<DateTime<Utc>>> {
        let row = sqlx::query_scalar!(
            "SELECT public.notifications_paused_until($1, $2, $3)",
            workspace_id,
            user_id,
            at
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn is_dnd_active(&self, workspace_id: Uuid, user_id: Uuid) -> sqlx::Result<bool> {
        Ok(self
            .paused_until(workspace_id, user_id, Utc::now())
            .await?
            .is_some())
    }

    pub async fn schedule(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<NotificationSchedule> {
        sqlx::query_as!(
            NotificationSchedule,
            r#"SELECT COALESCE(s.enabled, FALSE) AS "enabled!",
                      COALESCE(s.days, ARRAY[1, 2, 3, 4, 5]::SMALLINT[]) AS "days!",
                      COALESCE(s.start_minute, 540::SMALLINT) AS "start_minute!",
                      COALESCE(s.end_minute, 1080::SMALLINT) AS "end_minute!",
                      COALESCE(NULLIF(u.timezone, ''), 'UTC') AS "timezone!"
                 FROM users u
                 LEFT JOIN notification_schedules s
                   ON s.user_id = u.id AND s.workspace_id = $1
                WHERE u.id = $2"#,
            workspace_id,
            user_id
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn set_schedule(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        enabled: bool,
        days: &[i16],
        start_minute: i16,
        end_minute: i16,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            "INSERT INTO notification_schedules
                 (workspace_id, user_id, enabled, days, start_minute, end_minute)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (workspace_id, user_id) DO UPDATE
                SET enabled = EXCLUDED.enabled, days = EXCLUDED.days,
                    start_minute = EXCLUDED.start_minute, end_minute = EXCLUDED.end_minute,
                    updated_at = NOW()",
            workspace_id,
            user_id,
            enabled,
            days,
            start_minute,
            end_minute
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn mention_emails(&self, workspace_id: Uuid, user_id: Uuid) -> sqlx::Result<bool> {
        let enabled = sqlx::query_scalar!(
            "SELECT mention_emails FROM workspace_members WHERE workspace_id = $1 AND user_id = $2",
            workspace_id,
            user_id
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(enabled.unwrap_or(false))
    }

    pub async fn set_mention_emails(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        enabled: bool,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            "UPDATE workspace_members SET mention_emails = $3 WHERE workspace_id = $1 AND user_id = $2",
            workspace_id,
            user_id,
            enabled
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn recipients(
        &self,
        channel_id: Uuid,
        author_id: Option<Uuid>,
    ) -> sqlx::Result<Vec<Recipient>> {
        sqlx::query_as!(
            Recipient,
            r#"
            SELECT cm.user_id,
                   effective_notify_level(cm.notify_level, wm.notify_default, c.channel_type)
                       AS "level!: NotifyLevel",
                   cm.muted AS "muted!",
                   (c.channel_type IN ('dm', 'group_dm')) AS "direct!"
              FROM channel_members cm
              JOIN channels c ON c.id = cm.channel_id
              JOIN workspace_members wm
                ON wm.workspace_id = c.workspace_id AND wm.user_id = cm.user_id
             WHERE cm.channel_id = $1 AND cm.user_id IS DISTINCT FROM $2
            "#,
            channel_id,
            author_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn set_channel_level(
        &self,
        channel_id: Uuid,
        user_id: Uuid,
        level: Option<NotifyLevel>,
    ) -> sqlx::Result<Option<ChannelNotifyLevel>> {
        sqlx::query_as!(
            ChannelNotifyLevel,
            r#"
            UPDATE channel_members cm
               SET notify_level = $3
              FROM channels c, workspace_members wm
             WHERE cm.channel_id = $1 AND cm.user_id = $2
               AND c.id = cm.channel_id
               AND wm.workspace_id = c.workspace_id AND wm.user_id = cm.user_id
            RETURNING cm.notify_level AS "notify_level: NotifyLevel",
                      effective_notify_level(cm.notify_level, wm.notify_default, c.channel_type)
                          AS "effective_notify_level!: NotifyLevel"
            "#,
            channel_id,
            user_id,
            level as Option<NotifyLevel>
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn get_default(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<Option<NotifyLevel>> {
        sqlx::query_scalar!(
            r#"SELECT notify_default AS "notify_default: NotifyLevel"
                 FROM workspace_members WHERE workspace_id = $1 AND user_id = $2"#,
            workspace_id,
            user_id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn set_default(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        level: NotifyLevel,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            "UPDATE workspace_members SET notify_default = $3 WHERE workspace_id = $1 AND user_id = $2",
            workspace_id,
            user_id,
            level as NotifyLevel
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn keywords(&self, workspace_id: Uuid, user_id: Uuid) -> sqlx::Result<Vec<String>> {
        sqlx::query_scalar!(
            "SELECT keyword FROM notification_keywords
              WHERE workspace_id = $1 AND user_id = $2
              ORDER BY position",
            workspace_id,
            user_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn set_keywords(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        keywords: &[String],
    ) -> sqlx::Result<Vec<String>> {
        let mut tx = self.pool.begin().await?;
        sqlx::query!(
            "DELETE FROM notification_keywords WHERE workspace_id = $1 AND user_id = $2",
            workspace_id,
            user_id
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            r#"INSERT INTO notification_keywords (user_id, workspace_id, keyword, normalized, position)
               SELECT $2, $1, btrim(t.k), lower(public.search_normalize(btrim(t.k))), t.ord::int
                 FROM unnest($3::text[]) WITH ORDINALITY AS t(k, ord)
               ON CONFLICT DO NOTHING"#,
            workspace_id,
            user_id,
            keywords
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        self.keywords(workspace_id, user_id).await
    }

    pub async fn keyword_matches(
        &self,
        channel_id: Uuid,
        author_id: Option<Uuid>,
        content: &str,
    ) -> sqlx::Result<Vec<KeywordMatch>> {
        sqlx::query_as!(
            KeywordMatch,
            r#"SELECT DISTINCT ON (k.user_id) k.user_id AS "user_id!", k.keyword AS "keyword!"
                 FROM notification_keywords k
                 JOIN channels c ON c.id = $1 AND c.workspace_id = k.workspace_id
                 JOIN channel_members cm ON cm.channel_id = c.id AND cm.user_id = k.user_id
                WHERE k.user_id IS DISTINCT FROM $2
                  AND lower(public.search_normalize($3::text)) ~ public.keyword_pattern(k.normalized)
                ORDER BY k.user_id, k.position"#,
            channel_id,
            author_id,
            content
        )
        .fetch_all(&self.pool)
        .await
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

    pub async fn unread_count_everywhere(&self, user_id: Uuid) -> sqlx::Result<i64> {
        sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "count!" FROM notifications n
                JOIN workspace_members wm ON wm.workspace_id = n.workspace_id AND wm.user_id = n.user_id
               WHERE n.user_id = $1 AND n.is_read = false"#,
            user_id
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
        sqlx::query(
            "INSERT INTO workspace_members (workspace_id, user_id, role) VALUES ($1, $2, 'owner')",
        )
        .bind(row.0)
        .bind(owner_id)
        .execute(pool)
        .await
        .expect("insert member");
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
        let root = Uuid::new_v4();
        repo.create(
            user_id,
            workspace_id,
            &NotificationType::Reply,
            "New reply",
            Some("in a thread of A"),
            &serde_json::json!({ "channel_id": channel_a.to_string(), "thread_parent_id": root.to_string() }),
        )
        .await
        .expect("create thread reply");

        assert_eq!(
            repo.unread_count(user_id, workspace_id)
                .await
                .expect("count before"),
            3,
        );

        let updated = repo
            .mark_channel_read(user_id, workspace_id, channel_a)
            .await
            .expect("mark_channel_read");
        assert_eq!(
            updated, 1,
            "only channel A's own notification should be marked read, not its thread's"
        );

        assert_eq!(
            repo.unread_count(user_id, workspace_id)
                .await
                .expect("count after"),
            2,
            "channel B's notification and the thread's must remain unread",
        );

        assert_eq!(
            repo.mark_thread_read(user_id, root)
                .await
                .expect("mark_thread_read"),
            1,
            "reading the thread clears its notification",
        );
        assert_eq!(
            repo.unread_count_everywhere(user_id)
                .await
                .expect("count everywhere"),
            1,
        );
    }
}
