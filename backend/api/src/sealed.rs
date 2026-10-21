use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use rand::RngExt;
use sha2::{Digest, Sha256};

use shared_common::errors::{AppError, AppResult};

pub struct Sealed {
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
}

fn cipher(jwt_secret: &str, purpose: &str) -> ChaCha20Poly1305 {
    let mut hasher = Sha256::new();
    hasher.update(purpose.as_bytes());
    hasher.update(b":");
    hasher.update(jwt_secret.as_bytes());
    let key = hasher.finalize();
    ChaCha20Poly1305::new(&Key::try_from(&key[..]).expect("sha256 is a 32-byte key"))
}

pub fn seal(jwt_secret: &str, purpose: &str, plaintext: &str) -> AppResult<Sealed> {
    let mut nonce = [0u8; 12];
    rand::rng().fill(&mut nonce[..]);
    let ciphertext = cipher(jwt_secret, purpose)
        .encrypt(&Nonce::from(nonce), plaintext.as_bytes())
        .map_err(|_| AppError::Internal("Failed to protect a stored secret".into()))?;
    Ok(Sealed {
        ciphertext,
        nonce: nonce.to_vec(),
    })
}

pub fn open(jwt_secret: &str, purpose: &str, ciphertext: &[u8], nonce: &[u8]) -> AppResult<String> {
    let nonce = Nonce::try_from(nonce).map_err(|_| AppError::Internal("Corrupt nonce".into()))?;
    let plaintext = cipher(jwt_secret, purpose)
        .decrypt(&nonce, ciphertext)
        .map_err(|_| AppError::Internal("Failed to read a stored secret".into()))?;
    String::from_utf8(plaintext).map_err(|_| AppError::Internal("Corrupt stored secret".into()))
}

#[cfg(test)]
mod tests {
    use super::{open, seal};

    #[test]
    fn round_trips_and_keeps_purposes_apart() {
        let sealed = seal("jwt", "apps", "token-1").expect("seal");
        assert_eq!(
            open("jwt", "apps", &sealed.ciphertext, &sealed.nonce).expect("open"),
            "token-1"
        );
        assert!(open("jwt", "totp", &sealed.ciphertext, &sealed.nonce).is_err());
        assert!(open("other", "apps", &sealed.ciphertext, &sealed.nonce).is_err());
    }
}
