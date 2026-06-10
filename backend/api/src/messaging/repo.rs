use sqlx::PgPool;
use uuid::Uuid;

use super::models::*;

pub struct MessageRepo {
    pool: PgPool,
}

pub struct MessageSearch<'a> {
    pub query: &'a str,
    pub workspace_id: Uuid,
    pub requester_id: Uuid,
    pub channel_id: Option<Uuid>,
    pub author_id: Option<Uuid>,
    pub limit: i64,
    pub offset: i64,
}

impl MessageRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_message(
        &self,
        channel_id: Uuid,
        user_id: Uuid,
        content: &str,
        thread_parent_id: Option<Uuid>,
    ) -> sqlx::Result<Message> {
        let mut tx = self.pool.begin().await?;

        let msg = sqlx::query_as::<_, Message>(
            r#"
            INSERT INTO messages (channel_id, user_id, content, thread_parent_id)
            VALUES ($1, $2, $3, $4)
            RETURNING *
            "#,
        )
        .bind(channel_id)
        .bind(user_id)
        .bind(content)
        .bind(thread_parent_id)
        .fetch_one(&mut *tx)
        .await?;

        if let Some(parent_id) = thread_parent_id {
            sqlx::query("UPDATE messages SET reply_count = reply_count + 1 WHERE id = $1")
                .bind(parent_id)
                .execute(&mut *tx)
                .await?;
        }

        tx.commit().await?;

        Ok(msg)
    }

    pub async fn create_message_with_id(
        &self,
        id: Uuid,
        channel_id: Uuid,
        user_id: Uuid,
        content: &str,
        thread_parent_id: Option<Uuid>,
    ) -> sqlx::Result<Message> {
        let mut tx = self.pool.begin().await?;

        let msg = sqlx::query_as::<_, Message>(
            r#"
            INSERT INTO messages (id, channel_id, user_id, content, thread_parent_id)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(channel_id)
        .bind(user_id)
        .bind(content)
        .bind(thread_parent_id)
        .fetch_one(&mut *tx)
        .await?;

        if let Some(parent_id) = thread_parent_id {
            sqlx::query("UPDATE messages SET reply_count = reply_count + 1 WHERE id = $1")
                .bind(parent_id)
                .execute(&mut *tx)
                .await?;
        }

        tx.commit().await?;

        Ok(msg)
    }

    pub async fn create_system_message(
        &self,
        channel_id: Uuid,
        user_id: Uuid,
        content: &str,
        metadata: serde_json::Value,
    ) -> sqlx::Result<Message> {
        sqlx::query_as::<_, Message>(
            r#"
            INSERT INTO messages (channel_id, user_id, content, metadata)
            VALUES ($1, $2, $3, $4)
            RETURNING *
            "#,
        )
        .bind(channel_id)
        .bind(user_id)
        .bind(content)
        .bind(metadata)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn find_by_id(&self, id: Uuid) -> sqlx::Result<Option<Message>> {
        sqlx::query_as::<_, Message>("SELECT * FROM messages WHERE id = $1 AND deleted_at IS NULL")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn list_channel_messages(
        &self,
        channel_id: Uuid,
        limit: i64,
        before: Option<Uuid>,
    ) -> sqlx::Result<Vec<Message>> {
        if let Some(cursor) = before {
            sqlx::query_as::<_, Message>(
                r#"
                SELECT * FROM messages
                WHERE channel_id = $1
                  AND deleted_at IS NULL
                  AND thread_parent_id IS NULL
                  AND created_at < (SELECT created_at FROM messages WHERE id = $3)
                ORDER BY created_at DESC
                LIMIT $2
                "#,
            )
            .bind(channel_id)
            .bind(limit)
            .bind(cursor)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as::<_, Message>(
                r#"
                SELECT * FROM messages
                WHERE channel_id = $1
                  AND deleted_at IS NULL
                  AND thread_parent_id IS NULL
                ORDER BY created_at DESC
                LIMIT $2
                "#,
            )
            .bind(channel_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
        }
    }

    pub async fn list_thread_messages(
        &self,
        parent_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> sqlx::Result<Vec<Message>> {
        sqlx::query_as::<_, Message>(
            r#"
            SELECT * FROM messages
            WHERE thread_parent_id = $1 AND deleted_at IS NULL
            ORDER BY created_at ASC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(parent_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn update_message(&self, id: Uuid, content: &str) -> sqlx::Result<Message> {
        sqlx::query_as::<_, Message>(
            r#"
            UPDATE messages SET content = $2, updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(content)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn soft_delete_message(&self, id: Uuid) -> sqlx::Result<()> {
        sqlx::query("UPDATE messages SET deleted_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_pinned(&self, id: Uuid, pinned: bool) -> sqlx::Result<Message> {
        sqlx::query_as::<_, Message>(
            "UPDATE messages SET is_pinned = $2, updated_at = NOW() WHERE id = $1 RETURNING *",
        )
        .bind(id)
        .bind(pinned)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn list_pinned(&self, channel_id: Uuid) -> sqlx::Result<Vec<Message>> {
        sqlx::query_as::<_, Message>(
            "SELECT * FROM messages WHERE channel_id = $1 AND is_pinned = true AND deleted_at IS NULL ORDER BY updated_at DESC",
        )
        .bind(channel_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn add_reaction(
        &self,
        message_id: Uuid,
        user_id: Uuid,
        emoji: &str,
    ) -> sqlx::Result<Reaction> {
        sqlx::query_as::<_, Reaction>(
            r#"
            INSERT INTO reactions (message_id, user_id, emoji)
            VALUES ($1, $2, $3)
            RETURNING *
            "#,
        )
        .bind(message_id)
        .bind(user_id)
        .bind(emoji)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn remove_reaction(
        &self,
        message_id: Uuid,
        user_id: Uuid,
        emoji: &str,
    ) -> sqlx::Result<()> {
        sqlx::query("DELETE FROM reactions WHERE message_id = $1 AND user_id = $2 AND emoji = $3")
            .bind(message_id)
            .bind(user_id)
            .bind(emoji)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn list_reactions(&self, message_id: Uuid) -> sqlx::Result<Vec<Reaction>> {
        sqlx::query_as::<_, Reaction>(
            "SELECT * FROM reactions WHERE message_id = $1 ORDER BY created_at",
        )
        .bind(message_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn list_reactions_for_messages(
        &self,
        message_ids: &[Uuid],
    ) -> sqlx::Result<Vec<Reaction>> {
        if message_ids.is_empty() {
            return Ok(vec![]);
        }
        sqlx::query_as::<_, Reaction>(
            "SELECT * FROM reactions WHERE message_id = ANY($1) ORDER BY created_at",
        )
        .bind(message_ids)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn search(&self, params: MessageSearch<'_>) -> sqlx::Result<Vec<Message>> {
        sqlx::query_as::<_, Message>(
            r#"
            SELECT m.* FROM messages m
            JOIN channels c ON c.id = m.channel_id
            WHERE m.content_search @@ plainto_tsquery('english', $1)
              AND c.workspace_id = $2
              AND m.deleted_at IS NULL
              AND (
                c.channel_type = 'public'
                OR EXISTS (
                  SELECT 1 FROM channel_members cm
                  WHERE cm.channel_id = c.id AND cm.user_id = $3
                )
              )
              AND ($4::uuid IS NULL OR m.channel_id = $4)
              AND ($5::uuid IS NULL OR m.user_id = $5)
            ORDER BY ts_rank(m.content_search, plainto_tsquery('english', $1)) DESC
            LIMIT $6 OFFSET $7
            "#,
        )
        .bind(params.query)
        .bind(params.workspace_id)
        .bind(params.requester_id)
        .bind(params.channel_id)
        .bind(params.author_id)
        .bind(params.limit)
        .bind(params.offset)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn is_channel_admin(&self, channel_id: Uuid, user_id: Uuid) -> sqlx::Result<bool> {
        let row: (bool,) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM channel_members WHERE channel_id = $1 AND user_id = $2 AND role = 'admin')",
        )
        .bind(channel_id)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }

    pub async fn is_workspace_admin_for_channel(
        &self,
        channel_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<bool> {
        let row: (bool,) = sqlx::query_as(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM workspace_members wm
                JOIN channels c ON c.workspace_id = wm.workspace_id
                WHERE c.id = $1 AND wm.user_id = $2 AND wm.role IN ('admin', 'owner')
            )
            "#,
        )
        .bind(channel_id)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }

    pub async fn can_moderate_channel(
        &self,
        channel_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<bool> {
        let ch_admin = self.is_channel_admin(channel_id, user_id).await?;
        if ch_admin {
            return Ok(true);
        }
        self.is_workspace_admin_for_channel(channel_id, user_id)
            .await
    }

    pub async fn mark_read(
        &self,
        channel_id: Uuid,
        user_id: Uuid,
        message_id: Uuid,
    ) -> sqlx::Result<()> {
        sqlx::query(
            r#"
            UPDATE channel_members
            SET last_read_at = NOW(), last_read_msg = $3
            WHERE channel_id = $1 AND user_id = $2
            "#,
        )
        .bind(channel_id)
        .bind(user_id)
        .bind(message_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::PgPool;

    async fn seed_channel(pool: &PgPool) -> (Uuid, Uuid) {
        let user_id = Uuid::new_v4();
        sqlx::query("INSERT INTO users (id, email, status) VALUES ($1, $2, 'active')")
            .bind(user_id)
            .bind(format!("u-{}@test.local", user_id))
            .execute(pool)
            .await
            .expect("insert user");

        let workspace_id = Uuid::new_v4();
        sqlx::query("INSERT INTO workspaces (id, name, slug, owner_id) VALUES ($1, $2, $3, $4)")
            .bind(workspace_id)
            .bind("Test WS")
            .bind(format!("ws-{}", workspace_id))
            .bind(user_id)
            .execute(pool)
            .await
            .expect("insert workspace");

        let channel_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO channels (id, workspace_id, name, channel_type, created_by) VALUES ($1, $2, $3, 'public', $4)",
        )
        .bind(channel_id)
        .bind(workspace_id)
        .bind("general")
        .bind(user_id)
        .execute(pool)
        .await
        .expect("insert channel");

        sqlx::query(
            "INSERT INTO channel_members (channel_id, user_id, role) VALUES ($1, $2, 'member')",
        )
        .bind(channel_id)
        .bind(user_id)
        .execute(pool)
        .await
        .expect("insert channel_member");

        (user_id, channel_id)
    }

    #[sqlx::test(migrations = "../migrations")]
    async fn create_reply_increments_parent_reply_count(pool: PgPool) {
        let (user_id, channel_id) = seed_channel(&pool).await;
        let repo = MessageRepo::new(pool);

        let parent = repo
            .create_message(channel_id, user_id, "parent message", None)
            .await
            .expect("create parent");
        assert_eq!(parent.reply_count, 0, "fresh parent must have 0 replies");
        assert!(
            parent.thread_parent_id.is_none(),
            "parent must not have a thread_parent_id"
        );

        let reply = repo
            .create_message(channel_id, user_id, "a reply", Some(parent.id))
            .await
            .expect("create reply");
        assert_eq!(
            reply.thread_parent_id,
            Some(parent.id),
            "reply must point at the parent"
        );

        let parent_after = repo
            .find_by_id(parent.id)
            .await
            .expect("query parent")
            .expect("parent still exists");
        assert_eq!(
            parent_after.reply_count, 1,
            "creating one thread reply must increment parent.reply_count by exactly 1"
        );
    }

    #[sqlx::test(migrations = "../migrations")]
    async fn soft_delete_filters_message(pool: PgPool) {
        let (user_id, channel_id) = seed_channel(&pool).await;
        let repo = MessageRepo::new(pool);

        let msg = repo
            .create_message(channel_id, user_id, "doomed message", None)
            .await
            .expect("create message");

        assert!(
            repo.find_by_id(msg.id).await.expect("find").is_some(),
            "message must be visible before soft delete"
        );
        let before = repo
            .list_channel_messages(channel_id, 50, None)
            .await
            .expect("list before");
        assert!(
            before.iter().any(|m| m.id == msg.id),
            "message must appear in channel listing before soft delete"
        );

        repo.soft_delete_message(msg.id).await.expect("soft delete");

        assert!(
            repo.find_by_id(msg.id).await.expect("find after").is_none(),
            "find_by_id must return None after soft delete (deleted_at IS NOT NULL)"
        );

        let after = repo
            .list_channel_messages(channel_id, 50, None)
            .await
            .expect("list after");
        assert!(
            !after.iter().any(|m| m.id == msg.id),
            "soft-deleted message must not appear in channel listing"
        );
    }

    #[sqlx::test(migrations = "../migrations")]
    async fn cursor_pagination_orders_newest_first_without_overlap(pool: PgPool) {
        let (user_id, channel_id) = seed_channel(&pool).await;
        let seed_pool = pool.clone();
        let repo = MessageRepo::new(pool);

        let base = chrono::Utc::now() - chrono::Duration::minutes(10);
        let mut ids = Vec::new();
        for i in 0..5i64 {
            let id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO messages (id, channel_id, user_id, content, created_at) VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(id)
            .bind(channel_id)
            .bind(user_id)
            .bind(format!("msg {}", i))
            .bind(base + chrono::Duration::seconds(i))
            .execute(&seed_pool)
            .await
            .expect("insert message");
            ids.push(id);
        }

        let page1 = repo
            .list_channel_messages(channel_id, 2, None)
            .await
            .expect("page1");
        assert_eq!(page1.len(), 2, "first page must hold exactly the limit");
        assert_eq!(page1[0].id, ids[4], "first item must be the newest message");
        assert_eq!(page1[1].id, ids[3], "second item must be the next-newest");
        assert!(
            page1[0].created_at > page1[1].created_at,
            "results must be ordered newest-first by created_at"
        );

        let page2 = repo
            .list_channel_messages(channel_id, 2, Some(page1[1].id))
            .await
            .expect("page2");
        assert_eq!(page2.len(), 2, "second page must hold exactly the limit");
        assert_eq!(
            page2[0].id, ids[2],
            "page2 must continue strictly after the cursor"
        );
        assert_eq!(page2[1].id, ids[1]);

        for m in &page2 {
            assert!(
                !page1.iter().any(|p| p.id == m.id),
                "cursor pagination must not return overlapping rows across pages"
            );
        }

        let page3 = repo
            .list_channel_messages(channel_id, 2, Some(page2[1].id))
            .await
            .expect("page3");
        assert_eq!(
            page3.len(),
            1,
            "final page must contain only the remaining row"
        );
        assert_eq!(page3[0].id, ids[0], "final row must be the oldest message");

        let mut seen: Vec<Uuid> = page1
            .iter()
            .chain(page2.iter())
            .chain(page3.iter())
            .map(|m| m.id)
            .collect();
        seen.sort();
        let mut expected = ids.clone();
        expected.sort();
        assert_eq!(
            seen, expected,
            "pagination must cover every row exactly once"
        );
    }
}
