use std::time::{Duration, Instant};

use tracing::warn;

use shared_events::Event;

const BLOCK_MS: usize = 2_000;
const BATCH: usize = 100;
const REFRESH_STREAMS_EVERY: Duration = Duration::from_secs(30);

/// The set of workspace streams that exist. Publishers add to it, so the worker
/// can read every stream without scanning the keyspace and without being told
/// which workspaces exist.
pub const STREAM_INDEX_KEY: &str = "stream:index";

/// What `XREADGROUP` returns: per stream key, a list of `(id, flat field pairs)`.
type StreamBatches = Vec<(String, Vec<(String, Vec<String>)>)>;

pub struct Delivery {
    pub key: String,
    pub id: String,
    pub event: Event,
}

/// A consumer-group reader over the workspace streams.
///
/// Consumer groups are what let the worker run more than one replica: each
/// event goes to exactly one consumer in the group, and an unacknowledged event
/// comes back rather than being lost with the process that was holding it. That
/// makes delivery at-least-once, so anything with an outward side effect —
/// sending a notification, calling a webhook — has to be idempotent on its own.
pub struct StreamGroup {
    conn: redis::aio::ConnectionManager,
    group: String,
    consumer: String,
    streams: Vec<String>,
    refreshed_at: Option<Instant>,
}

impl StreamGroup {
    pub async fn connect(redis_url: &str, group: &str) -> Option<Self> {
        let client = redis::Client::open(redis_url)
            .inspect_err(|e| warn!("{group} consumer: failed to open Redis: {e}"))
            .ok()?;
        let conn = redis::aio::ConnectionManager::new(client)
            .await
            .inspect_err(|e| warn!("{group} consumer: failed to connect Redis: {e}"))
            .ok()?;

        Some(Self {
            conn,
            group: group.to_string(),
            consumer: format!("{group}-{}", uuid::Uuid::new_v4()),
            streams: Vec::new(),
            refreshed_at: None,
        })
    }

    async fn refresh_streams(&mut self) {
        // Re-check straight away while nothing is known: a worker that starts
        // before any workspace has been active would otherwise sit idle for a
        // full refresh interval after the first message arrives.
        let due = self.streams.is_empty()
            || self
                .refreshed_at
                .is_none_or(|at| at.elapsed() >= REFRESH_STREAMS_EVERY);
        if !due {
            return;
        }
        self.refreshed_at = Some(Instant::now());

        let keys: redis::RedisResult<Vec<String>> = redis::cmd("SMEMBERS")
            .arg(STREAM_INDEX_KEY)
            .query_async(&mut self.conn)
            .await;
        let keys = match keys {
            Ok(keys) => keys,
            Err(e) => {
                warn!("{}: failed to list streams: {}", self.group, e);
                return;
            }
        };

        for key in &keys {
            if self.streams.contains(key) {
                continue;
            }
            // From the beginning of the stream, not from its tail. A group is
            // only created the first time it meets a stream, and creating it at
            // the tail would silently skip everything published between the
            // stream appearing and the worker noticing it.
            let created: redis::RedisResult<String> = redis::cmd("XGROUP")
                .arg("CREATE")
                .arg(key)
                .arg(&self.group)
                .arg("0")
                .arg("MKSTREAM")
                .query_async(&mut self.conn)
                .await;
            if let Err(e) = created {
                if !e.to_string().contains("BUSYGROUP") {
                    warn!("{}: failed to create group on {}: {}", self.group, key, e);
                    continue;
                }
            }
        }

        self.streams = keys;
    }

    pub async fn next_batch(&mut self) -> Vec<Delivery> {
        self.refresh_streams().await;
        if self.streams.is_empty() {
            tokio::time::sleep(Duration::from_millis(BLOCK_MS as u64)).await;
            return Vec::new();
        }

        let mut cmd = redis::cmd("XREADGROUP");
        cmd.arg("GROUP")
            .arg(&self.group)
            .arg(&self.consumer)
            .arg("COUNT")
            .arg(BATCH)
            .arg("BLOCK")
            .arg(BLOCK_MS)
            .arg("STREAMS");
        for key in &self.streams {
            cmd.arg(key);
        }
        for _ in &self.streams {
            cmd.arg(">");
        }

        let response: redis::RedisResult<Option<StreamBatches>> =
            cmd.query_async(&mut self.conn).await;

        let batches = match response {
            Ok(Some(batches)) => batches,
            Ok(None) => return Vec::new(),
            Err(e) => {
                // A stream can be trimmed out of existence between the index
                // refresh and the read; drop the cache and try again next tick.
                warn!("{}: XREADGROUP failed: {}", self.group, e);
                self.refreshed_at = None;
                tokio::time::sleep(Duration::from_millis(500)).await;
                return Vec::new();
            }
        };

        let mut out = Vec::new();
        for (key, entries) in batches {
            for (id, fields) in entries {
                let body = fields
                    .chunks(2)
                    .find(|pair| pair.first().map(String::as_str) == Some("event"))
                    .and_then(|pair| pair.get(1));
                match body.and_then(|b| serde_json::from_str::<Event>(b).ok()) {
                    Some(event) => out.push(Delivery {
                        key: key.clone(),
                        id,
                        event,
                    }),
                    // Acknowledge what cannot be parsed, or it comes back for
                    // ever as a pending entry nobody can process.
                    None => self.ack(&key, &id).await,
                }
            }
        }
        out
    }

    pub async fn ack(&mut self, key: &str, id: &str) {
        let acked: redis::RedisResult<i64> = redis::cmd("XACK")
            .arg(key)
            .arg(&self.group)
            .arg(id)
            .query_async(&mut self.conn)
            .await;
        if let Err(e) = acked {
            warn!("{}: XACK failed for {} {}: {}", self.group, key, id, e);
        }
    }

    pub fn connection(&self) -> redis::aio::ConnectionManager {
        self.conn.clone()
    }
}
