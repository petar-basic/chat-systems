use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use redis::streams::{
    StreamAutoClaimOptions, StreamAutoClaimReply, StreamId, StreamPendingCountReply,
    StreamReadOptions, StreamReadReply,
};
use redis::AsyncCommands;

use tracing::warn;

use shared_events::Event;

const BLOCK_MS: usize = 2_000;
const BATCH: usize = 100;
const REFRESH_STREAMS_EVERY: Duration = Duration::from_secs(30);
const CLAIM_EVERY: Duration = Duration::from_secs(30);
const CLAIM_MIN_IDLE: Duration = Duration::from_secs(60);
pub const MAX_DELIVERIES: usize = 5;
/// How long past the block a reply is still expected, before the connection is
/// treated as broken.
const RESPONSE_HEADROOM: Duration = Duration::from_secs(5);

static CONSUMER_SEQ: AtomicUsize = AtomicUsize::new(0);

/// The set of workspace streams that exist. Publishers add to it, so the worker
/// can read every stream without scanning the keyspace and without being told
/// which workspaces exist.
pub use shared_events::STREAM_INDEX_KEY;

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
    claimed_at: Option<Instant>,
    claim_min_idle: Duration,
}

fn consumer_name(group: &str) -> String {
    let host = std::env::var("HOSTNAME")
        .ok()
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let seq = CONSUMER_SEQ.fetch_add(1, Ordering::Relaxed);
    format!("{group}-{host}-{}-{seq}", std::process::id())
}

fn parse_delivery(key: &str, entry: StreamId) -> Result<Delivery, String> {
    let body: Option<String> = entry.get("event");
    match body.and_then(|b| serde_json::from_str::<Event>(&b).ok()) {
        Some(event) => Ok(Delivery {
            key: key.to_string(),
            id: entry.id,
            event,
        }),
        None => Err(entry.id),
    }
}

impl StreamGroup {
    pub async fn connect(redis_url: &str, group: &str) -> Option<Self> {
        let client = redis::Client::open(redis_url)
            .inspect_err(|e| warn!("{group} consumer: failed to open Redis: {e}"))
            .ok()?;
        // redis 1.x gives a connection manager a 500ms response timeout by
        // default, which is shorter than the blocking read below. A read that
        // times out on the client still counts as delivered on the server: the
        // entries land in this consumer's pending list and `>` never offers
        // them again, so events go missing without an error anywhere.
        let config = redis::aio::ConnectionManagerConfig::new().set_response_timeout(Some(
            Duration::from_millis(BLOCK_MS as u64) + RESPONSE_HEADROOM,
        ));
        let conn = redis::aio::ConnectionManager::new_with_config(client, config)
            .await
            .inspect_err(|e| warn!("{group} consumer: failed to connect Redis: {e}"))
            .ok()?;

        Some(Self {
            conn,
            group: group.to_string(),
            consumer: consumer_name(group),
            streams: Vec::new(),
            refreshed_at: None,
            claimed_at: None,
            claim_min_idle: CLAIM_MIN_IDLE,
        })
    }

    pub fn claim_min_idle(mut self, min_idle: Duration) -> Self {
        self.claim_min_idle = min_idle;
        self
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

        let fresh: Vec<String> = keys
            .iter()
            .filter(|key| !self.streams.contains(key))
            .cloned()
            .collect();
        self.create_groups(&fresh).await;

        self.streams = keys;
    }

    async fn create_groups(&mut self, keys: &[String]) {
        for key in keys {
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
                }
            }
        }
    }

    async fn claim_abandoned(&mut self) -> Vec<Delivery> {
        let due = self.claimed_at.is_none_or(|at| at.elapsed() >= CLAIM_EVERY);
        if !due {
            return Vec::new();
        }
        self.claimed_at = Some(Instant::now());

        let min_idle_ms = self.claim_min_idle.as_millis() as usize;
        let mut out = Vec::new();
        for key in self.streams.clone() {
            let mut start = String::from("0-0");
            loop {
                let reply: redis::RedisResult<StreamAutoClaimReply> = self
                    .conn
                    .xautoclaim_options(
                        &key,
                        &self.group,
                        &self.consumer,
                        min_idle_ms,
                        &start,
                        StreamAutoClaimOptions::default().count(BATCH),
                    )
                    .await;
                let reply = match reply {
                    Ok(reply) => reply,
                    Err(e) => {
                        warn!("{}: XAUTOCLAIM failed on {}: {}", self.group, key, e);
                        break;
                    }
                };

                let deliveries = self.delivery_counts(&key, &reply.claimed).await;
                for entry in reply.claimed {
                    let times = deliveries.get(&entry.id).copied().unwrap_or(0);
                    if times > MAX_DELIVERIES {
                        warn!(
                            "{}: dropping {} {} after {} deliveries",
                            self.group, key, entry.id, times
                        );
                        self.ack(&key, &entry.id).await;
                        continue;
                    }
                    match parse_delivery(&key, entry) {
                        Ok(delivery) => out.push(delivery),
                        Err(id) => self.ack(&key, &id).await,
                    }
                }

                if reply.next_stream_id == "0-0" {
                    break;
                }
                start = reply.next_stream_id;
            }
        }
        out
    }

    async fn delivery_counts(&mut self, key: &str, claimed: &[StreamId]) -> HashMap<String, usize> {
        let (Some(first), Some(last)) = (claimed.first(), claimed.last()) else {
            return HashMap::new();
        };
        let reply: redis::RedisResult<StreamPendingCountReply> = self
            .conn
            .xpending_consumer_count(
                key,
                &self.group,
                &first.id,
                &last.id,
                claimed.len(),
                &self.consumer,
            )
            .await;
        match reply {
            Ok(reply) => reply
                .ids
                .into_iter()
                .map(|p| (p.id, p.times_delivered))
                .collect(),
            Err(e) => {
                warn!("{}: XPENDING failed on {}: {}", self.group, key, e);
                HashMap::new()
            }
        }
    }

    pub async fn next_batch(&mut self) -> Vec<Delivery> {
        self.refresh_streams().await;
        if self.streams.is_empty() {
            tokio::time::sleep(Duration::from_millis(BLOCK_MS as u64)).await;
            return Vec::new();
        }

        let claimed = self.claim_abandoned().await;
        if !claimed.is_empty() {
            return claimed;
        }

        let options = StreamReadOptions::default()
            .group(&self.group, &self.consumer)
            .count(BATCH)
            .block(BLOCK_MS);
        let cursors = vec![">"; self.streams.len()];

        let response: redis::RedisResult<Option<StreamReadReply>> = self
            .conn
            .xread_options(&self.streams, &cursors, &options)
            .await;

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
        for stream in batches.keys {
            for entry in stream.ids {
                match parse_delivery(&stream.key, entry) {
                    Ok(delivery) => out.push(delivery),
                    // Acknowledge what cannot be parsed, or it comes back for
                    // ever as a pending entry nobody can process.
                    Err(id) => self.ack(&stream.key, &id).await,
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
