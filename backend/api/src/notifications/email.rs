use std::time::Duration;

use uuid::Uuid;

use crate::email::outbox::{self, NewEmail};
use crate::state::AppState;

/// Long enough to be a digest rather than a stream, and a second chance for the
/// person to come online — at which point nothing is sent at all.
pub const DIGEST_WINDOW: Duration = Duration::from_secs(300);
const SWEEP_INTERVAL: Duration = Duration::from_secs(60);

pub struct PendingMention<'a> {
    pub user_id: Uuid,
    pub workspace_id: Uuid,
    pub channel_id: Option<Uuid>,
    pub message_id: Option<Uuid>,
    pub sender_name: &'a str,
    pub channel_name: &'a str,
}

/// Called only when nothing else could have reached them: no live socket, no
/// push subscription, not muted, not in do-not-disturb. Anything looser and this
/// becomes the duplicate email that makes people write a filter rule for your
/// domain.
pub async fn enqueue(state: &AppState, pending: PendingMention<'_>) {
    let wants_email: Option<bool> = sqlx::query_scalar!(
        "SELECT mention_emails FROM users WHERE id = $1",
        pending.user_id
    )
    .fetch_optional(&state.pool)
    .await
    .unwrap_or(None);

    if wants_email != Some(true) {
        return;
    }

    let result = sqlx::query!(
        r"
        INSERT INTO pending_mention_emails
            (user_id, workspace_id, channel_id, message_id, sender_name, channel_name)
        VALUES ($1, $2, $3, $4, $5, $6)
        ",
        pending.user_id,
        pending.workspace_id,
        pending.channel_id,
        pending.message_id,
        pending.sender_name,
        pending.channel_name
    )
    .execute(&state.pool)
    .await;

    if let Err(e) = result {
        tracing::warn!("could not queue a mention email: {}", e);
    }
}

#[derive(Debug)]
struct DueDigest {
    user_id: Uuid,
    workspace_id: Uuid,
    email: String,
    mentions: i64,
    channels: Vec<String>,
    senders: Vec<String>,
}

/// One pass. Separated from the loop so a test can run it without waiting a
/// minute, the way the retention job is arranged.
pub async fn flush_due(state: &AppState) -> usize {
    let cutoff = chrono::Utc::now() - chrono::Duration::from_std(DIGEST_WINDOW).unwrap_or_default();

    let due = sqlx::query_as!(
        DueDigest,
        r#"
        SELECT p.user_id, p.workspace_id, u.email,
               COUNT(*) AS "mentions!",
               ARRAY_AGG(DISTINCT COALESCE(p.channel_name, '')) AS "channels!",
               ARRAY_AGG(DISTINCT COALESCE(p.sender_name, '')) AS "senders!"
          FROM pending_mention_emails p
          JOIN users u ON u.id = p.user_id
         WHERE p.created_at <= $1
         GROUP BY p.user_id, p.workspace_id, u.email
        "#,
        cutoff
    )
    .fetch_all(&state.pool)
    .await;

    let due = match due {
        Ok(due) => due,
        Err(e) => {
            tracing::warn!("could not read pending mention emails: {}", e);
            return 0;
        }
    };

    let mut sent = 0;
    for digest in due {
        // They came back inside the window. The in-app badge is already waiting
        // for them, so the email would be telling them something they can see.
        let mut conn = state.redis.clone();
        let online =
            crate::presence::is_online(&mut conn, digest.workspace_id, digest.user_id).await;

        if !online {
            let subject = match digest.mentions {
                1 => "You were mentioned".to_string(),
                n => format!("You were mentioned {n} times"),
            };
            let body = body_for(&digest, &state.config.public_url);

            let queued = outbox::enqueue(
                &state.pool,
                NewEmail {
                    to: &digest.email,
                    subject: &subject,
                    text: &body,
                    html: None,
                },
            )
            .await;
            if let Err(e) = queued {
                tracing::warn!("could not queue a mention email: {}", e);
                continue;
            }
            sent += 1;
            metrics::counter!("mention_emails_queued_total").increment(1);
        }

        let cleared = sqlx::query!(
            "DELETE FROM pending_mention_emails \
              WHERE user_id = $1 AND workspace_id = $2 AND created_at <= $3",
            digest.user_id,
            digest.workspace_id,
            cutoff
        )
        .execute(&state.pool)
        .await;

        if let Err(e) = cleared {
            tracing::warn!("could not clear sent mention emails: {}", e);
        }
    }

    sent
}

/// Who, where, and a link. Not what was said: mail sits in an inbox on somebody
/// else's server indefinitely, and is the least private transport this product
/// touches.
fn body_for(digest: &DueDigest, public_url: &str) -> String {
    let people: Vec<&str> = digest
        .senders
        .iter()
        .map(String::as_str)
        .filter(|s| !s.is_empty())
        .collect();
    let places: Vec<String> = digest
        .channels
        .iter()
        .filter(|c| !c.is_empty())
        .map(|c| format!("#{c}"))
        .collect();

    let mut lines = Vec::new();
    lines.push(match digest.mentions {
        1 => "Somebody mentioned you while you were away.".to_string(),
        n => format!("You were mentioned {n} times while you were away."),
    });
    if !people.is_empty() {
        lines.push(format!("From: {}", people.join(", ")));
    }
    if !places.is_empty() {
        lines.push(format!("In: {}", places.join(", ")));
    }
    lines.push(String::new());
    lines.push(format!("Open the app: {public_url}/app"));
    lines.join("\n")
}

pub async fn start_digest_job(state: std::sync::Arc<AppState>) {
    if !state.auth_service.can_send_email() {
        tracing::info!("mention email digest disabled: no SMTP configured");
        return;
    }

    tracing::info!("mention email digest started");
    loop {
        tokio::time::sleep(SWEEP_INTERVAL).await;
        let sent = flush_due(&state).await;
        if sent > 0 {
            tracing::info!("sent {} mention digest emails", sent);
        }
    }
}
