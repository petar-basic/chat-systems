use serde::Serialize;
use uuid::Uuid;
use web_push::{
    ContentEncoding, SubscriptionInfo, SubscriptionKeys, VapidSignatureBuilder,
    WebPushMessageBuilder,
};

use super::repo::PushRepo;

/// Long enough that a phone which was asleep still gets the mention, short
/// enough that it is not woken up for something an hour stale.
const TTL_SECS: u32 = 900;
/// A push payload passes through a third party. Encrypted, but the plaintext is
/// still handed to a browser process this instance does not control, so it
/// carries a hint rather than the message.
const PREVIEW_CHARS: usize = 80;

#[derive(Debug, Clone)]
pub struct VapidKeys {
    pub public_key: String,
    pub private_key: String,
    pub subject: String,
}

impl VapidKeys {
    pub fn configured(&self) -> bool {
        !self.public_key.is_empty() && !self.private_key.is_empty()
    }
}

#[derive(Debug, Serialize)]
pub struct PushPayload {
    pub title: String,
    pub body: String,
    pub workspace_id: Option<String>,
    pub channel_id: Option<String>,
    pub conversation_id: Option<String>,
    pub message_id: Option<String>,
    pub badge_count: i64,
}

impl PushPayload {
    pub fn preview(content: &str) -> String {
        let trimmed = content.trim();
        if trimmed.chars().count() <= PREVIEW_CHARS {
            return trimmed.to_string();
        }
        let cut: String = trimmed.chars().take(PREVIEW_CHARS).collect();
        format!("{cut}…")
    }
}

#[derive(Clone)]
pub struct PushSender {
    repo: PushRepo,
    keys: VapidKeys,
    http: reqwest::Client,
}

impl PushSender {
    pub fn new(repo: PushRepo, keys: VapidKeys) -> Self {
        Self {
            repo,
            keys,
            http: reqwest::Client::new(),
        }
    }

    pub fn is_configured(&self) -> bool {
        self.keys.configured()
    }

    /// One request per live subscription. Failures are per subscription: a
    /// browser whose push service is down must not stop the person's other
    /// devices from being notified.
    pub async fn send_to_user(&self, user_id: Uuid, payload: &PushPayload) {
        if !self.is_configured() {
            return;
        }

        let subscriptions = match self.repo.list_for_user(user_id).await {
            Ok(subscriptions) => subscriptions,
            Err(e) => {
                tracing::warn!("push: could not read subscriptions for {}: {}", user_id, e);
                return;
            }
        };

        let body = match serde_json::to_vec(payload) {
            Ok(body) => body,
            Err(e) => {
                tracing::warn!("push: could not serialize payload: {}", e);
                return;
            }
        };

        for subscription in subscriptions {
            let info = SubscriptionInfo {
                endpoint: subscription.endpoint.clone(),
                keys: SubscriptionKeys {
                    p256dh: subscription.p256dh.clone(),
                    auth: subscription.auth.clone(),
                },
            };

            match self.deliver(&info, &body).await {
                Ok(status) if status.as_u16() == 410 || status.as_u16() == 404 => {
                    tracing::info!("push: pruning a subscription the service reported gone");
                    let _ = self.repo.delete_by_endpoint(&subscription.endpoint).await;
                    metrics::counter!("push_subscriptions_pruned_total").increment(1);
                }
                Ok(status) if status.is_success() => {
                    self.repo.touch(subscription.id).await;
                    metrics::counter!("push_notifications_sent_total").increment(1);
                }
                Ok(status) => {
                    tracing::warn!("push: service answered {}", status);
                    metrics::counter!("push_notifications_failed_total").increment(1);
                }
                Err(e) => {
                    tracing::warn!("push: delivery failed: {}", e);
                    metrics::counter!("push_notifications_failed_total").increment(1);
                }
            }
        }
    }

    /// The crate builds and encrypts the message; the request goes out over the
    /// HTTP client this project already uses everywhere else, rather than
    /// pulling a second TLS stack in behind it.
    async fn deliver(
        &self,
        info: &SubscriptionInfo,
        body: &[u8],
    ) -> Result<reqwest::StatusCode, String> {
        let mut signature_builder =
            VapidSignatureBuilder::from_base64(&self.keys.private_key, info)
                .map_err(|e| format!("invalid VAPID key: {e}"))?;
        signature_builder.add_claim("sub", self.keys.subject.clone());
        let signature = signature_builder
            .build()
            .map_err(|e| format!("could not sign: {e}"))?;

        let mut builder = WebPushMessageBuilder::new(info);
        builder.set_payload(ContentEncoding::Aes128Gcm, body);
        builder.set_vapid_signature(signature);
        builder.set_ttl(TTL_SECS);

        let message = builder
            .build()
            .map_err(|e| format!("could not encrypt: {e}"))?;

        let mut request = self
            .http
            .post(message.endpoint.to_string())
            .header("TTL", message.ttl.to_string());

        if let Some(payload) = message.payload {
            for (name, value) in payload.crypto_headers {
                request = request.header(name, value);
            }
            request = request
                .header("Content-Encoding", payload.content_encoding.to_str())
                .body(payload.content);
        } else {
            request = request.header("Content-Length", "0");
        }

        request
            .send()
            .await
            .map(|response| response.status())
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_preview_is_a_hint_not_the_message() {
        let short = "deploy is red";
        assert_eq!(PushPayload::preview(short), short);

        let long = "x".repeat(200);
        let preview = PushPayload::preview(&long);
        assert_eq!(preview.chars().count(), PREVIEW_CHARS + 1);
        assert!(preview.ends_with('…'));
    }

    #[test]
    fn a_multibyte_message_is_not_cut_mid_character() {
        let long = "čćžšđ".repeat(40);
        let preview = PushPayload::preview(&long);
        assert_eq!(preview.chars().count(), PREVIEW_CHARS + 1);
        assert!(preview.starts_with('č'));
    }

    #[test]
    fn push_stays_off_until_both_halves_of_the_key_are_present() {
        let half = VapidKeys {
            public_key: "public".into(),
            private_key: String::new(),
            subject: "mailto:ops@example.com".into(),
        };
        assert!(!half.configured());

        let whole = VapidKeys {
            private_key: "private".into(),
            ..half
        };
        assert!(whole.configured());
    }
}
