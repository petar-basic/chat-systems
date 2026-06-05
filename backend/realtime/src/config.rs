use tracing::info;

#[derive(Debug, Clone)]
pub struct RealtimeConfig {
    pub redis_url: String,
    pub database_url: String,
    pub jwt_secret: String,
    pub port: u16,
    pub cors_origins: String,
}

impl RealtimeConfig {
    pub fn from_env() -> Self {
        let config = Self {
            redis_url: env_or("REDIS_URL", "redis://127.0.0.1:6379"),
            database_url: env_or(
                "DATABASE_URL",
                "postgres://chat:chat@localhost:5432/chatsystems",
            ),
            jwt_secret: env_or("JWT_SECRET", "dev-secret-change-me-in-production"),
            port: parse_env("PORT", 3004),
            cors_origins: env_or("CORS_ORIGINS", "http://localhost:3001"),
        };
        if config.jwt_secret == "dev-secret-change-me-in-production" || config.jwt_secret.len() < 32
        {
            panic!(
                "JWT_SECRET must be set to a random value of at least 32 characters \
                 (must match chat-api). Generate one with: openssl rand -hex 32"
            );
        }
        info!("Config loaded: port={}", config.port);
        config
    }
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

    const STRONG_SECRET: &str = "0123456789abcdef0123456789abcdef0123456789abcdef";

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
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        let saved = vec![
            set_saved("JWT_SECRET", STRONG_SECRET),
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
        assert_eq!(config.jwt_secret, STRONG_SECRET);

        restore(saved);
    }

    #[test]
    fn from_env_panics_on_weak_jwt_secret() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        let saved = vec![
            set_saved("JWT_SECRET", "short"),
            set_saved(
                "DATABASE_URL",
                "postgres://chat:chat@localhost:5432/chatsystems",
            ),
            set_saved("REDIS_URL", "redis://127.0.0.1:6379"),
        ];

        let result = std::panic::catch_unwind(RealtimeConfig::from_env);

        restore(saved);

        assert!(result.is_err(), "weak JWT_SECRET must panic");
    }
}
