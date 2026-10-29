mod repo;
pub mod reseal;

use std::collections::HashMap;

use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use rand::RngExt;
use sha2::{Digest, Sha256};

use shared_common::errors::{AppError, AppResult};

const KEY_BYTES: usize = 32;

pub const APP_SECRETS: &str = "workspace-app-secrets";
pub const TOTP_SECRETS: &str = "totp-secret-encryption";
pub const SETUP_TOKEN: &str = "setup-token";
pub const COMMAND_RESPONSES: &str = "command-responses";

#[derive(Clone)]
pub struct Keyring {
    current: String,
    keys: HashMap<String, [u8; KEY_BYTES]>,
}

impl std::fmt::Debug for Keyring {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut ids: Vec<&String> = self.keys.keys().collect();
        ids.sort();
        f.debug_struct("Keyring")
            .field("current", &self.current)
            .field("ids", &ids)
            .finish()
    }
}

impl Keyring {
    pub fn parse(raw: &str) -> Result<Self, String> {
        let mut current = None;
        let mut keys = HashMap::new();
        for entry in raw.split(',').map(str::trim).filter(|e| !e.is_empty()) {
            let (id, encoded) = entry
                .split_once(':')
                .ok_or_else(|| format!("{entry:?} is not id:base64-key"))?;
            if id.is_empty()
                || !id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                return Err(format!("key id {id:?} must be letters, digits, '-' or '_'"));
            }
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(encoded.trim())
                .map_err(|_| format!("key {id:?} is not base64"))?;
            let key: [u8; KEY_BYTES] = bytes
                .try_into()
                .map_err(|_| format!("key {id:?} must be {KEY_BYTES} bytes"))?;
            if keys.insert(id.to_string(), key).is_some() {
                return Err(format!("key id {id:?} appears twice"));
            }
            current.get_or_insert_with(|| id.to_string());
        }
        let current = current.ok_or_else(|| "no key given".to_string())?;
        Ok(Self { current, keys })
    }

    pub fn current_id(&self) -> &str {
        &self.current
    }

    pub fn derive(&self, purpose: &str) -> [u8; KEY_BYTES] {
        derive(&self.keys[&self.current], purpose)
    }

    fn cipher(&self, key_id: &str, purpose: &str) -> AppResult<ChaCha20Poly1305> {
        let key = self.keys.get(key_id).ok_or_else(|| {
            AppError::Internal(format!(
                "A stored secret was sealed with key {key_id:?}, which SECRETS_KEYS no longer lists"
            ))
        })?;
        Ok(cipher_from(derive(key, purpose)))
    }
}

pub struct Sealed {
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
    pub key_id: String,
}

fn derive(secret: &[u8], purpose: &str) -> [u8; KEY_BYTES] {
    let mut hasher = Sha256::new();
    hasher.update(purpose.as_bytes());
    hasher.update(b":");
    hasher.update(secret);
    hasher.finalize().into()
}

fn cipher_from(key: [u8; KEY_BYTES]) -> ChaCha20Poly1305 {
    ChaCha20Poly1305::new(&Key::from(key))
}

pub fn seal(keyring: &Keyring, purpose: &str, plaintext: &str) -> AppResult<Sealed> {
    let mut nonce = [0u8; 12];
    rand::rng().fill(&mut nonce[..]);
    let ciphertext = keyring
        .cipher(keyring.current_id(), purpose)?
        .encrypt(&Nonce::from(nonce), plaintext.as_bytes())
        .map_err(|_| AppError::Internal("Failed to protect a stored secret".into()))?;
    Ok(Sealed {
        ciphertext,
        nonce: nonce.to_vec(),
        key_id: keyring.current_id().to_string(),
    })
}

pub fn open(
    keyring: &Keyring,
    purpose: &str,
    ciphertext: &[u8],
    nonce: &[u8],
    key_id: Option<&str>,
) -> AppResult<String> {
    let key_id = key_id.ok_or_else(|| {
        AppError::Internal(
            "A stored secret is still sealed with JWT_SECRET; the API re-seals it when it starts"
                .into(),
        )
    })?;
    decrypt(&keyring.cipher(key_id, purpose)?, ciphertext, nonce)
}

