//! Per-test databases, cloned from a migrated template.
//!
//! Every integration test wants a database nobody else is touching. Running the
//! migrations to build one costs about 0.4s; cloning a template that already has
//! them costs about 0.1s, and there are hundreds of tests.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::migrate::Migrator;
use sqlx::postgres::PgPoolOptions;
use sqlx::{Connection, PgConnection, PgPool};
use tokio::sync::OnceCell;

/// Namespaced so a run cannot collide with the application's own advisory locks.
const TEMPLATE_LOCK_KEY: i64 = 0x0063_6861_7473_7973;
const TEST_DB_PREFIX: &str = "chatsys_test_";
const TEMPLATE_PREFIX: &str = "chatsys_tmpl_";
/// A database left behind by a killed run. Old enough that no live test can own it.
const STALE_AFTER_MS: u128 = 60 * 60 * 1000;

static TEMPLATE: OnceCell<String> = OnceCell::const_new();
/// Tests start in the same millisecond often enough that the clock alone is not
/// a unique name.
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub struct TestDb {
    name: String,
    pool: PgPool,
}

impl TestDb {
    /// Clones the template for this migration set, building it first if this is
    /// the first test in the process to ask for it.
    pub async fn create(migrations_dir: &str) -> Self {
        let template = TEMPLATE
            .get_or_init(|| async { ensure_template(migrations_dir).await })
            .await;

        let name = format!(
            "{TEST_DB_PREFIX}{}_{}_{}",
            now_ms(),
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        );
        let mut admin = connect_admin().await;
        run(
            &mut admin,
            format!(r#"CREATE DATABASE "{name}" TEMPLATE "{template}""#),
        )
        .await
        .expect("clone the test template");
        admin.close().await.ok();

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url(&name))
            .await
            .expect("connect to the test database");

        Self { name, pool }
    }

    pub fn pool(&self) -> PgPool {
        self.pool.clone()
    }

    /// Dropped explicitly rather than in `Drop`, which cannot await. A test that
    /// panics leaves its database behind; the next run sweeps it up.
    pub async fn cleanup(self) {
        self.pool.close().await;
        let mut admin = connect_admin().await;
        run(
            &mut admin,
            format!(r#"DROP DATABASE IF EXISTS "{}" WITH (FORCE)"#, self.name),
        )
        .await
        .ok();
        admin.close().await.ok();
    }
}

async fn ensure_template(migrations_dir: &str) -> String {
    let migrator = Migrator::new(std::path::Path::new(migrations_dir))
        .await
        .expect("read the migrations directory");
    let name = format!("{TEMPLATE_PREFIX}{:016x}", schema_fingerprint(&migrator));

    let mut admin = connect_admin().await;
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(TEMPLATE_LOCK_KEY)
        .execute(&mut admin)
        .await
        .expect("take the template lock");

    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_database WHERE datname = $1)")
            .bind(&name)
            .fetch_one(&mut admin)
            .await
            .expect("look for the template");

    if !exists {
        run(&mut admin, format!(r#"CREATE DATABASE "{name}""#))
            .await
            .expect("create the template database");

        // The migrations run on a single connection that is closed straight
        // after: Postgres refuses to clone a database anybody is connected to.
        let mut template_conn = PgConnection::connect(&database_url(&name))
            .await
            .expect("connect to the template");
        migrator
            .run(&mut template_conn)
            .await
            .expect("migrate the template");
        template_conn.close().await.ok();
    }

    drop_stale_databases(&mut admin).await;

    sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(TEMPLATE_LOCK_KEY)
        .execute(&mut admin)
        .await
        .ok();
    admin.close().await.ok();

    name
}

/// Databases from a run that was killed before it could clean up. Anything from
/// the last hour might belong to a test running right now, so it is left alone.
async fn drop_stale_databases(admin: &mut PgConnection) {
    let names: Vec<String> = sqlx::query_scalar(
        "SELECT datname FROM pg_database WHERE datname LIKE $1 ORDER BY datname",
    )
    .bind(format!("{TEST_DB_PREFIX}%"))
    .fetch_all(&mut *admin)
    .await
    .unwrap_or_default();

    let cutoff = now_ms().saturating_sub(STALE_AFTER_MS);
    for name in names {
        let stamped = name
            .strip_prefix(TEST_DB_PREFIX)
            .and_then(|rest| rest.split('_').next())
            .and_then(|ms| ms.parse::<u128>().ok());
        if stamped.is_some_and(|created| created < cutoff) {
            run(
                admin,
                format!(r#"DROP DATABASE IF EXISTS "{name}" WITH (FORCE)"#),
            )
            .await
            .ok();
        }
    }
}

/// Changes to any migration — a new one, or an edit to an old one — have to
/// produce a different template, or tests run against yesterday's schema.
fn schema_fingerprint(migrator: &Migrator) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for migration in migrator.iter() {
        for byte in migration
            .version
            .to_le_bytes()
            .iter()
            .chain(migration.checksum.iter())
        {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash
}

/// Postgres has no parameters for identifiers, and sqlx 0.9 wants a dynamic
/// statement marked at the call site. Every name here is generated by this file.
async fn run(conn: &mut PgConnection, sql: String) -> Result<(), sqlx::Error> {
    sqlx::query(sqlx::AssertSqlSafe(sql))
        .execute(conn)
        .await
        .map(|_| ())
}

async fn connect_admin() -> PgConnection {
    PgConnection::connect(&base_url())
        .await
        .expect("connect to the maintenance database")
}

fn base_url() -> String {
    std::env::var("DATABASE_URL").expect("DATABASE_URL must be set to run the integration tests")
}

fn database_url(name: &str) -> String {
    let base = base_url();
    let (prefix, rest) = base.rsplit_once('/').expect("a database url has a path");
    match rest.split_once('?') {
        Some((_, query)) => format!("{prefix}/{name}?{query}"),
        None => format!("{prefix}/{name}"),
    }
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after 1970")
        .as_millis()
}
