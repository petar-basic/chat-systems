use sqlx::PgPool;

use shared_common::errors::AppResult;

use super::repo::{self, Stored};
use super::{open, open_with_jwt_secret, seal, Keyring, Sealed, APP_SECRETS, TOTP_SECRETS};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub resealed: usize,
    pub unreadable: Vec<String>,
}

impl Report {
    fn record(&mut self, stored: &Stored, outcome: AppResult<bool>) {
        match outcome {
            Ok(true) => self.resealed += 1,
            Ok(false) => {}
            Err(e) => {
                tracing::error!("{} could not be re-sealed: {e}", stored.id);
                self.unreadable.push(stored.id.clone());
            }
        }
    }
}

fn reseal(
    keyring: &Keyring,
    jwt_secret: &str,
    purpose: &str,
    stored: &Stored,
) -> AppResult<Sealed> {
    let plaintext = match stored.key_id.as_deref() {
        Some(key_id) => open(
            keyring,
            purpose,
            &stored.ciphertext,
            &stored.nonce,
            Some(key_id),
        )?,
        None => open_with_jwt_secret(jwt_secret, purpose, &stored.ciphertext, &stored.nonce)?,
    };
    seal(keyring, purpose, &plaintext)
}

pub async fn run(pool: &PgPool, keyring: &Keyring, jwt_secret: &str) -> AppResult<Report> {
    let current = keyring.current_id();
    let mut report = Report::default();

    for app in repo::app_secrets_off(pool, current).await? {
        let outcome = match reseal(keyring, jwt_secret, APP_SECRETS, &app.stored) {
            Ok(sealed) => repo::reseal_app_secret(pool, &app, &sealed)
                .await
                .map_err(Into::into),
            Err(e) => Err(e),
        };
        report.record(&app.stored, outcome);
    }

    for connection in repo::access_tokens_off(pool, current).await? {
        let outcome = match reseal(keyring, jwt_secret, APP_SECRETS, &connection.stored) {
            Ok(sealed) => repo::reseal_access_token(pool, &connection, &sealed)
                .await
                .map_err(Into::into),
            Err(e) => Err(e),
        };
        report.record(&connection.stored, outcome);
    }

    for connection in repo::refresh_tokens_off(pool, current).await? {
        let outcome = match reseal(keyring, jwt_secret, APP_SECRETS, &connection.stored) {
            Ok(sealed) => repo::reseal_refresh_token(pool, &connection, &sealed)
                .await
                .map_err(Into::into),
            Err(e) => Err(e),
        };
        report.record(&connection.stored, outcome);
    }

    for totp in repo::totp_secrets_off(pool, current).await? {
        let outcome = match reseal(keyring, jwt_secret, TOTP_SECRETS, &totp.stored) {
            Ok(sealed) => repo::reseal_totp_secret(pool, &totp, &sealed)
                .await
                .map_err(Into::into),
            Err(e) => Err(e),
        };
        report.record(&totp.stored, outcome);
    }

    if report.unreadable.is_empty() {
        tracing::info!(
            "stored secrets: {} re-sealed, every one is on key {current:?}",
            report.resealed
        );
    } else {
        tracing::error!(
            "stored secrets: {} re-sealed, {} could not be read and are still on an older key; \
             SECRETS_KEYS must list the key they were sealed with",
            report.resealed,
            report.unreadable.len()
        );
    }
    Ok(report)
}
