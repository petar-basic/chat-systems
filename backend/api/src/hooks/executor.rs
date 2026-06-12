use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use hmac::{Hmac, Mac};
use redis::AsyncCommands;
use sha2::Sha256;
use tracing::{info, warn};

use super::models::Hook;
use super::repo::HookRepo;
use super::ssrf;

type HmacSha256 = Hmac<Sha256>;

const HOOK_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const HOOK_MAX_ATTEMPTS: u32 = 3;
const HOOK_BACKOFF_BASE: Duration = Duration::from_millis(500);
const HOOK_BACKOFF_CAP: Duration = Duration::from_secs(5);
const HOOK_BODY_MAX_BYTES: usize = 4096;
const SIGNATURE_HEADER: &str = "X-ChatSystems-Signature";

pub async fn start_hook_consumer(redis_url: &str, hook_repo: Arc<HookRepo>) {
    let client = match redis::Client::open(redis_url) {
        Ok(c) => c,
        Err(e) => {
            warn!("Hook consumer: failed to connect Redis: {}", e);
            return;
        }
    };

    let mut pubsub = match client.get_async_pubsub().await {
        Ok(ps) => ps,
        Err(e) => {
            warn!("Hook consumer: failed to get pubsub: {}", e);
            return;
        }
    };

    if let Err(e) = pubsub.subscribe("events:message").await {
        warn!("Hook consumer: failed to subscribe: {}", e);
        return;
    }

    let http = match reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(HOOK_REQUEST_TIMEOUT)
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            warn!("Hook consumer: failed to build HTTP client: {}", e);
            return;
        }
    };

    info!("Hook consumer started");
    let mut stream = pubsub.into_on_message();

    while let Some(msg) = stream.next().await {
        let payload: String = match msg.get_payload() {
            Ok(p) => p,
            Err(_) => continue,
        };

        let event: serde_json::Value = match serde_json::from_str(&payload) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let event_type = event
            .get("event_type")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if event_type != "message.created" {
            continue;
        }

        let event_payload = match event.get("payload") {
            Some(p) => p.clone(),
            None => continue,
        };

        let workspace_id = event_payload
            .get("workspace_id")
            .and_then(|v| v.as_str())
            .and_then(|v| v.parse::<uuid::Uuid>().ok());

        let Some(ws_id) = workspace_id else { continue };

        let hooks = match hook_repo.list_active_outgoing_hooks(ws_id).await {
            Ok(h) => h,
            Err(e) => {
                warn!(workspace_id = %ws_id, "Hook consumer: failed to list hooks: {}", e);
                continue;
            }
        };

        for hook in hooks {
            dispatch_hook(&http, &hook_repo, &hook, &event_type, &event_payload).await;
        }
    }

    warn!("Hook consumer: event stream ended, exiting for restart");
}

async fn dispatch_hook(
    http: &reqwest::Client,
    hook_repo: &HookRepo,
    hook: &Hook,
    event_type: &str,
    event_payload: &serde_json::Value,
) {
    let Some(url_str) = hook.config.get("url").and_then(|v| v.as_str()) else {
        warn!(hook_id = %hook.id, "Hook skipped: config.url missing or not a string");
        return;
    };

    let url = match ssrf::validate_outbound_url(url_str).await {
        Ok(u) => u,
        Err(e) => {
            warn!(hook_id = %hook.id, "Hook skipped: url failed SSRF validation: {}", e);
            return;
        }
    };

    let secret = hook
        .config
        .get("secret")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let body = match serde_json::to_vec(event_payload) {
        Ok(b) => b,
        Err(e) => {
            warn!(hook_id = %hook.id, "Hook skipped: failed to serialize payload: {}", e);
            return;
        }
    };
    let signature = sign_body(secret, &body);

    let mut last_status: Option<i32> = None;
    let mut last_body: Option<String> = None;

    for attempt in 1..=HOOK_MAX_ATTEMPTS {
        let resp = http
            .post(url.clone())
            .header("Content-Type", "application/json")
            .header(SIGNATURE_HEADER, &signature)
            .timeout(HOOK_REQUEST_TIMEOUT)
            .body(body.clone())
            .send()
            .await;

        match resp {
            Ok(r) => {
                let status = r.status();
                let status_code = status.as_u16() as i32;
                let resp_body = r.text().await.unwrap_or_else(|e| {
                    warn!("failed to read webhook response body: {e}");
                    String::new()
                });
                last_status = Some(status_code);
                last_body = Some(truncate_body(&resp_body));

                if status.is_success() {
                    break;
                }

                if status.is_client_error() && status.as_u16() != 429 {
                    break;
                }
            }
            Err(e) => {
                last_status = None;
                last_body = Some(truncate_body(&format!("request error: {e}")));
            }
        }

        if attempt < HOOK_MAX_ATTEMPTS {
            tokio::time::sleep(backoff_for(attempt)).await;
        }
    }

    if let Err(e) = hook_repo
        .log_execution(
            hook.id,
            event_type,
            event_payload,
            last_status,
            last_body.as_deref(),
        )
        .await
    {
        warn!(hook_id = %hook.id, "Hook consumer: failed to log execution: {}", e);
    }
}

fn sign_body(secret: &str, body: &[u8]) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(body);
    let digest = mac.finalize().into_bytes();
    let mut hex = String::with_capacity(7 + digest.len() * 2);
    hex.push_str("sha256=");
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

fn backoff_for(attempt: u32) -> Duration {
    let factor = 1u32 << (attempt - 1);
    HOOK_BACKOFF_BASE
        .saturating_mul(factor)
        .min(HOOK_BACKOFF_CAP)
}

fn truncate_body(body: &str) -> String {
    if body.len() <= HOOK_BODY_MAX_BYTES {
        return body.to_string();
    }
    let mut end = HOOK_BODY_MAX_BYTES;
    while end > 0 && !body.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…[truncated]", &body[..end])
}

pub async fn start_reminder_checker(redis_url: &str, hook_repo: Arc<HookRepo>) {
    let client = match redis::Client::open(redis_url) {
        Ok(c) => c,
        Err(e) => {
            warn!("Reminder checker: failed to connect Redis: {}", e);
            return;
        }
    };

    let mut conn = match redis::aio::ConnectionManager::new(client).await {
        Ok(c) => c,
        Err(e) => {
            warn!("Reminder checker: failed to get connection: {}", e);
            return;
        }
    };

    info!("Reminder checker started");

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;

        let reminders = match hook_repo.get_due_reminders().await {
            Ok(r) => r,
            Err(e) => {
                warn!("Failed to get due reminders: {}", e);
                continue;
            }
        };

        for reminder in reminders {
            let notif_event = serde_json::json!({
                "event_type": "notification.push",
                "payload": {
                    "user_id": reminder.target_user_id.to_string(),
                    "channel_id": reminder.channel_id,
                    "title": "Reminder",
                    "body": reminder.content,
                    "priority": "mention",
                }
            });

            let json = notif_event.to_string();
            if let Err(e) = conn.publish::<_, _, ()>("events:notification", &json).await {
                warn!("failed to publish reminder notification: {e}");
            }

            if let Err(e) = hook_repo.mark_reminder_delivered(reminder.id).await {
                warn!("Failed to mark reminder delivered: {}", e);
            }
        }
    }
}
