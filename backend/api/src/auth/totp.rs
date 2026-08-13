use chacha20poly1305::aead::{Aead, KeyInit, OsRng};
use chacha20poly1305::{AeadCore, ChaCha20Poly1305, Key, Nonce};
use sha2::{Digest, Sha256};
use totp_rs::{Algorithm, Secret, TOTP};

use shared_common::errors::{AppError, AppResult};

const STEP_SECS: u64 = 30;
const DIGITS: usize = 6;
/// One step either side, which is the usual allowance for clock drift.
const SKEW: u8 = 1;
pub const RECOVERY_CODE_COUNT: usize = 10;

/// The TOTP secret is encrypted with a key derived from the instance's JWT
/// secret. A database dump full of TOTP secrets is the same failure as one full
/// of passwords, and the second factor is worth nothing if the first breach
/// hands it over too.
fn cipher(jwt_secret: &str) -> ChaCha20Poly1305 {
    let mut hasher = Sha256::new();
    hasher.update(b"totp-secret-encryption:");
    hasher.update(jwt_secret.as_bytes());
    let key = hasher.finalize();
    ChaCha20Poly1305::new(Key::from_slice(&key))
}

pub fn encrypt_secret(jwt_secret: &str, secret: &str) -> AppResult<(Vec<u8>, Vec<u8>)> {
    let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
    let ciphertext = cipher(jwt_secret)
        .encrypt(&nonce, secret.as_bytes())
        .map_err(|_| AppError::Internal("Failed to protect the TOTP secret".into()))?;
    Ok((ciphertext, nonce.to_vec()))
}

pub fn decrypt_secret(jwt_secret: &str, ciphertext: &[u8], nonce: &[u8]) -> AppResult<String> {
    let plaintext = cipher(jwt_secret)
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| AppError::Internal("Failed to read the TOTP secret".into()))?;
    String::from_utf8(plaintext).map_err(|_| AppError::Internal("Corrupt TOTP secret".into()))
}

pub fn generate_secret() -> String {
    Secret::generate_secret().to_encoded().to_string()
}

fn totp(secret: &str, email: &str, issuer: &str) -> AppResult<TOTP> {
    let bytes = Secret::Encoded(secret.to_string())
        .to_bytes()
        .map_err(|_| AppError::Internal("Invalid TOTP secret".into()))?;
    TOTP::new(
        Algorithm::SHA1,
        DIGITS,
        SKEW,
        STEP_SECS,
        bytes,
        Some(issuer.to_string()),
        email.to_string(),
    )
    .map_err(|e| AppError::Internal(format!("TOTP setup failed: {e}")))
}

pub fn provisioning_uri(secret: &str, email: &str, issuer: &str) -> AppResult<String> {
    Ok(totp(secret, email, issuer)?.get_url())
}

/// The step a code belongs to, so a code cannot be replayed while it is still
/// inside its own validity window — checking the code alone would accept the
/// same six digits repeatedly for thirty seconds.
pub fn current_step(at: i64) -> i64 {
    at / STEP_SECS as i64
}

pub fn verify(secret: &str, email: &str, issuer: &str, code: &str) -> AppResult<bool> {
    let totp = totp(secret, email, issuer)?;
    Ok(totp.check_current(code).unwrap_or(false))
}

pub fn generate_recovery_codes() -> Vec<String> {
    use rand::RngCore;
    (0..RECOVERY_CODE_COUNT)
        .map(|_| {
            let mut bytes = [0u8; 5];
            rand::rngs::OsRng.fill_bytes(&mut bytes);
            let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
            format!("{}-{}", &hex[..5], &hex[5..])
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_secret_survives_the_round_trip() {
        let secret = generate_secret();
        let (ciphertext, nonce) = encrypt_secret("instance-secret", &secret).expect("encrypt");
        assert_ne!(
            ciphertext,
            secret.as_bytes(),
            "it is not stored in the clear"
        );
        let back = decrypt_secret("instance-secret", &ciphertext, &nonce).expect("decrypt");
        assert_eq!(back, secret);
    }

    #[test]
    fn a_different_instance_secret_cannot_read_it() {
        let secret = generate_secret();
        let (ciphertext, nonce) = encrypt_secret("instance-secret", &secret).expect("encrypt");
        assert!(decrypt_secret("another-secret", &ciphertext, &nonce).is_err());
    }

    #[test]
    fn the_provisioning_uri_names_the_account_and_issuer() {
        let secret = generate_secret();
        let uri = provisioning_uri(&secret, "user@test.local", "Chat Systems").expect("uri");
        assert!(uri.starts_with("otpauth://totp/"));
        assert!(uri.contains("user%40test.local") || uri.contains("user@test.local"));
        assert!(uri.contains("Chat%20Systems") || uri.contains("Chat Systems"));
    }

    #[test]
    fn recovery_codes_are_distinct_and_shaped_for_typing() {
        let codes = generate_recovery_codes();
        assert_eq!(codes.len(), RECOVERY_CODE_COUNT);
        let unique: std::collections::HashSet<&String> = codes.iter().collect();
        assert_eq!(unique.len(), codes.len());
        assert!(codes.iter().all(|c| c.len() == 11 && c.contains('-')));
    }

    #[test]
    fn steps_advance_once_every_thirty_seconds() {
        assert_eq!(current_step(0), current_step(29));
        assert_ne!(current_step(0), current_step(30));
    }
}
