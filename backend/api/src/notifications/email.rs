use std::time::Duration;

use uuid::Uuid;

use super::text::{self, Words};
use crate::email::message::{Button, Email};
use crate::email::outbox::{self, NewEmail};
use crate::email::unsubscribe;
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
    pub sender_name: Option<&'a str>,
    pub channel_name: Option<&'a str>,
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
    workspace_name: String,
    email: String,
    language: Option<String>,
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
        SELECT p.user_id, p.workspace_id, w.name AS workspace_name, u.email,
               u.preferences->>'language' AS "language?",
               COUNT(*) FILTER (WHERE NOT p.direct) AS "mentions!",
               COUNT(*) FILTER (WHERE p.direct) AS "direct_messages!",
               COALESCE(ARRAY_AGG(DISTINCT COALESCE(p.channel_name, '')) FILTER (WHERE NOT p.direct), '{}') AS "channels!",
               ARRAY_AGG(DISTINCT COALESCE(p.sender_name, '')) AS "senders!",
               CASE WHEN COUNT(*) = 1 THEN (ARRAY_AGG(p.channel_id))[1] END AS "channel_id?",
               CASE WHEN COUNT(*) = 1 THEN (ARRAY_AGG(p.message_id))[1] END AS "message_id?"
          FROM pending_mention_emails p
          JOIN users u ON u.id = p.user_id
          JOIN workspaces w ON w.id = p.workspace_id
         WHERE p.created_at <= $1
           AND NOT u.is_bot
           AND public.notifications_paused_until(p.workspace_id, p.user_id, $2) IS NULL
         GROUP BY p.user_id, p.workspace_id, w.name, u.email, u.preferences->>'language'
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

    let words = text::words(text::language(digest.language.as_deref()));
    let unsubscribe = unsubscribe::url_for(
        &state.config.public_url,
        &state.config.secrets_keys,
        digest.user_id,
        digest.workspace_id,
    );
    let rendered = match digest_email(digest, &state.config.public_url, &words, unsubscribe).render(
        &digest.email,
        &state.config,
        &words,
    ) {
        Ok(rendered) => rendered,
        Err(e) => {
            tracing::warn!("could not write a mention email: {}", e);
            return Ok(false);
        }
    };
    outbox::enqueue(&mut *tx, NewEmail::rendered(&digest.email, &rendered)).await?;
    tx.commit().await?;
    Ok(true)
}

fn what_happened(digest: &DueDigest, words: &Words) -> String {
    let mentions = || words.count("email.mentions", digest.mentions, &[]);
    let dms = || words.count("email.directMessages", digest.direct_messages, &[]);
    match (digest.mentions, digest.direct_messages) {
        (0, _) => words.get("email.youHave", &[("things", &dms())]),
        (_, 0) => words.get("email.youHave", &[("things", &mentions())]),
        _ => words.get(
            "email.youHaveBoth",
            &[("mentions", &mentions()), ("directMessages", &dms())],
        ),
    }
}

fn subject_for(digest: &DueDigest, words: &Words) -> String {
    let summary = match (digest.mentions, digest.direct_messages) {
        (1, 0) => words.get("email.mentioned", &[]),
        (n, 0) => words.count("email.mentionedTimes", n, &[]),
        (0, 1) => words.get("email.aDirectMessage", &[]),
        _ => what_happened(digest, words),
    };
    words.get(
        "email.inWorkspace",
        &[("summary", &summary), ("workspace", &digest.workspace_name)],
    )
}

fn digest_email(digest: &DueDigest, public_url: &str, words: &Words, unsubscribe: String) -> Email {
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

    let summary = what_happened(digest, words);
    let mut paragraphs = Vec::new();
    if !people.is_empty() {
        paragraphs.push(words.get("email.from", &[("people", &people.join(", "))]));
    }
    if !places.is_empty() {
        paragraphs.push(words.get("email.in", &[("places", &places.join(", "))]));
    }
    let workspace = &digest.workspace_name;
    let home = format!(
        "{}/app/{}",
        public_url.trim_end_matches('/'),
        digest.workspace_id
    );
    let button = match (digest.channel_id, digest.message_id) {
        (Some(channel), Some(message)) => Button {
            label: words.get("email.digest.open", &[]),
            url: format!("{home}/{channel}/{message}"),
        },
        _ => Button {
            label: words.get("email.digest.openWorkspace", &[("workspace", workspace)]),
            url: home,
        },
    };
    Email {
        subject: subject_for(digest, words),
        heading: words.get("email.whileAway", &[("summary", &summary)]),
        paragraphs,
        button: Some(button),
        reason: Some(words.get("email.digest.reason", &[("workspace", workspace)])),
        unsubscribe: Some(unsubscribe),
        ..Email::default()
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::models::Language;

    const JA: &str = r#"{
        "email": {
            "mentioned": "メンションされました",
            "mentions_other": "{{count}} 件のメンション",
            "youHave": "{{things}}があります",
            "inWorkspace": "{{workspace}}: {{summary}}"
        }
    }"#;

    fn digest(mentions: i64, direct_messages: i64) -> DueDigest {
        DueDigest {
            user_id: Uuid::nil(),
            workspace_id: Uuid::nil(),
            workspace_name: "Ops".into(),
            email: "ana@test.local".into(),
            language: Some("ja".into()),
            mentions,
            direct_messages,
            channels: vec!["general".into()],
            senders: vec!["Bo".into()],
            channel_id: None,
            message_id: None,
        }
    }

    #[test]
    fn the_digest_is_written_in_the_readers_language() {
        let catalog = text::Catalog::parse(include_str!("../../locales/en.json"), JA);
        let ja = catalog.words(Language::Ja);
        assert_eq!(subject_for(&digest(1, 0), &ja), "Ops: メンションされました");
        assert_eq!(
            subject_for(&digest(0, 2), &ja),
            "Ops: 2 direct messagesがあります"
        );
        let email = digest_email(&digest(3, 0), "https://chat.test", &ja, "u".into());
        assert_eq!(
            email.heading,
            "3 件のメンションがあります while you were away."
        );
        assert_eq!(email.paragraphs, vec!["From: Bo", "In: #general"]);
    }

    #[test]
    fn without_the_strings_the_digest_is_english() {
        let catalog = text::Catalog::parse(include_str!("../../locales/en.json"), "{}");
        let ja = catalog.words(Language::Ja);
        assert_eq!(
            subject_for(&digest(2, 0), &ja),
            "You were mentioned 2 times in Ops"
        );
        assert_eq!(
            subject_for(&digest(2, 1), &ja),
            "You have 2 mentions and 1 direct message in Ops"
        );
        let email = digest_email(&digest(1, 0), "https://chat.test/", &ja, "u".into());
        let button = email.button.expect("a button");
        assert_eq!(button.label, "Open Ops");
        assert_eq!(
            button.url,
            "https://chat.test/app/00000000-0000-0000-0000-000000000000"
        );
        assert_eq!(email.unsubscribe.as_deref(), Some("u"));
    }
}
