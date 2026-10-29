use std::time::Duration;

use uuid::Uuid;

use crate::email::outbox::{self, NewEmail};
use crate::state::AppState;

/// Long enough to be a digest rather than a stream, and a second chance for the
/// person to read it — at which point nothing is sent at all.
pub const DIGEST_WINDOW: Duration = Duration::from_secs(300);
const SWEEP_INTERVAL: Duration = Duration::from_secs(60);

pub struct PendingMention<'a> {
    pub user_id: Uuid,
    pub workspace_id: Uuid,
    pub channel_id: Option<Uuid>,
    pub message_id: Option<Uuid>,
    pub sender_name: &'a str,
    pub channel_name: &'a str,
    pub direct: bool,
}

pub async fn enqueue(state: &AppState, pending: PendingMention<'_>) {
    let wants_email = state
        .notification_repo
        .mention_emails(pending.workspace_id, pending.user_id)
        .await
        .unwrap_or(false);

    if !wants_email {
        return;
    }

    let result = sqlx::query!(
        r"
        INSERT INTO pending_mention_emails
            (user_id, workspace_id, channel_id, message_id, sender_name, channel_name, direct)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (user_id, message_id) DO NOTHING
        ",
        pending.user_id,
        pending.workspace_id,
        pending.channel_id,
        pending.message_id,
        pending.sender_name,
        pending.channel_name,
        pending.direct
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
    direct_messages: i64,
    channels: Vec<String>,
    senders: Vec<String>,
    channel_id: Option<Uuid>,
    message_id: Option<Uuid>,
}

/// One pass. Separated from the loop so a test can run it without waiting a
/// minute, the way the retention job is arranged.
pub async fn flush_due(state: &AppState) -> usize {
    let now = chrono::Utc::now();
    let cutoff = now - chrono::Duration::from_std(DIGEST_WINDOW).unwrap_or_default();

    let seen = sqlx::query!(
        r#"
        DELETE FROM pending_mention_emails p
         WHERE p.created_at <= $1
           AND NOT EXISTS (SELECT 1 FROM notifications n
                            WHERE n.user_id = p.user_id
                              AND n.data->>'message_id' = p.message_id::text
                              AND NOT n.is_read)
        "#,
        cutoff
    )
    .execute(&state.pool)
    .await;
    if let Err(e) = seen {
        tracing::warn!("could not drop mention emails already read: {}", e);
    }

    let due = sqlx::query_as!(
        DueDigest,
        r#"
        SELECT p.user_id, p.workspace_id, u.email,
               COUNT(*) FILTER (WHERE NOT p.direct) AS "mentions!",
               COUNT(*) FILTER (WHERE p.direct) AS "direct_messages!",
               COALESCE(ARRAY_AGG(DISTINCT COALESCE(p.channel_name, '')) FILTER (WHERE NOT p.direct), '{}') AS "channels!",
               ARRAY_AGG(DISTINCT COALESCE(p.sender_name, '')) AS "senders!",
               CASE WHEN COUNT(*) = 1 THEN (ARRAY_AGG(p.channel_id))[1] END AS "channel_id?",
               CASE WHEN COUNT(*) = 1 THEN (ARRAY_AGG(p.message_id))[1] END AS "message_id?"
          FROM pending_mention_emails p
          JOIN users u ON u.id = p.user_id
         WHERE p.created_at <= $1
           AND NOT u.is_bot
           AND public.notifications_paused_until(p.workspace_id, p.user_id, $2) IS NULL
         GROUP BY p.user_id, p.workspace_id, u.email
        "#,
        cutoff,
        now
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
        match send_digest(state, &digest, cutoff).await {
            Ok(true) => {
                sent += 1;
                metrics::counter!("mention_emails_queued_total").increment(1);
            }
            Ok(false) => {}
            Err(e) => tracing::warn!("could not queue a mention email: {}", e),
        }
    }

    sent
}

async fn send_digest(
    state: &AppState,
    digest: &DueDigest,
    cutoff: chrono::DateTime<chrono::Utc>,
) -> sqlx::Result<bool> {
    let mut tx = state.pool.begin().await?;
    let claimed = sqlx::query!(
        "DELETE FROM pending_mention_emails \
          WHERE user_id = $1 AND workspace_id = $2 AND created_at <= $3",
        digest.user_id,
        digest.workspace_id,
        cutoff
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if claimed == 0 {
        return Ok(false);
    }

    let subject = subject_for(digest);
    let body = body_for(digest, &state.config.public_url);
    outbox::enqueue(
        &mut *tx,
        NewEmail {
            to: &digest.email,
            subject: &subject,
            text: &body,
            html: None,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(true)
}

fn counted(n: i64, one: &str, many: &str) -> String {
    match n {
        1 => format!("1 {one}"),
        n => format!("{n} {many}"),
    }
}

fn what_happened(digest: &DueDigest) -> String {
    match (digest.mentions, digest.direct_messages) {
        (0, dms) => format!(
            "You have {}",
            counted(dms, "direct message", "direct messages")
        ),
        (mentions, 0) => format!("You have {}", counted(mentions, "mention", "mentions")),
        (mentions, dms) => format!(
            "You have {} and {}",
            counted(mentions, "mention", "mentions"),
            counted(dms, "direct message", "direct messages")
        ),
    }
}

fn subject_for(digest: &DueDigest) -> String {
    match (digest.mentions, digest.direct_messages) {
        (1, 0) => "You were mentioned".to_string(),
        (n, 0) => format!("You were mentioned {n} times"),
        (0, 1) => "You have a direct message".to_string(),
        _ => what_happened(digest),
    }
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

    let mut lines = vec![format!("{} while you were away.", what_happened(digest))];
    if !people.is_empty() {
        lines.push(format!("From: {}", people.join(", ")));
    }
    if !places.is_empty() {
        lines.push(format!("In: {}", places.join(", ")));
    }
    lines.push(String::new());
    let workspace = format!("{public_url}/app/{}", digest.workspace_id);
    lines.push(match (digest.channel_id, digest.message_id) {
        (Some(channel), Some(message)) => format!("Open it: {workspace}/{channel}/{message}"),
        _ => format!("Open the workspace: {workspace}"),
    });
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
