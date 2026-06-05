use tracing::info;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub port: u16,

    pub database_url: String,

    pub redis_url: String,

    pub jwt_secret: String,
    pub access_token_expiry: i64,
    pub refresh_token_expiry: i64,

    pub admin_email: Option<String>,
    pub admin_password: Option<String>,

    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_user: String,
    pub smtp_password: String,
    pub smtp_from_address: String,
    pub smtp_from_name: String,
    pub smtp_use_tls: bool,

    pub public_url: String,
    pub instance_name: String,
    pub instance_icon_url: Option<String>,

    pub cors_origins: String,

    pub storage_backend: StorageBackend,
    pub local_storage_path: String,
    pub s3_endpoint: String,
    pub s3_region: String,
    pub s3_bucket: String,
    pub s3_access_key: String,
    pub s3_secret_key: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StorageBackend {
    Local,
    S3,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let storage_backend = match env_or("STORAGE_BACKEND", "local").to_lowercase().as_str() {
            "s3" | "minio" => StorageBackend::S3,
            _ => StorageBackend::Local,
        };

        let config = Self {
            port: parse_env("PORT", 3000),
            database_url: env_or(
                "DATABASE_URL",
                "postgres://chat:chat@localhost:5432/chatsystems",
            ),
            redis_url: env_or("REDIS_URL", "redis://127.0.0.1:6379"),
            jwt_secret: env_or("JWT_SECRET", "dev-secret-change-me-in-production"),
            access_token_expiry: parse_env("ACCESS_TOKEN_EXPIRY", 3600),
            refresh_token_expiry: parse_env("REFRESH_TOKEN_EXPIRY", 604800),
            admin_email: std::env::var("ADMIN_EMAIL").ok(),
            admin_password: std::env::var("ADMIN_PASSWORD").ok(),
            smtp_host: env_or("SMTP_HOST", "localhost"),
            smtp_port: parse_env("SMTP_PORT", 1025),
            smtp_user: env_or("SMTP_USER", ""),
            smtp_password: env_or("SMTP_PASSWORD", ""),
            smtp_from_address: env_or("SMTP_FROM_ADDRESS", "noreply@chatsystems.local"),
            smtp_from_name: env_or("SMTP_FROM_NAME", "Chat Systems"),
            smtp_use_tls: parse_env("SMTP_USE_TLS", false),
            public_url: env_or("PUBLIC_URL", "http://localhost:3000"),
            instance_name: env_or("INSTANCE_NAME", "Chat Systems"),
            instance_icon_url: std::env::var("INSTANCE_ICON_URL").ok(),
            cors_origins: env_or("CORS_ORIGINS", "http://localhost:3001"),
            storage_backend,
            local_storage_path: env_or("LOCAL_STORAGE_PATH", "./data/files"),
            s3_endpoint: env_or("S3_ENDPOINT", "http://localhost:9000"),
            s3_region: env_or("S3_REGION", "us-east-1"),
            s3_bucket: env_or("S3_BUCKET", "chatsystems"),
            s3_access_key: env_or("S3_ACCESS_KEY", "minioadmin"),
            s3_secret_key: env_or("S3_SECRET_KEY", "minioadmin"),
        };
        if config.jwt_secret == "dev-secret-change-me-in-production" || config.jwt_secret.len() < 32
        {
            panic!(
                "JWT_SECRET must be set to a random value of at least 32 characters.\n\
                 Current value is either the insecure default or too short.\n\
                 Generate one with: openssl rand -hex 32"
            );
        }

        if config.public_url.starts_with("http://")
            && !config.public_url.contains("localhost")
            && !config.public_url.contains("127.0.0.1")
        {
            tracing::warn!(
                "PUBLIC_URL is plaintext http:// on a non-localhost host ({}). \
                 Auth cookies will NOT be marked Secure and can leak over the network. \
                 Set PUBLIC_URL to https://your-domain in production.",
                config.public_url
            );
        }

        info!(
            "Config loaded: port={}, storage={:?}",
            config.port, config.storage_backend
        );
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
            unset_saved("SMTP_HOST"),
            set_saved("PORT", "8080"),
            set_saved("ACCESS_TOKEN_EXPIRY", "120"),
            set_saved("SMTP_USE_TLS", "true"),
        ];

        let config = AppConfig::from_env();

        assert_eq!(config.smtp_host, "localhost");
        assert_eq!(config.port, 8080u16);
        assert_eq!(config.access_token_expiry, 120i64);
        assert!(config.smtp_use_tls);
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

        let result = std::panic::catch_unwind(AppConfig::from_env);

        restore(saved);

        assert!(result.is_err(), "weak JWT_SECRET must panic");
    }
}
