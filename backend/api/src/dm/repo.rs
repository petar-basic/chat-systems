use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::models::{DirectMessage, DmConversation, DmReaction};

pub struct DmRepo {
    pool: PgPool,
}

impl DmRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list_conversations(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> sqlx::Result<Vec<DmConversation>> {
        sqlx::query_as::<_, DmConversation>(
            r"
            SELECT c.partner_id, c.last_message_at, r.last_read_at
            FROM (
                SELECT
                    CASE WHEN from_user_id = $2 THEN to_user_id ELSE from_user_id END AS partner_id,
                    MAX(created_at) AS last_message_at
                FROM direct_messages
                WHERE workspace_id = $1
                    AND (from_user_id = $2 OR to_user_id = $2)
                    AND deleted_at IS NULL
                GROUP BY partner_id
            ) c
            LEFT JOIN dm_reads r
                ON r.user_id = $2 AND r.workspace_id = $1 AND r.partner_id = c.partner_id
            ORDER BY c.last_message_at DESC
            ",
        )
        .bind(workspace_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn add_reaction(
        &self,
        message_id: Uuid,
        user_id: Uuid,
        emoji: &str,
    ) -> sqlx::Result<DmReaction> {
        sqlx::query_as::<_, DmReaction>(
            r"
            INSERT INTO dm_reactions (message_id, user_id, emoji)
            VALUES ($1, $2, $3)
            RETURNING *
            ",
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
        sqlx::query(
            "DELETE FROM dm_reactions WHERE message_id = $1 AND user_id = $2 AND emoji = $3",
        )
        .bind(message_id)
        .bind(user_id)
        .bind(emoji)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_reactions_for_messages(
        &self,
        message_ids: &[Uuid],
    ) -> sqlx::Result<Vec<DmReaction>> {
        if message_ids.is_empty() {
            return Ok(vec![]);
        }
        sqlx::query_as::<_, DmReaction>(
            "SELECT * FROM dm_reactions WHERE message_id = ANY($1) ORDER BY created_at",
        )
        .bind(message_ids)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn mark_read(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        partner_id: Uuid,
    ) -> sqlx::Result<()> {
        sqlx::query(
            r"
            INSERT INTO dm_reads (user_id, workspace_id, partner_id, last_read_at)
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (user_id, workspace_id, partner_id)
            DO UPDATE SET last_read_at = NOW()
            ",
        )
        .bind(user_id)
        .bind(workspace_id)
        .bind(partner_id)
        .execute(&self.pool)
        .await
        .map(|_| ())
    }

    pub async fn list_messages(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        partner_id: Uuid,
        limit: i64,
        before: Option<DateTime<Utc>>,
    ) -> sqlx::Result<Vec<DirectMessage>> {
        sqlx::query_as::<_, DirectMessage>(
            r"
            SELECT * FROM direct_messages
            WHERE workspace_id = $1
                AND (
                    (from_user_id = $2 AND to_user_id = $3)
                    OR (from_user_id = $3 AND to_user_id = $2)
                )
                AND deleted_at IS NULL
                AND ($4::timestamptz IS NULL OR created_at < $4)
            ORDER BY created_at DESC
            LIMIT $5
            ",
        )
        .bind(workspace_id)
        .bind(user_id)
        .bind(partner_id)
        .bind(before)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn create(
        &self,
        id: Uuid,
        workspace_id: Uuid,
        from_user_id: Uuid,
        to_user_id: Uuid,
        content: &str,
    ) -> sqlx::Result<DirectMessage> {
        sqlx::query_as::<_, DirectMessage>(
            r"
            INSERT INTO direct_messages (id, workspace_id, from_user_id, to_user_id, content)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            ",
        )
        .bind(id)
        .bind(workspace_id)
        .bind(from_user_id)
        .bind(to_user_id)
        .bind(content)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_by_id(&self, id: Uuid) -> sqlx::Result<Option<DirectMessage>> {
        sqlx::query_as::<_, DirectMessage>(
            "SELECT * FROM direct_messages WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn update(&self, id: Uuid, content: &str) -> sqlx::Result<DirectMessage> {
        sqlx::query_as::<_, DirectMessage>(
            r"
            UPDATE direct_messages
            SET content = $2, edited_at = NOW(), updated_at = NOW()
            WHERE id = $1 AND deleted_at IS NULL
            RETURNING *
            ",
        )
        .bind(id)
        .bind(content)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn soft_delete(&self, id: Uuid) -> sqlx::Result<DirectMessage> {
        sqlx::query_as::<_, DirectMessage>(
            r"
            UPDATE direct_messages
            SET deleted_at = NOW(), updated_at = NOW()
            WHERE id = $1
            RETURNING *
            ",
        )
        .bind(id)
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

    #[sqlx::test(migrations = "../migrations")]
    async fn create_persists_dm_and_get_by_id_reads_it(pool: PgPool) {
        let alice = insert_user(&pool, "alice@test.local").await;
        let bob = insert_user(&pool, "bob@test.local").await;
        let workspace_id = insert_workspace(&pool, alice, "dm-ws-1").await;
        let repo = DmRepo::new(pool.clone());

        let id = Uuid::new_v4();
        let created = repo
            .create(id, workspace_id, alice, bob, "hi bob")
            .await
            .expect("create should persist the DM");

        assert_eq!(created.id, id);
        assert_eq!(created.from_user_id, alice);
        assert_eq!(created.to_user_id, bob);
        assert_eq!(created.content, "hi bob");
        assert!(created.deleted_at.is_none(), "a new DM is not soft-deleted");

        let db_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM direct_messages WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .expect("count by id");
        assert_eq!(
            db_count.0, 1,
            "create() must write a row to direct_messages"
        );

        let fetched = repo
            .get_by_id(id)
            .await
            .expect("get_by_id")
            .expect("DM should exist");
        assert_eq!(fetched.id, id);
        assert_eq!(fetched.content, "hi bob");
    }

    #[sqlx::test(migrations = "../migrations")]
    async fn conversation_is_symmetric_and_deduped(pool: PgPool) {
        let alice = insert_user(&pool, "alice@test.local").await;
        let bob = insert_user(&pool, "bob@test.local").await;
        let workspace_id = insert_workspace(&pool, alice, "dm-ws-2").await;
        let repo = DmRepo::new(pool.clone());

        let m1 = repo
            .create(Uuid::new_v4(), workspace_id, alice, bob, "hi bob")
            .await
            .expect("alice -> bob");
        let m2 = repo
            .create(Uuid::new_v4(), workspace_id, bob, alice, "hey alice")
            .await
            .expect("bob -> alice");

        let alice_view = repo
            .list_messages(workspace_id, alice, bob, 50, None)
            .await
            .expect("alice list_messages");
        let alice_ids: Vec<Uuid> = alice_view.iter().map(|m| m.id).collect();
        assert_eq!(
            alice_view.len(),
            2,
            "Alice must see both messages of the conversation"
        );
        assert!(alice_ids.contains(&m1.id) && alice_ids.contains(&m2.id));

        let bob_view = repo
            .list_messages(workspace_id, bob, alice, 50, None)
            .await
            .expect("bob list_messages");
        let bob_ids: Vec<Uuid> = bob_view.iter().map(|m| m.id).collect();
        assert_eq!(bob_view.len(), 2, "Bob must see the identical conversation");
        assert!(bob_ids.contains(&m1.id) && bob_ids.contains(&m2.id));

        let alice_convos = repo
            .list_conversations(workspace_id, alice)
            .await
            .expect("alice list_conversations");
        assert_eq!(
            alice_convos.len(),
            1,
            "the two-way exchange is one conversation for Alice"
        );
        assert_eq!(alice_convos[0].partner_id, bob, "Alice's partner is Bob");

        let bob_convos = repo
            .list_conversations(workspace_id, bob)
            .await
            .expect("bob list_conversations");
        assert_eq!(
            bob_convos.len(),
            1,
            "the two-way exchange is one conversation for Bob"
        );
        assert_eq!(bob_convos[0].partner_id, alice, "Bob's partner is Alice");
    }

    #[sqlx::test(migrations = "../migrations")]
    async fn soft_deleted_dm_is_excluded_from_get_by_id(pool: PgPool) {
        let alice = insert_user(&pool, "alice@test.local").await;
        let bob = insert_user(&pool, "bob@test.local").await;
        let workspace_id = insert_workspace(&pool, alice, "dm-ws-3").await;
        let repo = DmRepo::new(pool.clone());

        let doomed = repo
            .create(Uuid::new_v4(), workspace_id, alice, bob, "delete me")
            .await
            .expect("create doomed DM");
        let survivor = repo
            .create(Uuid::new_v4(), workspace_id, alice, bob, "keep me")
            .await
            .expect("create surviving DM");

        assert!(
            repo.get_by_id(doomed.id)
                .await
                .expect("get_by_id pre-delete")
                .is_some(),
            "DM must be readable before soft-delete"
        );

        let affected = sqlx::query("UPDATE direct_messages SET deleted_at = NOW() WHERE id = $1")
            .bind(doomed.id)
            .execute(&pool)
            .await
            .expect("soft-delete update")
            .rows_affected();
        assert_eq!(affected, 1, "exactly the doomed DM is soft-deleted");

        let after = repo
            .get_by_id(doomed.id)
            .await
            .expect("get_by_id post-delete");
        assert!(
            after.is_none(),
            "a soft-deleted DM must be excluded from get_by_id"
        );

        let still_present: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM direct_messages WHERE id = $1")
                .bind(doomed.id)
                .fetch_one(&pool)
                .await
                .expect("count doomed row");
        assert_eq!(
            still_present.0, 1,
            "soft-delete keeps the row physically present"
        );

        assert!(
            repo.get_by_id(survivor.id)
                .await
                .expect("get_by_id survivor")
                .is_some(),
            "the non-deleted DM must remain readable"
        );
        let listed = repo
            .list_messages(workspace_id, alice, bob, 50, None)
            .await
            .expect("list_messages after soft-delete");
        assert_eq!(
            listed.len(),
            1,
            "list_messages must exclude the soft-deleted DM"
        );
        assert_eq!(
            listed[0].id, survivor.id,
            "only the surviving DM remains in the conversation"
        );
    }
}
