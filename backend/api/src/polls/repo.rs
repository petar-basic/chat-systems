use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::models::{OptionTally, OptionVoters, Poll, PollDraft, PollPlace};

#[derive(Clone)]
pub struct PollRepo {
    pool: PgPool,
}

impl PollRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_in(
        &self,
        conn: &mut PgConnection,
        message_id: Uuid,
        created_by: Uuid,
        draft: &PollDraft,
    ) -> sqlx::Result<Poll> {
        let poll = sqlx::query_as!(
            Poll,
            r#"INSERT INTO polls (message_id, question, multiple_choice, anonymous, closes_at, created_by)
               VALUES ($1, $2, $3, $4, $5, $6)
               RETURNING id, message_id, question, multiple_choice, anonymous, closes_at, closed_at, created_by"#,
            message_id,
            draft.question,
            draft.multiple_choice,
            draft.anonymous,
            draft.closes_at,
            created_by
        )
        .fetch_one(&mut *conn)
        .await?;
        sqlx::query!(
            "INSERT INTO poll_options (poll_id, position, label)
             SELECT $1, (o.position - 1)::smallint, o.label
               FROM unnest($2::text[]) WITH ORDINALITY AS o(label, position)",
            poll.id,
            &draft.options
        )
        .execute(&mut *conn)
        .await?;
        Ok(poll)
    }

    pub async fn find(&self, id: Uuid) -> sqlx::Result<Option<PollPlace>> {
        let row = sqlx::query!(
            r#"SELECT p.id, p.message_id, p.question, p.multiple_choice, p.anonymous,
                      p.closes_at, p.closed_at, p.created_by,
                      m.channel_id, c.workspace_id, (m.deleted_at IS NOT NULL) AS "message_deleted!"
                 FROM polls p
                 JOIN messages m ON m.id = p.message_id
                 JOIN channels c ON c.id = m.channel_id
                WHERE p.id = $1"#,
            id
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|row| PollPlace {
            channel_id: row.channel_id,
            workspace_id: row.workspace_id,
            message_deleted: row.message_deleted,
            poll: Poll {
                id: row.id,
                message_id: row.message_id,
                question: row.question,
                multiple_choice: row.multiple_choice,
                anonymous: row.anonymous,
                closes_at: row.closes_at,
                closed_at: row.closed_at,
                created_by: row.created_by,
            },
        }))
    }

    pub async fn lock_in(&self, conn: &mut PgConnection, id: Uuid) -> sqlx::Result<Poll> {
        sqlx::query_as!(
            Poll,
            "SELECT id, message_id, question, multiple_choice, anonymous, closes_at, closed_at, created_by
               FROM polls WHERE id = $1 FOR UPDATE",
            id
        )
        .fetch_one(&mut *conn)
        .await
    }

    pub async fn option_ids_in(
        &self,
        conn: &mut PgConnection,
        poll_id: Uuid,
    ) -> sqlx::Result<Vec<Uuid>> {
        sqlx::query_scalar!(
            "SELECT id FROM poll_options WHERE poll_id = $1 ORDER BY position",
            poll_id
        )
        .fetch_all(&mut *conn)
        .await
    }

    pub async fn replace_vote_in(
        &self,
        conn: &mut PgConnection,
        poll: &Poll,
        user_id: Uuid,
        option_ids: &[Uuid],
    ) -> sqlx::Result<()> {
        sqlx::query!(
            "DELETE FROM poll_votes WHERE poll_id = $1 AND user_id = $2",
            poll.id,
            user_id
        )
        .execute(&mut *conn)
        .await?;
        sqlx::query!(
            "INSERT INTO poll_votes (poll_id, option_id, user_id, single_choice)
             SELECT $1, option_id, $2, $3 FROM unnest($4::uuid[]) AS option_id",
            poll.id,
            user_id,
            !poll.multiple_choice,
            option_ids
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    pub async fn tally_in(
        &self,
        conn: &mut PgConnection,
        poll_id: Uuid,
    ) -> sqlx::Result<(Vec<OptionTally>, i64)> {
        let options = sqlx::query_as!(
            OptionTally,
            r#"SELECT o.id, o.label, COUNT(v.user_id) AS "votes!"
                 FROM poll_options o
                 LEFT JOIN poll_votes v ON v.option_id = o.id
                WHERE o.poll_id = $1
                GROUP BY o.id, o.label, o.position
                ORDER BY o.position"#,
            poll_id
        )
        .fetch_all(&mut *conn)
        .await?;
        let voters = sqlx::query_scalar!(
            r#"SELECT COUNT(DISTINCT user_id) AS "voters!" FROM poll_votes WHERE poll_id = $1"#,
            poll_id
        )
        .fetch_one(&mut *conn)
        .await?;
        Ok((options, voters))
    }

    pub async fn close_in(&self, conn: &mut PgConnection, poll_id: Uuid) -> sqlx::Result<Poll> {
        sqlx::query_as!(
            Poll,
            "UPDATE polls SET closed_at = LEAST(NOW(), COALESCE(closes_at, NOW()))
              WHERE id = $1
              RETURNING id, message_id, question, multiple_choice, anonymous, closes_at, closed_at, created_by",
            poll_id
        )
        .fetch_one(&mut *conn)
        .await
    }

    pub async fn my_vote(&self, poll_id: Uuid, user_id: Uuid) -> sqlx::Result<Vec<Uuid>> {
        sqlx::query_scalar!(
            "SELECT v.option_id FROM poll_votes v
               JOIN poll_options o ON o.id = v.option_id
              WHERE v.poll_id = $1 AND v.user_id = $2
              ORDER BY o.position",
            poll_id,
            user_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn voters(&self, poll_id: Uuid) -> sqlx::Result<Vec<OptionVoters>> {
        sqlx::query_as!(
            OptionVoters,
            r#"SELECT o.id AS option_id,
                      COALESCE(array_agg(v.user_id ORDER BY v.created_at, v.user_id)
                               FILTER (WHERE v.user_id IS NOT NULL), '{}') AS "user_ids!"
                 FROM poll_options o
                 LEFT JOIN poll_votes v ON v.option_id = o.id
                WHERE o.poll_id = $1
                GROUP BY o.id, o.position
                ORDER BY o.position"#,
            poll_id
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn due(&self, limit: i64) -> sqlx::Result<Vec<Uuid>> {
        sqlx::query_scalar!(
            "SELECT id FROM polls
              WHERE closed_at IS NULL AND closes_at <= NOW()
              ORDER BY closes_at
              LIMIT $1",
            limit
        )
        .fetch_all(&self.pool)
        .await
    }
}
