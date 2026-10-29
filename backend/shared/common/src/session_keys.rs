use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use aws_lc_rs::signature::{Ed25519KeyPair, KeyPair};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use jsonwebtoken::errors::{Error, ErrorKind};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::de::DeserializeOwned;
use serde::Serialize;

const SPKI_PREFIX: [u8; 12] = [
    0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
];
const PUBLIC_KEY_BYTES: usize = 32;

#[cfg(any(test, feature = "testing"))]
pub const TEST_SIGNING_KEYS: &str =
    "test:MC4CAQAwBQYDK2VwBCIEIGI30Lx2rSsmmyhD7LBOZcQoxLcgnMDiUmMNTIEro7Kk";

fn entries(raw: &str) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for entry in raw.split(',').map(str::trim).filter(|e| !e.is_empty()) {
        let (id, encoded) = entry
            .split_once(':')
            .ok_or_else(|| "every entry must be id:base64-key".to_string())?;
        if id.is_empty()
            || !id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(format!("key id {id:?} must be letters, digits, '-' or '_'"));
        }
        if !seen.insert(id.to_string()) {
            return Err(format!("key id {id:?} appears twice"));
        }
        let bytes = STANDARD
            .decode(encoded.trim())
            .map_err(|_| format!("key {id:?} is not base64"))?;
        out.push((id.to_string(), bytes));
    }
    if out.is_empty() {
        return Err("no key given".into());
    }
    Ok(out)
}

#[derive(Clone)]
pub struct VerifyingKeys {
    order: Arc<Vec<(String, [u8; PUBLIC_KEY_BYTES])>>,
    keys: Arc<HashMap<String, DecodingKey>>,
}

impl std::fmt::Debug for VerifyingKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VerifyingKeys")
            .field("ids", &self.ids())
            .finish()
    }
}

impl VerifyingKeys {
    pub fn parse(raw: &str) -> Result<Self, String> {
        let mut keys = Vec::new();
        for (id, der) in entries(raw)? {
            let public: [u8; PUBLIC_KEY_BYTES] = der
                .strip_prefix(SPKI_PREFIX.as_slice())
                .and_then(|key| key.try_into().ok())
                .ok_or_else(|| format!("key {id:?} is not an Ed25519 public key"))?;
            keys.push((id, public));
        }
        Ok(Self::from_public(keys))
    }

    fn from_public(order: Vec<(String, [u8; PUBLIC_KEY_BYTES])>) -> Self {
        let keys = order
            .iter()
            .map(|(id, public)| (id.clone(), DecodingKey::from_ed_der(public)))
            .collect();
        Self {
            order: Arc::new(order),
            keys: Arc::new(keys),
        }
    }

    pub fn ids(&self) -> Vec<&str> {
        self.order.iter().map(|(id, _)| id.as_str()).collect()
    }

    pub fn listing(&self) -> String {
        self.order
            .iter()
            .map(|(id, public)| {
                let mut der = SPKI_PREFIX.to_vec();
                der.extend_from_slice(public);
                format!("{id}:{}", STANDARD.encode(der))
            })
            .collect::<Vec<_>>()
            .join(",")
    }

    pub fn decode<T: DeserializeOwned>(&self, token: &str) -> Result<T, Error> {
        let header = jsonwebtoken::decode_header(token)?;
        if header.alg != Algorithm::EdDSA {
            return Err(ErrorKind::InvalidAlgorithm.into());
        }
        let key = header
            .kid
            .as_deref()
            .and_then(|kid| self.keys.get(kid))
            .ok_or_else(|| Error::from(ErrorKind::InvalidToken))?;
        Ok(jsonwebtoken::decode::<T>(token, key, &Validation::new(Algorithm::EdDSA))?.claims)
    }
}

#[derive(Clone)]
pub struct SigningKeys {
    current: String,
    encoding: EncodingKey,
    verifying: VerifyingKeys,
}

impl std::fmt::Debug for SigningKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SigningKeys")
            .field("current", &self.current)
            .field("ids", &self.verifying.ids())
            .finish()
    }
}

impl SigningKeys {
    pub fn parse(raw: &str) -> Result<Self, String> {
        let mut current = None;
        let mut public = Vec::new();
        for (id, der) in entries(raw)? {
            let pair = Ed25519KeyPair::from_pkcs8(&der)
                .map_err(|_| format!("key {id:?} is not an Ed25519 private key in PKCS#8"))?;
            let key: [u8; PUBLIC_KEY_BYTES] = pair
                .public_key()
                .as_ref()
                .try_into()
                .map_err(|_| format!("key {id:?} has no 32-byte public half"))?;
            current.get_or_insert_with(|| (id.clone(), EncodingKey::from_ed_der(&der)));
            public.push((id, key));
        }
        let (current, encoding) = current.ok_or_else(|| "no key given".to_string())?;
        Ok(Self {
            current,
            encoding,
            verifying: VerifyingKeys::from_public(public),
        })
    }

    pub fn current_id(&self) -> &str {
        &self.current
    }

