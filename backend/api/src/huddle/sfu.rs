use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde::Serialize;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use crate::config::AppConfig;

pub const TOKEN_TTL_SECS: i64 = 600;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VideoGrant<'a> {
    room_join: bool,
    room: &'a str,
    can_publish: bool,
    can_subscribe: bool,
    can_publish_data: bool,
}

#[derive(Serialize)]
struct Claims<'a> {
    iss: &'a str,
    sub: String,
    nbf: i64,
    exp: i64,
    video: VideoGrant<'a>,
}

pub fn room_for(huddle_id: Uuid) -> String {
    format!("huddle-{huddle_id}")
}

pub fn access_token(
    config: &AppConfig,
    room: &str,
    identity: Uuid,
    now: DateTime<Utc>,
) -> AppResult<(String, DateTime<Utc>)> {
    let expires_at = now + Duration::seconds(TOKEN_TTL_SECS);
    let claims = Claims {
        iss: &config.livekit_api_key,
        sub: identity.to_string(),
        nbf: now.timestamp(),
        exp: expires_at.timestamp(),
        video: VideoGrant {
            room_join: true,
            room,
            can_publish: true,
            can_subscribe: true,
            can_publish_data: false,
        },
    };
    let token = jsonwebtoken::encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(config.livekit_api_secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(format!("Could not sign the SFU token: {e}")))?;
    Ok((token, expires_at))
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{DecodingKey, Validation};

    fn config() -> AppConfig {
        AppConfig {
            livekit_url: "wss://sfu.example.com".into(),
            livekit_api_key: "APIkey".into(),
            livekit_api_secret: "s".repeat(32),
            ..AppConfig::for_tests()
        }
    }

    #[test]
    fn a_token_lets_one_person_into_one_room_for_ten_minutes() {
        let user = Uuid::new_v4();
        let now = Utc::now();
        let (token, expires_at) = access_token(&config(), "huddle-x", user, now).expect("token");

        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(&["APIkey"]);
        let claims = jsonwebtoken::decode::<serde_json::Value>(
            &token,
            &DecodingKey::from_secret("s".repeat(32).as_bytes()),
            &validation,
        )
        .expect("signed with the API secret")
        .claims;

        assert_eq!(claims["sub"], user.to_string());
        assert_eq!(claims["exp"], expires_at.timestamp());
        assert_eq!(expires_at - now, Duration::minutes(10));
        assert_eq!(
            claims["video"],
            serde_json::json!({
                "roomJoin": true,
                "room": "huddle-x",
                "canPublish": true,
                "canSubscribe": true,
                "canPublishData": false,
            })
        );
    }

    #[test]
    fn a_room_is_named_after_its_huddle() {
        let id = Uuid::nil();
        assert_eq!(room_for(id), format!("huddle-{id}"));
    }
}