fn open_with_jwt_secret(
    jwt_secret: &str,
    purpose: &str,
    ciphertext: &[u8],
    nonce: &[u8],
) -> AppResult<String> {
    decrypt(
        &cipher_from(derive(jwt_secret.as_bytes(), purpose)),
        ciphertext,
        nonce,
    )
}

fn decrypt(cipher: &ChaCha20Poly1305, ciphertext: &[u8], nonce: &[u8]) -> AppResult<String> {
    let nonce = Nonce::try_from(nonce).map_err(|_| AppError::Internal("Corrupt nonce".into()))?;
    let plaintext = cipher
        .decrypt(&nonce, ciphertext)
        .map_err(|_| AppError::Internal("Failed to read a stored secret".into()))?;
    String::from_utf8(plaintext).map_err(|_| AppError::Internal("Corrupt stored secret".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(byte: u8) -> String {
        base64::engine::general_purpose::STANDARD.encode([byte; KEY_BYTES])
    }

    #[test]
    fn round_trips_and_keeps_purposes_apart() {
        let ring = Keyring::parse(&format!("a:{}", key(1))).expect("ring");
        let sealed = seal(&ring, "apps", "token-1").expect("seal");
        assert_eq!(sealed.key_id, "a");
        assert_eq!(
            open(&ring, "apps", &sealed.ciphertext, &sealed.nonce, Some("a")).expect("open"),
            "token-1"
        );
        assert!(open(&ring, "totp", &sealed.ciphertext, &sealed.nonce, Some("a")).is_err());
    }

    #[test]
    fn opens_with_the_key_a_value_names_and_seals_with_the_first() {
        let old = Keyring::parse(&format!("old:{}", key(1))).expect("ring");
        let sealed = seal(&old, "apps", "token-1").expect("seal");

        let rotated = Keyring::parse(&format!("new:{},old:{}", key(2), key(1))).expect("ring");
        assert_eq!(rotated.current_id(), "new");
        assert_eq!(
            open(
                &rotated,
                "apps",
                &sealed.ciphertext,
                &sealed.nonce,
                Some("old")
            )
            .expect("open"),
            "token-1"
        );
        assert_eq!(
            seal(&rotated, "apps", "token-2").expect("seal").key_id,
            "new"
        );

        let retired = Keyring::parse(&format!("new:{}", key(2))).expect("ring");
        let err = open(
            &retired,
            "apps",
            &sealed.ciphertext,
            &sealed.nonce,
            Some("old"),
        )
        .expect_err("a retired key cannot open");
        assert!(err.to_string().contains("\"old\""), "{err}");
    }

    #[test]
    fn reads_what_was_sealed_under_the_jwt_secret() {
        let legacy = cipher_from(derive(b"jwt-secret", "apps"));
        let nonce = [7u8; 12];
        let ciphertext = legacy
            .encrypt(&Nonce::from(nonce), b"token-0".as_slice())
            .expect("encrypt");
        assert_eq!(
            open_with_jwt_secret("jwt-secret", "apps", &ciphertext, &nonce).expect("open"),
            "token-0"
        );
    }

    #[test]
    fn a_value_with_no_key_id_is_not_opened_at_runtime() {
        let ring = Keyring::parse(&format!("a:{}", key(1))).expect("ring");
        let sealed = seal(&ring, "apps", "token-1").expect("seal");
        assert!(open(&ring, "apps", &sealed.ciphertext, &sealed.nonce, None).is_err());
    }

    #[test]
    fn refuses_a_malformed_ring() {
        assert!(Keyring::parse("").is_err());
        assert!(Keyring::parse("nokey").is_err());
        assert!(Keyring::parse("a:not-base64!").is_err());
        assert!(Keyring::parse(&format!(
            "a:{}",
            base64::engine::general_purpose::STANDARD.encode([1u8; 16])
        ))
        .is_err());
        assert!(Keyring::parse(&format!("a:{},a:{}", key(1), key(2))).is_err());
        assert!(Keyring::parse(&format!("a b:{}", key(1))).is_err());
    }
}
