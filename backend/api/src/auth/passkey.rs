use std::time::Duration;

use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use shared_common::errors::{AppError, AppResult};
use uuid::Uuid;
use webauthn_rs::prelude::{
    DiscoverableAuthentication, PasskeyAuthentication, PasskeyRegistration, Url, Webauthn,
    WebauthnBuilder,
};

use crate::config::AppConfig;
use crate::state::AppState;

pub const CHALLENGE_TTL_SECS: u64 = 120;
pub const MAX_PER_ACCOUNT: i64 = 10;

#[derive(Serialize, Deserialize)]
pub enum Ceremony {
    Registration {
        user_id: Uuid,
        state: PasskeyRegistration,
    },
    SignIn {
        state: DiscoverableAuthentication,
    },
    SecondFactor {
        user_id: Uuid,
        state: PasskeyAuthentication,
    },
}

pub fn relying_party(config: &AppConfig) -> AppResult<Webauthn> {
    let origin = Url::parse(&config.public_url)
        .map_err(|e| AppError::Internal(format!("PUBLIC_URL is not a URL: {e}")))?;
    let rp_id = origin
        .host_str()
        .ok_or_else(|| AppError::Internal("PUBLIC_URL has no host".into()))?
        .to_string();
    WebauthnBuilder::new(&rp_id, &origin)
        .map_err(|e| AppError::Internal(format!("Passkeys are misconfigured: {e}")))?
        .rp_name(&config.instance_name)
        .allow_any_port(rp_id == "localhost")
        .timeout(Duration::from_secs(CHALLENGE_TTL_SECS))
        .build()
        .map_err(|e| AppError::Internal(format!("Passkeys are misconfigured: {e}")))
}

pub fn browser_options<T: Serialize>(challenge: &T) -> AppResult<serde_json::Value> {
    let mut options = serde_json::to_value(challenge)
        .map_err(|e| AppError::Internal(format!("Could not describe the passkey request: {e}")))?;
    if let Some(fields) = options.as_object_mut() {
        fields.remove("mediation");
    }
    Ok(options)
}

pub fn require_discoverable(options: &mut serde_json::Value) {
    if let Some(selection) = options
        .pointer_mut("/publicKey/authenticatorSelection")
        .and_then(|s| s.as_object_mut())
    {
        selection.insert("residentKey".into(), "required".into());
        selection.insert("requireResidentKey".into(), true.into());
    }
}

fn challenge_key(id: Uuid) -> String {
    format!("passkey:challenge:{id}")
}

pub async fn stash(state: &AppState, ceremony: &Ceremony) -> AppResult<Uuid> {
    let id = Uuid::new_v4();
    let payload = serde_json::to_string(ceremony)
        .map_err(|e| AppError::Internal(format!("Could not store the passkey request: {e}")))?;
    let mut conn = state.redis.clone();
    let _: () = conn
        .set_ex(challenge_key(id), payload, CHALLENGE_TTL_SECS)
        .await
        .map_err(|e| AppError::Internal(format!("Could not store the passkey request: {e}")))?;
    Ok(id)
}

pub async fn take(state: &AppState, id: Uuid) -> AppResult<Ceremony> {
    let mut conn = state.redis.clone();
    let stored: Option<String> = conn
        .get_del(challenge_key(id))
        .await
        .map_err(|e| AppError::Internal(format!("Could not read the passkey request: {e}")))?;
    stored
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .ok_or_else(|| {
            AppError::Unauthorized("That passkey request expired, please try again".into())
        })
}
