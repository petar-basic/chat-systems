use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use tracing::{info, warn};
use uuid::Uuid;

use crate::state::AppState;

pub const MAX_ATTEMPTS: i32 = 8;
const POLL_INTERVAL: Duration = Duration::from_secs(2);
const BATCH: i64 = 20;
const LEASE: chrono::Duration = chrono::Duration::minutes(2);

#[derive(Debug, sqlx::FromRow)]
pub struct OutboundEmail {
    pub id: Uuid,
    pub to_address: String,
    pub subject: String,
    pub text_body: String,
    pub html_body: Option<String>,
    pub attempts: i32,
    pub next_attempt_at: Option<DateTime<Utc>>,
    pub sent_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub struct NewEmail<'a> {
    pub to: &'a str,
    pub subject: &'a str,
    pub text: &'a str,
    pub html: Option<&'a str>,
}

pub async fn enqueue(pool: &PgPool, email: NewEmail<'_>) -> sqlx::Result<Uuid> {
    sqlx::query_scalar(
        "INSERT INTO outbound_emails (to_address, subject, text_body, html_body)
         VALUES ($1, $2, $3, $4)
         RETURNING id",
    )
    .bind(email.to)
    .bind(email.subject)
    .bind(email.text)
    .bind(email.html)
    .fetch_one(pool)
    .await
}

pub async fn claim_due(pool: &PgPool, limit: i64) -> sqlx::Result<Vec<OutboundEmail>> {
    sqlx::query_as::<_, OutboundEmail>(
        "UPDATE outbound_emails
            SET attempts = attempts + 1, next_attempt_at = $2
          WHERE id IN (
                SELECT id FROM outbound_emails
                 WHERE sent_at IS NULL AND next_attempt_at <= NOW()
                 ORDER BY next_attempt_at
                 LIMIT $1
                 FOR UPDATE SKIP LOCKED)
      RETURNING *",
    )
    .bind(limit)
    .bind(Utc::now() + LEASE)
    .fetch_all(pool)
    .await
}

pub async fn mark_sent(pool: &PgPool, id: Uuid) -> sqlx::Result<()> {
    sqlx::query("UPDATE outbound_emails SET sent_at = NOW(), last_error = NULL WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn mark_failed(pool: &PgPool, id: Uuid, attempts: i32, error: &str) -> sqlx::Result<()> {
    let next_attempt_at = (attempts < MAX_ATTEMPTS).then(|| Utc::now() + backoff(attempts));
    sqlx::query("UPDATE outbound_emails SET next_attempt_at = $2, last_error = $3 WHERE id = $1")
        .bind(id)
        .bind(next_attempt_at)
        .bind(error)
        .execute(pool)
        .await?;
    Ok(())
}

fn backoff(attempts: i32) -> chrono::Duration {
    let minutes = 4_i64.pow(attempts.saturating_sub(1).clamp(0, 4) as u32);
    chrono::Duration::minutes(minutes.min(240))
}

pub async fn flush_due(state: &AppState) -> usize {
    let due = match claim_due(&state.pool, BATCH).await {
        Ok(due) => due,
        Err(e) => {
            warn!("could not claim outbound emails: {}", e);
            return 0;
        }
    };

    let mut sent = 0;
    for email in due {
        let delivered = state
            .auth_service
            .deliver(
                &email.to_address,
                &email.subject,
                &email.text_body,
                email.html_body.as_deref(),
            )
            .await;
        let outcome = match delivered {
            Ok(()) => {
                sent += 1;
                metrics::counter!("outbound_emails_sent_total").increment(1);
                mark_sent(&state.pool, email.id).await
            }
            Err(e) => {
                warn!(
                    email_id = %email.id,
                    attempts = email.attempts,
                    "email delivery failed: {}", e
                );
                mark_failed(&state.pool, email.id, email.attempts, &e.to_string()).await
            }
        };
        if let Err(e) = outcome {
            warn!(email_id = %email.id, "could not record email outcome: {}", e);
        }
    }
    sent
}

pub async fn start_outbox_worker(state: Arc<AppState>) {
    info!("Email outbox worker started");
    loop {
        tokio::time::sleep(POLL_INTERVAL).await;
        flush_due(&state).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn queue_one(pool: &PgPool) -> Uuid {
        enqueue(
            pool,
            NewEmail {
                to: "someone@example.com",
                subject: "hello",
                text: "body",
                html: None,
            },
        )
        .await
        .unwrap()
    }

    async fn make_due(pool: &PgPool, id: Uuid) {
        sqlx::query("UPDATE outbound_emails SET next_attempt_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await
            .unwrap();
    }

    #[test_macros::db_test(migrations = "../migrations")]
    async fn a_claimed_email_is_leased_until_its_outcome_is_recorded(pool: PgPool) {
        let id = queue_one(&pool).await;

        let first = claim_due(&pool, 10).await.unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].id, id);
        assert_eq!(first[0].attempts, 1);

        assert!(
            claim_due(&pool, 10).await.unwrap().is_empty(),
            "a leased email is not handed to a second worker"
        );

        mark_sent(&pool, id).await.unwrap();
        make_due(&pool, id).await;
        assert!(
            claim_due(&pool, 10).await.unwrap().is_empty(),
            "a sent email is never claimed again"
        );
    }

    #[test_macros::db_test(migrations = "../migrations")]
    async fn a_failure_backs_off_and_eventually_gives_up(pool: PgPool) {
        let id = queue_one(&pool).await;
        let claimed = claim_due(&pool, 10).await.unwrap();
        mark_failed(&pool, id, claimed[0].attempts, "smtp down")
            .await
            .unwrap();

        let (next, error): (Option<DateTime<Utc>>, Option<String>) =
            sqlx::query_as("SELECT next_attempt_at, last_error FROM outbound_emails WHERE id = $1")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(next.unwrap() > Utc::now() + chrono::Duration::seconds(30));
        assert_eq!(error.as_deref(), Some("smtp down"));

        mark_failed(&pool, id, MAX_ATTEMPTS, "still down")
            .await
            .unwrap();
        let dead: Option<DateTime<Utc>> =
            sqlx::query_scalar("SELECT next_attempt_at FROM outbound_emails WHERE id = $1")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(
            dead.is_none(),
            "past the attempt limit the email is parked, not retried"
        );
    }

    #[test_macros::db_test(migrations = "../migrations")]
    async fn an_invite_is_queued_rather_than_sent_inline(pool: PgPool) {
        let (_, state) = crate::http_tests::common::app_and_state(pool).await;

        state
            .auth_service
            .send_invite_email("new@example.com", "Ops", "https://chat.example/invite/x")
            .await
            .unwrap();

        let queued: Vec<(String, String, Option<String>)> = sqlx::query_as(
            "SELECT to_address, subject, html_body FROM outbound_emails WHERE sent_at IS NULL",
        )
        .fetch_all(&state.pool)
        .await
        .unwrap();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].0, "new@example.com");
        assert!(queued[0].1.contains("Ops"));
        assert!(queued[0].2.as_deref().unwrap_or("").contains("Join Ops"));
    }

    #[test]
    fn backoff_grows_and_caps() {
        assert_eq!(backoff(1), chrono::Duration::minutes(1));
        assert_eq!(backoff(2), chrono::Duration::minutes(4));
        assert_eq!(backoff(3), chrono::Duration::minutes(16));
        assert_eq!(backoff(4), chrono::Duration::minutes(64));
        assert_eq!(backoff(5), chrono::Duration::minutes(240));
        assert_eq!(backoff(9), chrono::Duration::minutes(240));
    }
}
