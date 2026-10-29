use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use tracing::{info, warn};
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::repo::Claimed;
use crate::hooks::{executor, ssrf};
use crate::sealed;
use crate::state::AppState;

pub const RETRY_NUM_HEADER: &str = "X-Chat-Retry-Num";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const CHALLENGE_TIMEOUT: Duration = Duration::from_secs(5);
const LEASE_SECS: f64 = 30.0;
const BATCH: i64 = 100;
const IN_FLIGHT: usize = 16;
const IDLE: Duration = Duration::from_millis(500);
const SWEEP_EVERY: Duration = Duration::from_secs(10);
const KEEP_SETTLED_SECS: f64 = 24.0 * 60.0 * 60.0;
const RETRY_DELAYS_SECS: [f64; 6] = [5.0, 30.0, 120.0, 300.0, 600.0, 900.0];
const ERROR_MAX_CHARS: usize = 300;

pub fn retry_delay(attempts: i32) -> f64 {
    let index = usize::try_from(attempts.max(1) - 1).unwrap_or(0);
    RETRY_DELAYS_SECS[index.min(RETRY_DELAYS_SECS.len() - 1)]
}

pub fn client() -> AppResult<reqwest::Client> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| AppError::Internal(e.to_string()))
}

pub fn seal_secret(state: &AppState, secret: &str) -> AppResult<sealed::Sealed> {
    sealed::seal(
        &state.config.secrets_keys,
        sealed::BOT_SIGNING_SECRETS,
        secret,
    )
}

fn open_secret(
    state: &AppState,
    ciphertext: &[u8],
    nonce: &[u8],
    key_id: Option<&str>,
) -> AppResult<String> {
    sealed::open(
        &state.config.secrets_keys,
        sealed::BOT_SIGNING_SECRETS,
        ciphertext,
        nonce,
        key_id,
    )
}

pub async fn signing_secret(state: &AppState, app_id: Uuid) -> AppResult<String> {
    if let Some(stored) = state.bot_repo.signing_secret(app_id).await? {
        return open_secret(
            state,
            &stored.ciphertext,
            &stored.nonce,
            stored.key_id.as_deref(),
        );
    }
    let secret = super::generate_secret();
    state
        .bot_repo
        .set_signing_secret(app_id, &seal_secret(state, &secret)?)
        .await?;
    Ok(secret)
}

fn short(error: &str) -> String {
    error.chars().take(ERROR_MAX_CHARS).collect()
}

pub async fn verify_request_url(state: &AppState, app_id: Uuid, url: &str) -> AppResult<()> {
    let target = ssrf::validate_outbound_url_with(url, state.config.webhook_allow_private_targets)
        .await
        .map_err(|e| AppError::BadRequest(format!("The request URL is not reachable: {e}")))?;
    let secret = signing_secret(state, app_id).await?;
    let challenge = super::generate_secret();
    let body = serde_json::to_vec(&serde_json::json!({
        "type": "url_verification",
        "challenge": challenge,
        "app_id": app_id,
    }))
    .map_err(|e| AppError::Internal(e.to_string()))?;

    let response = client()?
        .post(target)
        .header("Content-Type", "application/json")
        .header(
            executor::SIGNATURE_HEADER,
            executor::sign_body(&secret, &body),
        )
        .timeout(CHALLENGE_TIMEOUT)
        .body(body)
        .send()
        .await
        .map_err(|e| {
            AppError::BadRequest(format!(
                "The request URL did not answer: {}",
                short(&e.to_string())
            ))
        })?;
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    let echoed = text.trim() == challenge
        || serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v.get("challenge")?.as_str().map(str::to_string))
            .is_some_and(|c| c == challenge);
    if status.is_success() && echoed {
        Ok(())
    } else {
        Err(AppError::BadRequest(format!(
            "The request URL answered {status} without the challenge"
        )))
    }
}

async fn send_one(state: &AppState, http: &reqwest::Client, delivery: Claimed) {
    let outcome = attempt(state, http, &delivery).await;
    let saved = match outcome {
        Ok(()) => state.app_event_repo.delivered(delivery.id).await,
        Err(error) => {
            state
                .app_event_repo
                .retry_later(delivery.id, retry_delay(delivery.attempts), &short(&error))
                .await
        }
    };
    if let Err(e) = saved {
        warn!(delivery = %delivery.id, "App events: failed to record a delivery: {e}");
    }
}

async fn attempt(
    state: &AppState,
    http: &reqwest::Client,
    delivery: &Claimed,
) -> Result<(), String> {
    let url = delivery
        .request_url
        .as_deref()
        .ok_or_else(|| "The app has no request URL".to_string())?;
    let target = ssrf::validate_outbound_url_with(url, state.config.webhook_allow_private_targets)
        .await
        .map_err(|e| format!("The request URL is not reachable: {e}"))?;
    let (Some(ciphertext), Some(nonce)) = (
        delivery.signing_secret_encrypted.as_deref(),
        delivery.signing_secret_nonce.as_deref(),
    ) else {
        return Err("The app has no signing secret".into());
    };
    let secret = open_secret(
        state,
        ciphertext,
        nonce,
        delivery.signing_secret_key_id.as_deref(),
    )
    .map_err(|e| e.to_string())?;
    let body = serde_json::to_vec(&delivery.body).map_err(|e| e.to_string())?;

    let mut request = http
        .post(target)
        .header("Content-Type", "application/json")
        .header(
            executor::SIGNATURE_HEADER,
            executor::sign_body(&secret, &body),
        )
        .timeout(REQUEST_TIMEOUT);
    if delivery.attempts > 1 {
        request = request.header(RETRY_NUM_HEADER, (delivery.attempts - 1).to_string());
    }
    let response = request
        .body(body)
        .send()
        .await
        .map_err(|e| format!("The request failed: {e}"))?;
    let status = response.status();
    if status.is_success() {
        Ok(())
    } else {
        Err(format!("The request URL answered {status}"))
    }
}

pub async fn send_due(state: &AppState, http: &reqwest::Client) -> usize {
    let claimed = match state.app_event_repo.claim_http(LEASE_SECS, BATCH).await {
        Ok(claimed) => claimed,
        Err(e) => {
            warn!("App events: failed to claim deliveries: {e}");
            return 0;
        }
    };
    let count = claimed.len();
    futures_util::stream::iter(claimed)
        .for_each_concurrent(IN_FLIGHT, |delivery| send_one(state, http, delivery))
        .await;
    count
}

pub async fn sweep(state: &AppState) {
    if let Err(e) = state.app_event_repo.give_up_on_expired().await {
        warn!("App events: failed to give up on expired deliveries: {e}");
    }
    if let Err(e) = state.app_event_repo.purge(KEEP_SETTLED_SECS).await {
        warn!("App events: failed to purge settled deliveries: {e}");
    }
}

pub async fn start_sender(state: Arc<AppState>) {
    let http = match client() {
        Ok(http) => http,
        Err(e) => {
            warn!("App event sender: failed to build HTTP client: {e}");
            return;
        }
    };
    info!("App event sender started");
    let mut swept = tokio::time::Instant::now();
    loop {
        if swept.elapsed() >= SWEEP_EVERY {
            sweep(&state).await;
            swept = tokio::time::Instant::now();
        }
        if send_due(&state, &http).await == 0 {
            tokio::time::sleep(IDLE).await;
        }
    }
}
