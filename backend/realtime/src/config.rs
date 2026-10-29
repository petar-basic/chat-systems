use tracing::info;

use shared_common::session_keys::VerifyingKeys;

use crate::connection_manager::{HuddleSfu, HUDDLE_MAX_PARTICIPANTS};

#[derive(Debug, Clone)]
pub struct RealtimeConfig {
    pub redis_url: String,
    pub database_url: String,
    pub jwt_public_keys: VerifyingKeys,
    pub port: u16,
    pub cors_origins: String,
    pub pg_pool_max: u32,
    pub huddle_sfu: Option<HuddleSfu>,
}

impl RealtimeConfig {
    pub fn from_env() -> Self {
        let config = Self {
            redis_url: env_or("REDIS_URL", "redis://127.0.0.1:6379"),
            database_url: env_or(
                "DATABASE_URL",
                "postgres://chat:chat@localhost:5432/chatsystems",
            ),
            jwt_public_keys: jwt_public_keys_from_env(),
            port: parse_env("PORT", 3004),
            cors_origins: env_or("CORS_ORIGINS", "http://localhost:3001"),
            pg_pool_max: parse_env("PG_POOL_MAX", 5),
            huddle_sfu: huddle_sfu_from_env(),
        };
        info!("Config loaded: port={}", config.port);
        config
    }
}

fn jwt_public_keys_from_env() -> VerifyingKeys {
    let raw = std::env::var("JWT_PUBLIC_KEYS").unwrap_or_default();
    VerifyingKeys::parse(&raw).unwrap_or_else(|e| {
        panic!(
            "JWT_PUBLIC_KEYS must list the public half of every key in the API's \
             JWT_SIGNING_KEYS ({e}). The API logs the line to copy when it starts."
        )
    })
}

fn huddle_sfu_from_env() -> Option<HuddleSfu> {
    if env_or("LIVEKIT_URL", "").is_empty() {
        return None;
    }
    let threshold = parse_env("HUDDLE_SFU_THRESHOLD", 6);
    if threshold >= HUDDLE_MAX_PARTICIPANTS {
        panic!(
            "HUDDLE_SFU_THRESHOLD must be below {HUDDLE_MAX_PARTICIPANTS}, the most a mesh holds, \
             or a huddle fills up before it ever moves to the SFU"
        );
    }
    Some(HuddleSfu {
        threshold,
        max_participants: parse_env("HUDDLE_SFU_MAX_PARTICIPANTS", 100),
    })
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn parse_env<T>(key: &str, default: T) -> T
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    match std::env::var(key) {
        Ok(raw) => raw.parse().unwrap_or_else(|e| {
            panic!("{key} has invalid value {raw:?}: {e}");
        }),
        Err(_) => default,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    const PUBLIC_KEYS: &str = "k1:MCowBQYDK2VwAyEApxzjT5NaCoM2MEpVGExZHIEbu4Q9d860CyDsO1lqJQs=,k0:MCowBQYDK2VwAyEAnQEMGgN/qh38DOoqO5rmPogWyV7Z6k0dK6TC7ew9YpU=";
    const PRIVATE_KEY: &str = "k1:MC4CAQAwBQYDK2VwBCIEIGI30Lx2rSsmmyhD7LBOZcQoxLcgnMDiUmMNTIEro7Kk";

    struct Saved {
        key: &'static str,
        prev: Option<String>,
    }

    fn set_saved(key: &'static str, value: &str) -> Saved {
        let prev = std::env::var(key).ok();
        std::env::set_var(key, value);
        Saved { key, prev }
    }

    fn unset_saved(key: &'static str) -> Saved {
        let prev = std::env::var(key).ok();
        std::env::remove_var(key);
        Saved { key, prev }
    }

    fn restore(saved: Vec<Saved>) {
        for s in saved {
            match s.prev {
                Some(v) => std::env::set_var(s.key, v),
                None => std::env::remove_var(s.key),
            }
        }
    }

    #[test]
    fn from_env_applies_defaults_and_parses_set_values() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let saved = vec![
            set_saved("JWT_PUBLIC_KEYS", PUBLIC_KEYS),
            set_saved(
                "DATABASE_URL",
                "postgres://chat:chat@localhost:5432/chatsystems",
            ),
            set_saved("REDIS_URL", "redis://127.0.0.1:6379"),
            unset_saved("CORS_ORIGINS"),
            set_saved("PORT", "9090"),
        ];

        let config = RealtimeConfig::from_env();

        assert_eq!(config.cors_origins, "http://localhost:3001");
        assert_eq!(config.port, 9090u16);
        assert_eq!(config.jwt_public_keys.ids(), vec!["k1", "k0"]);

        restore(saved);
    }

    #[test]
    fn the_sfu_is_on_only_with_livekit_and_a_threshold_the_mesh_can_reach() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let saved = vec![
            set_saved("JWT_PUBLIC_KEYS", PUBLIC_KEYS),
            unset_saved("LIVEKIT_URL"),
            unset_saved("HUDDLE_SFU_THRESHOLD"),
            unset_saved("HUDDLE_SFU_MAX_PARTICIPANTS"),
        ];

        let off = RealtimeConfig::from_env().huddle_sfu;
        std::env::set_var("LIVEKIT_URL", "wss://sfu.example.com");
        let on = RealtimeConfig::from_env().huddle_sfu;
        std::env::set_var("HUDDLE_SFU_THRESHOLD", HUDDLE_MAX_PARTICIPANTS.to_string());
        let unreachable = std::panic::catch_unwind(RealtimeConfig::from_env);

        restore(saved);

        assert_eq!(off, None);
        assert_eq!(
            on,
            Some(HuddleSfu {
                threshold: 6,
                max_participants: 100,
            })
        );
        assert!(
            unreachable.is_err(),
            "a threshold the mesh never reaches must refuse to start"
        );
    }

    #[test]
    fn from_env_refuses_a_private_key() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let saved = vec![
            set_saved("JWT_PUBLIC_KEYS", PRIVATE_KEY),
            set_saved(
                "DATABASE_URL",
                "postgres://chat:chat@localhost:5432/chatsystems",
            ),
            set_saved("REDIS_URL", "redis://127.0.0.1:6379"),
        ];

        let result = std::panic::catch_unwind(RealtimeConfig::from_env);

        restore(saved);

        assert!(
            result.is_err(),
            "the gateway takes public keys only, never something that can sign"
        );
    }

    #[test]
    fn from_env_panics_without_public_keys() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let saved = vec![
            unset_saved("JWT_PUBLIC_KEYS"),
            set_saved(
                "DATABASE_URL",
                "postgres://chat:chat@localhost:5432/chatsystems",
            ),
            set_saved("REDIS_URL", "redis://127.0.0.1:6379"),
        ];

        let result = std::panic::catch_unwind(RealtimeConfig::from_env);

        restore(saved);

        assert!(result.is_err(), "a missing JWT_PUBLIC_KEYS must panic");
    }
}
