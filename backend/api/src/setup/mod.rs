pub mod models;
pub mod repo;
pub mod routes;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use hmac::{Hmac, KeyInit, Mac};
use rand::RngExt;
use sha2::Sha256;
use tracing::warn;

use shared_common::errors::AppResult;

use crate::state::AppState;

type HmacSha256 = Hmac<Sha256>;

const TOKEN_BYTES: usize = 18;

fn mac(secret: &str, nonce: &str) -> HmacSha256 {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(b"instance-setup:");
    mac.update(nonce.as_bytes());
    mac
}

pub fn token_for(secret: &str, nonce: &str) -> String {
    URL_SAFE_NO_PAD.encode(&mac(secret, nonce).finalize().into_bytes()[..TOKEN_BYTES])
}

pub fn token_matches(secret: &str, nonce: &str, token: &str) -> bool {
    URL_SAFE_NO_PAD.decode(token.trim()).is_ok_and(|bytes| {
        bytes.len() == TOKEN_BYTES && mac(secret, nonce).verify_truncated_left(&bytes).is_ok()
    })
}

fn fresh_nonce() -> String {
    let mut bytes = [0u8; 24];
    rand::rng().fill(&mut bytes[..]);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub async fn announce(state: &AppState) -> AppResult<()> {
    if state.setup_repo.has_instance_admin().await? {
        state.setup_repo.clear().await?;
        return Ok(());
    }
    let nonce = state.setup_repo.ensure_nonce(&fresh_nonce()).await?;
    let token = token_for(&state.config.jwt_secret, &nonce);
    warn!(
        "This instance has no admin yet. Open {}/setup and enter the setup token {}",
        state.config.public_url.trim_end_matches('/'),
        token
    );
    Ok(())
}