    pub fn verifying(&self) -> &VerifyingKeys {
        &self.verifying
    }

    pub fn encode<T: Serialize>(&self, claims: &T) -> Result<String, Error> {
        let mut header = Header::new(Algorithm::EdDSA);
        header.kid = Some(self.current.clone());
        jsonwebtoken::encode(&header, claims, &self.encoding)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    const A: &str = "a:MC4CAQAwBQYDK2VwBCIEIGI30Lx2rSsmmyhD7LBOZcQoxLcgnMDiUmMNTIEro7Kk";
    const B: &str = "b:MC4CAQAwBQYDK2VwBCIEIMmVeJctnPI4cTDrkYWTEXJUa2oijgBEjxlvG4x9LXe/";
    const A_PUBLIC: &str = "a:MCowBQYDK2VwAyEApxzjT5NaCoM2MEpVGExZHIEbu4Q9d860CyDsO1lqJQs=";

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Claims {
        sub: String,
        exp: i64,
    }

    fn claims() -> Claims {
        Claims {
            sub: "someone".into(),
            exp: 4_102_444_800,
        }
    }

    fn keys(raw: &str) -> SigningKeys {
        SigningKeys::parse(raw).expect("keys")
    }

    #[test]
    fn a_token_names_its_key_and_verifies() {
        let signing = keys(A);
        let token = signing.encode(&claims()).expect("encode");
        let header = jsonwebtoken::decode_header(&token).expect("header");
        assert_eq!(header.alg, Algorithm::EdDSA);
        assert_eq!(header.kid.as_deref(), Some("a"));
        assert_eq!(
            signing
                .verifying()
                .decode::<Claims>(&token)
                .expect("decode"),
            claims()
        );
    }

    #[test]
    fn the_public_half_alone_verifies() {
        let token = keys(A).encode(&claims()).expect("encode");
        let gateway = VerifyingKeys::parse(A_PUBLIC).expect("public");
        assert_eq!(gateway.decode::<Claims>(&token).expect("decode"), claims());
        assert_eq!(keys(A).verifying().listing(), A_PUBLIC);
    }

    #[test]
    fn a_rotated_ring_still_verifies_what_the_old_key_signed() {
        let old = keys(A).encode(&claims()).expect("encode");
        let rotated = keys(&format!("{B},{A}"));
        assert_eq!(rotated.current_id(), "b");
        assert!(rotated.verifying().decode::<Claims>(&old).is_ok());
        let new = rotated.encode(&claims()).expect("encode");
        assert_eq!(
            jsonwebtoken::decode_header(&new)
                .expect("header")
                .kid
                .as_deref(),
            Some("b")
        );
    }

    #[test]
    fn a_retired_or_unknown_key_is_refused() {
        let old = keys(A).encode(&claims()).expect("encode");
        assert!(keys(B).verifying().decode::<Claims>(&old).is_err());

        let impostor = keys(&B.replacen("b:", "a:", 1))
            .encode(&claims())
            .expect("encode");
        assert!(keys(A).verifying().decode::<Claims>(&impostor).is_err());
    }

    #[test]
    fn hs256_and_alg_none_are_refused() {
        let mut header = Header::new(Algorithm::HS256);
        header.kid = Some("a".into());
        let hs = jsonwebtoken::encode(
            &header,
            &claims(),
            &EncodingKey::from_secret(b"a-shared-secret-of-thirty-two-bytes"),
        )
        .expect("encode");
        assert!(keys(A).verifying().decode::<Claims>(&hs).is_err());

        let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let none = format!(
            "{}.{}.",
            b64.encode(r#"{"alg":"none","typ":"JWT","kid":"a"}"#),
            b64.encode(serde_json::to_vec(&claims()).expect("json"))
        );
        assert!(keys(A).verifying().decode::<Claims>(&none).is_err());
    }

    #[test]
    fn a_token_without_a_kid_is_refused() {
        let signing = keys(A);
        let token =
            jsonwebtoken::encode(&Header::new(Algorithm::EdDSA), &claims(), &signing.encoding)
                .expect("encode");
        assert!(signing.verifying().decode::<Claims>(&token).is_err());
    }

    #[test]
    fn malformed_keys_are_refused() {
        assert!(SigningKeys::parse("").is_err());
        assert!(SigningKeys::parse("no-colon").is_err());
        assert!(SigningKeys::parse("a:not base64!").is_err());
        assert!(SigningKeys::parse("a:c2hvcnQ=").is_err());
        assert!(SigningKeys::parse(&format!("{A},{A}")).is_err());
        assert!(SigningKeys::parse(A_PUBLIC).is_err());
        assert!(VerifyingKeys::parse(A).is_err());
        assert!(VerifyingKeys::parse("").is_err());
    }

    #[test]
    fn debug_shows_no_key_material() {
        let shown = format!("{:?}", keys(&format!("{A},{B}")));
        assert!(shown.contains("\"a\""));
        assert!(!shown.contains("MC4C"));
    }
}
