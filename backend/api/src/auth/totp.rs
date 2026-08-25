use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use rand::RngExt;
use sha2::{Digest, Sha256};
use totp_rs::{Algorithm, Builder, Secret, Totp};

use shared_common::errors::{AppError, AppResult};

const STEP_SECS: u64 = 30;
const DIGITS: u8 = 6;
/// One step either side, which is the usual allowance for clock drift.
const SKEW: u16 = 1;
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
    ChaCha20Poly1305::new(&Key::try_from(&key[..]).expect("sha256 is a 32-byte key"))
}

pub fn encrypt_secret(jwt_secret: &str, secret: &str) -> AppResult<(Vec<u8>, Vec<u8>)> {
    // aead 0.6 dropped the nonce helper; twelve random bytes is what it did.
    let mut nonce_bytes = [0u8; 12];
    rand::rng().fill(&mut nonce_bytes[..]);
    let nonce = Nonce::from(nonce_bytes);
    let ciphertext = cipher(jwt_secret)
        .encrypt(&nonce, secret.as_bytes())
        .map_err(|_| AppError::Internal("Failed to protect the TOTP secret".into()))?;
    Ok((ciphertext, nonce_bytes.to_vec()))
}

pub fn decrypt_secret(jwt_secret: &str, ciphertext: &[u8], nonce: &[u8]) -> AppResult<String> {
    let plaintext = cipher(jwt_secret)
        .decrypt(
            &Nonce::try_from(nonce).map_err(|_| AppError::Internal("Corrupt TOTP nonce".into()))?,
            ciphertext,
        )
        .map_err(|_| AppError::Internal("Failed to read the TOTP secret".into()))?;
    String::from_utf8(plaintext).map_err(|_| AppError::Internal("Corrupt TOTP secret".into()))
}

pub fn generate_secret() -> String {
    Secret::generate().to_base32()
}

fn totp(secret: &str, email: &str, issuer: &str) -> AppResult<Totp> {
    let secret = Secret::try_from_base32(secret)
        .map_err(|_| AppError::Internal("Invalid TOTP secret".into()))?;
    Builder::new()
        .with_algorithm(Algorithm::SHA1)
        .with_digits(DIGITS)
        .with_skew(SKEW)
        .with_step_duration(STEP_SECS)
        .with_secret(secret)
        .with_issuer(Some(issuer.to_string()))
        .with_account_name(email.to_string())
        .build()
        .map_err(|e| AppError::Internal(format!("TOTP setup failed: {e}")))
}

pub fn provisioning_uri(secret: &str, email: &str, issuer: &str) -> AppResult<String> {
    totp(secret, email, issuer)?
        .to_url()
        .map_err(|e| AppError::Internal(format!("TOTP url failed: {e}")))
}

/// The step a code belongs to, so a code cannot be replayed while it is still
/// inside its own validity window — checking the code alone would accept the
/// same six digits repeatedly for thirty seconds.
pub fn current_step(at: i64) -> i64 {
    at / STEP_SECS as i64
}

pub fn verify(secret: &str, email: &str, issuer: &str, code: &str) -> AppResult<bool> {
    Ok(matching_step(secret, email, issuer, code, chrono::Utc::now().timestamp())?.is_some())
}

fn codes_match(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes()
        .zip(b.bytes())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

/// The step the code itself belongs to, not the step the request arrived in.
/// Skew means one code stays valid across three steps, so claiming the request's
/// step would let the same digits through again as soon as the clock ticked over
/// — the replay guard has to pin the code to where it came from.
pub fn matching_step(
    secret: &str,
    email: &str,
    issuer: &str,
    code: &str,
    at: i64,
) -> AppResult<Option<i64>> {
    let totp = totp(secret, email, issuer)?;
    let current = current_step(at);
    for step in [
        current,
        current - i64::from(SKEW),
        current + i64::from(SKEW),
    ] {
        if step < 0 {
            continue;
        }
        let generated = totp.generate(step as u64 * STEP_SECS).to_string();
        if codes_match(&generated, code) {
            return Ok(Some(step));
        }
    }
    Ok(None)
}

pub fn generate_recovery_codes() -> Vec<String> {
    (0..RECOVERY_CODE_COUNT)
        .map(|_| {
            let mut bytes = [0u8; 5];
            rand::rng().fill(&mut bytes[..]);
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
    fn a_code_is_pinned_to_the_step_that_produced_it() {
        let secret = generate_secret();
        let at = 1_700_000_000i64;
        let step = current_step(at);
        let code = Builder::new()
            .with_algorithm(Algorithm::SHA1)
            .with_digits(DIGITS)
            .with_skew(SKEW)
            .with_step_duration(STEP_SECS)
            .with_secret(Secret::try_from_base32(&secret).expect("secret"))
            .with_issuer(Some("Chat Systems"))
            .with_account_name("user@test.local")
            .build()
            .expect("totp")
            .generate(at as u64)
            .to_string();

        for offset in [0, 30, -30] {
            assert_eq!(
                matching_step(
                    &secret,
                    "user@test.local",
                    "Chat Systems",
                    &code,
                    at + offset
                )
                .expect("match"),
                Some(step),
                "the same code reports its own step whichever neighbouring step asks"
            );
        }

        assert_eq!(
            matching_step(&secret, "user@test.local", "Chat Systems", &code, at + 90)
                .expect("match"),
            None,
            "outside the skew window the code is simply wrong"
        );
        assert_eq!(
            matching_step(&secret, "user@test.local", "Chat Systems", "000000", at).expect("match"),
            None,
        );
    }

    #[test]
    fn steps_advance_once_every_thirty_seconds() {
        assert_eq!(current_step(0), current_step(29));
        assert_ne!(current_step(0), current_step(30));
    }
}
