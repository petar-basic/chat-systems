use std::sync::Arc;
use std::time::Duration;

use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use tracing::{info, warn};

use super::models::Hook;
use super::repo::HookRepo;
use super::ssrf;
use crate::authz;
use crate::messaging::stream_group::StreamGroup;
use crate::state::AppState;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

const HOOK_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const HOOK_MAX_ATTEMPTS: u32 = 3;
const HOOK_BACKOFF_BASE: Duration = Duration::from_millis(500);
const HOOK_BACKOFF_CAP: Duration = Duration::from_secs(5);
const HOOK_BODY_MAX_BYTES: usize = 4096;
pub(crate) const SIGNATURE_HEADER: &str = "X-ChatSystems-Signature";

pub async fn start_hook_consumer(redis_url: &str, state: Arc<AppState>) {
    let Some(mut group) = StreamGroup::connect(redis_url, "hooks").await else {
        return;
    };

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

    loop {
        for delivery in group.next_batch().await {
            dispatch_delivery(&http, &state, &delivery).await;
            group.ack(&delivery.key, &delivery.id).await;
        }
    }
}

async fn dispatch_delivery(
    http: &reqwest::Client,
    state: &AppState,
    delivery: &crate::messaging::stream_group::Delivery,
) {
    let hook_repo = &state.hook_repo;
    let event_type = delivery.event.event_type.as_str();
    if event_type != "message.created" {
        return;
    }
    let event_payload = &delivery.event.payload;

    let Some(ws_id) = event_payload
        .get("workspace_id")
        .and_then(|v| v.as_str())
        .and_then(|v| v.parse::<uuid::Uuid>().ok())
    else {
        return;
    };

    let Some(channel_id) = event_payload
        .get("channel_id")
        .and_then(|v| v.as_str())
        .and_then(|v| v.parse::<uuid::Uuid>().ok())
    else {
        return;
    };

    let hooks = hooks_to_fire(state, ws_id, channel_id).await;
    if hooks.is_empty() {
        return;
    }

    let delivered = outbound_payload(event_payload);

    for hook in hooks {
        // Consumer groups redeliver anything that was not acknowledged, so the
        // same event can arrive twice after a worker dies mid-dispatch. Claiming
        // the pair first means the second arrival calls nobody: a webhook is an
        // outward side effect and firing it twice is not a retry, it is a bug in
        // somebody else's system.
        match hook_repo.claim_execution(hook.id, delivery.event.id).await {
            Ok(true) => {}
            Ok(false) => {
                info!(hook_id = %hook.id, "Hook consumer: already dispatched, skipping redelivery");
                continue;
            }
            Err(e) => {
                warn!(hook_id = %hook.id, "Hook consumer: failed to claim execution: {}", e);
                continue;
            }
        }
        dispatch_hook(
            http,
            hook_repo,
            &hook,
            event_type,
            &delivered,
            delivery.event.id,
        )
        .await;
    }
}

pub(crate) async fn hooks_to_fire(state: &AppState, ws_id: Uuid, channel_id: Uuid) -> Vec<Hook> {
    let hooks = match state
        .hook_repo
        .list_active_outgoing_hooks_for_channel(ws_id, channel_id)
        .await
    {
        Ok(h) => h,
        Err(e) => {
            warn!(workspace_id = %ws_id, "Hook consumer: failed to list hooks: {}", e);
            return Vec::new();
        }
    };
    let mut firing = Vec::with_capacity(hooks.len());
    for hook in hooks {
        match authz::still_acting(state, ws_id, hook.created_by, Some(channel_id)).await {
            Ok(None) => firing.push(hook),
            Ok(Some(why)) => {
                info!(hook_id = %hook.id, "Hook disabled: {}", why.reason());
                if let Err(e) = state.hook_repo.disable(hook.id, why.reason()).await {
                    warn!(hook_id = %hook.id, "Hook consumer: failed to disable hook: {}", e);
                }
            }
            Err(e) => {
                warn!(hook_id = %hook.id, "Hook consumer: failed to check its creator: {}", e);
            }
        }
    }
    firing
}

/// The event that fans out internally is not the event a third party gets. It
/// carries `mentioned_user_ids` and whatever the message model grows next, and a
/// webhook is an endpoint outside the trust boundary — so the outbound shape is
/// enumerated, not filtered.
fn outbound_payload(event_payload: &serde_json::Value) -> serde_json::Value {
    let mut out = serde_json::Map::new();
    for field in [
        "id",
        "channel_id",
        "workspace_id",
        "user_id",
        "content",
        "created_at",
    ] {
        if let Some(value) = event_payload.get(field) {
            out.insert(field.to_string(), value.clone());
        }
    }
    serde_json::Value::Object(out)
}

async fn dispatch_hook(
    http: &reqwest::Client,
    hook_repo: &HookRepo,
    hook: &Hook,
    event_type: &str,
    event_payload: &serde_json::Value,
    event_id: uuid::Uuid,
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
        .record_execution_result(
            hook.id,
            event_id,
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

pub(crate) fn sign_body(secret: &str, body: &[u8]) -> String {
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
