use std::collections::HashSet;

use axum::extract::ws::{CloseFrame, Message};
use dashmap::DashMap;
use redis::AsyncCommands;
use sqlx::PgPool;
use tokio::sync::mpsc;
use tracing::warn;
use uuid::Uuid;

pub const WRITER_CHANNEL_CAP: usize = 256;

pub const PRESENCE_TTL_SECS: u64 = 60;

/// Distinct from the revocation code so the client can tell "you are too slow"
/// from "you may not connect" and reconnect immediately in the first case.
pub const BACKPRESSURE_CLOSE_CODE: u16 = 4003;

pub const HUDDLE_TTL_SECS: i64 = 120;

pub const SESSION_REVOKED_CLOSE_CODE: u16 = 4001;

/// Mirrors the API's `RevocationRecord`: a revocation invalidates every token
/// issued before it, minus one optionally spared session.
#[derive(Debug, serde::Deserialize)]
pub struct RevocationRecord {
    pub at: i64,
    #[serde(default)]
    pub except_jti: Option<Uuid>,
}

impl RevocationRecord {
    pub fn covers(&self, claims: &crate::Claims) -> bool {
        if claims.iat > self.at {
            return false;
        }
        !matches!((self.except_jti, claims.jti), (Some(except), Some(jti)) if except == jti)
    }
}

pub type WsSender = mpsc::Sender<Message>;

#[derive(Debug)]
pub struct Connection {
    pub user_id: Uuid,
    pub token_jti: Option<Uuid>,
    pub sender: WsSender,
    pub subscribed_workspaces: HashSet<Uuid>,
    pub subscribed_channels: HashSet<Uuid>,
    pub subscribed_huddles: HashSet<Uuid>,
}

/// Who a delivery is for. `Everyone` is the live path; `Connection` is a replay
/// being handed to the one client that asked for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Audience {
    Everyone,
    Connection(Uuid),
}

impl Audience {
    fn includes(self, conn_id: &Uuid) -> bool {
        match self {
            Self::Everyone => true,
            Self::Connection(only) => only == *conn_id,
        }
    }
}

pub struct ConnectionManager {
    connections: DashMap<Uuid, Connection>,
    user_connections: DashMap<Uuid, HashSet<Uuid>>,
    db: PgPool,
    redis: redis::aio::ConnectionManager,
}

impl std::fmt::Debug for ConnectionManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectionManager")
            .field("connections", &self.connections.len())
            .field("user_connections", &self.user_connections.len())
            .finish()
    }
}

impl ConnectionManager {
    pub fn new(db: PgPool, redis: redis::aio::ConnectionManager) -> Self {
        Self {
            connections: DashMap::new(),
            user_connections: DashMap::new(),
            db,
            redis,
        }
    }

    pub fn db(&self) -> &PgPool {
        &self.db
    }

    pub fn redis(&self) -> redis::aio::ConnectionManager {
        self.redis.clone()
    }

    pub async fn is_channel_member(&self, channel_id: Uuid, user_id: Uuid) -> bool {
        let result = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM channel_members WHERE channel_id = $1 AND user_id = $2)",
        )
        .bind(channel_id)
        .bind(user_id)
        .fetch_one(&self.db)
        .await;

        match result {
            Ok(is_member) => is_member,
            Err(e) => {
                warn!(
                    "is_channel_member DB error (denying) channel={} user={}: {}",
                    channel_id, user_id, e
                );
                false
            }
        }
    }

    pub async fn is_workspace_member(&self, workspace_id: Uuid, user_id: Uuid) -> bool {
        let result = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM workspace_members WHERE workspace_id = $1 AND user_id = $2)",
        )
        .bind(workspace_id)
        .bind(user_id)
        .fetch_one(&self.db)
        .await;

        match result {
            Ok(is_member) => is_member,
            Err(e) => {
                warn!(
                    "is_workspace_member DB error (denying) workspace={} user={}: {}",
                    workspace_id, user_id, e
                );
                false
            }
        }
    }

    pub fn add_connection(
        &self,
        conn_id: Uuid,
        user_id: Uuid,
        token_jti: Option<Uuid>,
        sender: WsSender,
    ) -> bool {
        self.connections.insert(
            conn_id,
            Connection {
                user_id,
                token_jti,
                sender,
                subscribed_workspaces: HashSet::new(),
                subscribed_channels: HashSet::new(),
                subscribed_huddles: HashSet::new(),
            },
        );
        let mut entry = self.user_connections.entry(user_id).or_default();
        let was_empty = entry.is_empty();
        entry.insert(conn_id);
        was_empty
    }

    pub fn remove_connection(&self, conn_id: &Uuid) -> Option<(Uuid, bool)> {
        if let Some((_, conn)) = self.connections.remove(conn_id) {
            let mut was_last = false;
            if let Some(mut conns) = self.user_connections.get_mut(&conn.user_id) {
                conns.remove(conn_id);
                if conns.is_empty() {
                    drop(conns);
                    self.user_connections.remove(&conn.user_id);
                    was_last = true;
                }
            }
            Some((conn.user_id, was_last))
        } else {
            None
        }
    }

    pub fn subscribe_workspace(&self, conn_id: &Uuid, workspace_id: Uuid) {
        if let Some(mut conn) = self.connections.get_mut(conn_id) {
            conn.subscribed_workspaces.insert(workspace_id);
        }
    }

    pub fn join_channel(&self, conn_id: &Uuid, channel_id: Uuid) {
        if let Some(mut conn) = self.connections.get_mut(conn_id) {
            conn.subscribed_channels.insert(channel_id);
        }
    }

    pub fn leave_channel(&self, conn_id: &Uuid, channel_id: Uuid) {
        if let Some(mut conn) = self.connections.get_mut(conn_id) {
            conn.subscribed_channels.remove(&channel_id);
        }
    }

    pub fn join_huddle(&self, conn_id: &Uuid, huddle_id: Uuid) {
        if let Some(mut conn) = self.connections.get_mut(conn_id) {
            conn.subscribed_huddles.insert(huddle_id);
        }
    }

    /// Membership is checked when a connection subscribes, so a socket that is
    /// already subscribed keeps receiving until it closes. Removal has to reach
    /// in and drop the subscription.
    pub fn leave_channel_for_user(&self, user_id: Uuid, channel_id: Uuid) {
        let conn_ids: Vec<Uuid> = match self.user_connections.get(&user_id) {
            Some(conns) => conns.iter().copied().collect(),
            None => return,
        };
        for conn_id in conn_ids {
            if let Some(mut conn) = self.connections.get_mut(&conn_id) {
                conn.subscribed_channels.remove(&channel_id);
            }
        }
    }

    pub fn leave_workspace_for_user(&self, user_id: Uuid, workspace_id: Uuid) {
        let conn_ids: Vec<Uuid> = match self.user_connections.get(&user_id) {
            Some(conns) => conns.iter().copied().collect(),
            None => return,
        };
        for conn_id in conn_ids {
            if let Some(mut conn) = self.connections.get_mut(&conn_id) {
                conn.subscribed_workspaces.remove(&workspace_id);
            }
        }
    }

    pub fn leave_huddle(&self, conn_id: &Uuid, huddle_id: Uuid) {
        if let Some(mut conn) = self.connections.get_mut(conn_id) {
            conn.subscribed_huddles.remove(&huddle_id);
        }
    }

    pub fn huddle_ids_for_conn(&self, conn_id: &Uuid) -> Vec<Uuid> {
        self.connections
            .get(conn_id)
            .map(|c| c.subscribed_huddles.iter().copied().collect())
            .unwrap_or_default()
    }

    pub fn user_in_huddle_local(&self, user_id: Uuid, huddle_id: Uuid) -> bool {
        self.connections
            .iter()
            .any(|c| c.user_id == user_id && c.subscribed_huddles.contains(&huddle_id))
    }

    fn enqueue(&self, conn_id: &Uuid, message: &str) -> Option<Uuid> {
        let conn = self.connections.get(conn_id)?;
        match conn.sender.try_send(Message::Text(message.to_string())) {
            Ok(()) => None,
            Err(mpsc::error::TrySendError::Full(_)) => {
                warn!(
                    "backpressure: writer channel full, dropping slow connection conn={} user={}",
                    conn_id, conn.user_id
                );
                metrics::counter!("realtime_backpressure_drops_total").increment(1);
                // Tell it why. Dropped silently, the client waits up to a
                // heartbeat to notice and then refetches everything; told to go
                // away, it reconnects at once and replays from its last id.
                let _ = conn.sender.try_send(Message::Close(Some(CloseFrame {
                    code: BACKPRESSURE_CLOSE_CODE,
                    reason: "too far behind".into(),
                })));
                Some(conn.user_id)
            }
            Err(mpsc::error::TrySendError::Closed(_)) => Some(conn.user_id),
        }
    }

    fn fan_out<F>(&self, message: &str, predicate: F)
    where
        F: Fn(&Connection) -> bool,
    {
        self.fan_out_to(Audience::Everyone, message, predicate);
    }

    /// Replay reuses this, restricted to one connection, so a replayed event
    /// goes through the *same* visibility predicate as the live one. Giving
    /// replay its own routing is where a naive implementation leaks a private
    /// channel into somebody's backlog.
    fn fan_out_to<F>(&self, audience: Audience, message: &str, predicate: F)
    where
        F: Fn(&Connection) -> bool,
    {
        let targets: Vec<Uuid> = self
            .connections
            .iter()
            .filter(|c| audience.includes(c.key()) && predicate(c.value()))
            .map(|c| *c.key())
            .collect();

        for conn_id in targets {
            if let Some(user_id) = self.enqueue(&conn_id, message) {
                self.drop_dead_connection(&conn_id, user_id);
            }
        }
    }

    fn drop_dead_connection(&self, conn_id: &Uuid, _user_id: Uuid) {
        self.remove_connection(conn_id);
    }

    pub async fn broadcast_to_channel(&self, audience: Audience, channel_id: Uuid, message: &str) {
        self.fan_out_to(audience, message, |c| {
            c.subscribed_channels.contains(&channel_id)
        });
    }

    pub async fn broadcast_to_workspace(
        &self,
        audience: Audience,
        workspace_id: Uuid,
        message: &str,
    ) {
        self.fan_out_to(audience, message, |c| {
            c.subscribed_workspaces.contains(&workspace_id)
        });
    }

    pub async fn broadcast_to_huddle(&self, audience: Audience, huddle_id: Uuid, message: &str) {
        self.fan_out_to(audience, message, |c| {
            c.subscribed_huddles.contains(&huddle_id)
        });
    }

    pub async fn broadcast_to_all(&self, audience: Audience, message: &str) {
        self.fan_out_to(audience, message, |_| true);
    }

    pub async fn send_to_user(&self, audience: Audience, user_id: Uuid, message: &str) {
        let conn_ids: Vec<Uuid> = match self.user_connections.get(&user_id) {
            Some(conns) => conns.iter().copied().collect(),
            None => return,
        };
        for conn_id in conn_ids.into_iter().filter(|id| audience.includes(id)) {
            if let Some(uid) = self.enqueue(&conn_id, message) {
                self.drop_dead_connection(&conn_id, uid);
            }
        }
    }

    /// One frame to one socket, without going through a visibility predicate:
    /// used for answers to that client's own request, never for fan-out.
    pub fn send_to_conn(&self, conn_id: Uuid, message: &str) {
        if let Some(user_id) = self.enqueue(&conn_id, message) {
            self.drop_dead_connection(&conn_id, user_id);
        }
    }

    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    pub fn disconnect_user(&self, user_id: Uuid, reason: &str) {
        self.disconnect_user_except(user_id, None, reason);
    }

    /// A revoked session must not linger until its token deadline. The close
    /// frame carries a code the client uses to stop reconnecting instead of
    /// looping against a socket it can no longer open.
    pub fn disconnect_user_except(&self, user_id: Uuid, except_jti: Option<Uuid>, reason: &str) {
        let conn_ids: Vec<Uuid> = match self.user_connections.get(&user_id) {
            Some(conns) => conns.iter().copied().collect(),
            None => return,
        };
        for conn_id in conn_ids {
            if let Some(conn) = self.connections.get(&conn_id) {
                if except_jti.is_some() && conn.token_jti == except_jti {
                    continue;
                }
                let frame = CloseFrame {
                    code: SESSION_REVOKED_CLOSE_CODE,
                    reason: reason.to_string().into(),
                };
                let _ = conn.sender.try_send(Message::Close(Some(frame)));
            }
        }
    }

    pub async fn is_revoked(&self, claims: &crate::Claims) -> bool {
        let mut conn = self.redis.clone();
        let raw: redis::RedisResult<Option<String>> =
            conn.get(format!("revoked:{}", claims.sub)).await;
        let payload = match raw {
            Ok(Some(payload)) => payload,
            Ok(None) => return false,
            Err(e) => {
                warn!("revocation lookup failed for user {}: {}", claims.sub, e);
                return false;
            }
        };
        let record: RevocationRecord = match serde_json::from_str(&payload) {
            Ok(record) => record,
            Err(_) => return true,
        };
        record.covers(claims)
    }

    fn presence_key(workspace_id: &Uuid) -> String {
        format!("presence:ws:{workspace_id}")
    }

    fn now_secs() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }

    /// One sorted set per workspace, scored by the moment the entry goes stale.
    /// Reading presence used to mean scanning the whole Redis keyspace — which
    /// also holds rate-limit keys, revocation flags and huddle membership, so the
    /// cost grew with traffic that has nothing to do with presence, on a path
    /// that runs for every `subscribe` frame.
    ///
    /// A node that dies leaves entries behind; they expire by score and are
    /// trimmed on the next read, so there is no sweeper to run and nothing stays
    /// online forever.
    pub async fn presence_set_online(&self, user_id: Uuid, workspace_ids: &[Uuid]) {
        if workspace_ids.is_empty() {
            return;
        }
        let mut conn = self.redis.clone();
        let expires_at = Self::now_secs() + PRESENCE_TTL_SECS as i64;

        let mut pipe = redis::pipe();
        for workspace_id in workspace_ids {
            pipe.zadd(
                Self::presence_key(workspace_id),
                user_id.to_string(),
                expires_at,
            )
            .ignore();
        }
        if let Err(e) = pipe.query_async::<()>(&mut conn).await {
            warn!("presence_set_online redis error user={}: {}", user_id, e);
        }
    }

    pub async fn presence_refresh(&self, user_id: Uuid, workspace_ids: &[Uuid]) {
        self.presence_set_online(user_id, workspace_ids).await;
    }

    /// Returns whether the user is now offline everywhere. With a sorted set the
    /// last writer wins and the score is the latest heartbeat, so a user with a
    /// live connection on another node keeps a future score and stays online —
    /// which is the behaviour the per-node keys were approximating.
    pub async fn presence_clear(&self, user_id: Uuid, workspace_ids: &[Uuid]) -> bool {
        if workspace_ids.is_empty() {
            return true;
        }
        let mut conn = self.redis.clone();
        let mut pipe = redis::pipe();
        for workspace_id in workspace_ids {
            pipe.zrem(Self::presence_key(workspace_id), user_id.to_string())
                .ignore();
        }
        if let Err(e) = pipe.query_async::<()>(&mut conn).await {
            warn!("presence_clear redis error user={}: {}", user_id, e);
            return false;
        }
        true
    }

    pub async fn online_users_in_workspace(&self, workspace_id: Uuid) -> Vec<Uuid> {
        let started = std::time::Instant::now();
        let mut conn = self.redis.clone();
        let key = Self::presence_key(&workspace_id);
        let now = Self::now_secs();

        let mut pipe = redis::pipe();
        pipe.cmd("ZREMRANGEBYSCORE")
            .arg(&key)
            .arg("-inf")
            .arg(now)
            .ignore();
        pipe.cmd("ZRANGEBYSCORE").arg(&key).arg(now).arg("+inf");

        let result: redis::RedisResult<(Vec<String>,)> = pipe.query_async(&mut conn).await;
        let users = match result {
            Ok((raw,)) => raw
                .into_iter()
                .filter_map(|id| id.parse::<Uuid>().ok())
                .collect(),
            Err(e) => {
                warn!(
                    "online_users_in_workspace redis error workspace={}: {}",
                    workspace_id, e
                );
                Vec::new()
            }
        };

        metrics::histogram!("realtime_presence_query_duration_seconds")
            .record(started.elapsed().as_secs_f64());
        users
    }

    /// Losing membership should take the user out of that workspace's presence
    /// immediately rather than at the end of the TTL, when anyone still looking
    /// would see a ghost.
    pub async fn presence_leave_workspace(&self, user_id: Uuid, workspace_id: Uuid) {
        let mut conn = self.redis.clone();
        let res: redis::RedisResult<()> = conn
            .zrem(Self::presence_key(&workspace_id), user_id.to_string())
            .await;
        if let Err(e) = res {
            warn!(
                "presence_leave_workspace redis error user={} workspace={}: {}",
                user_id, workspace_id, e
            );
        }
    }

    pub async fn user_workspace_ids(&self, user_id: Uuid) -> Vec<Uuid> {
        let result = sqlx::query_scalar::<_, Uuid>(
            "SELECT workspace_id FROM workspace_members WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_all(&self.db)
        .await;
        match result {
            Ok(ids) => ids,
            Err(e) => {
                warn!("user_workspace_ids DB error user={}: {}", user_id, e);
                Vec::new()
            }
        }
    }

    pub fn send_to_workspace_members(&self, exclude: Uuid, workspace_ids: &[Uuid], message: &str) {
        self.fan_out(message, |c| {
            c.user_id != exclude
                && c.subscribed_workspaces
                    .iter()
                    .any(|w| workspace_ids.contains(w))
        });
    }

    pub async fn publish_presence(&self, user_id: Uuid, status: &str) {
        let workspace_ids = self.user_workspace_ids(user_id).await;
        self.publish_event(
            "events:presence",
            "presence.changed",
            serde_json::json!({
                "user_id": user_id,
                "status": status,
                "workspace_ids": workspace_ids,
            }),
        )
        .await;
    }

    pub async fn publish_typing(&self, channel_id: Uuid, user_id: Uuid, is_typing: bool) {
        self.publish_event(
            "events:typing",
            "typing.indicator",
            serde_json::json!({
                "channel_id": channel_id,
                "user_id": user_id,
                "is_typing": is_typing,
            }),
        )
        .await;
    }

    pub async fn publish_huddle(&self, event_type: &str, payload: serde_json::Value) {
        let channel = match event_type {
            "huddle.member_joined" | "huddle.member_left" => "events:huddle",
            _ => "events:huddle-signal",
        };
        self.publish_event(channel, event_type, payload).await;
    }

    fn huddle_members_key(huddle_id: &Uuid) -> String {
        format!("huddle:{huddle_id}:members")
    }

    pub async fn huddle_redis_join(&self, huddle_id: Uuid, user_id: Uuid) {
        let mut conn = self.redis.clone();
        let key = Self::huddle_members_key(&huddle_id);
        let _: redis::RedisResult<()> = conn.sadd(&key, user_id.to_string()).await;
        let _: redis::RedisResult<()> = conn.expire(&key, HUDDLE_TTL_SECS).await;
    }

    pub async fn huddle_redis_leave(&self, huddle_id: Uuid, user_id: Uuid) {
        let mut conn = self.redis.clone();
        let key = Self::huddle_members_key(&huddle_id);
        let _: redis::RedisResult<()> = conn.srem(&key, user_id.to_string()).await;
    }

    pub async fn huddle_redis_is_member(&self, huddle_id: Uuid, user_id: Uuid) -> bool {
        let mut conn = self.redis.clone();
        let key = Self::huddle_members_key(&huddle_id);
        let res: redis::RedisResult<bool> = conn.sismember(&key, user_id.to_string()).await;
        res.unwrap_or(false)
    }

    pub async fn huddle_redis_members(&self, huddle_id: Uuid) -> Vec<Uuid> {
        let mut conn = self.redis.clone();
        let key = Self::huddle_members_key(&huddle_id);
        let res: redis::RedisResult<Vec<String>> = conn.smembers(&key).await;
        match res {
            Ok(v) => v.into_iter().filter_map(|s| s.parse().ok()).collect(),
            Err(e) => {
                warn!(
                    "huddle_redis_members redis error huddle={}: {}",
                    huddle_id, e
                );
                Vec::new()
            }
        }
    }

    pub async fn huddle_redis_refresh_conn(&self, conn_id: &Uuid, user_id: Uuid) {
        for huddle_id in self.huddle_ids_for_conn(conn_id) {
            self.huddle_redis_join(huddle_id, user_id).await;
        }
    }

    async fn publish_event(
        &self,
        redis_channel: &str,
        event_type: &str,
        payload: serde_json::Value,
    ) {
        let envelope = serde_json::json!({
            "event_type": event_type,
            "payload": payload,
        });
        let json = envelope.to_string();
        let mut conn = self.redis.clone();
        let res: redis::RedisResult<()> = conn.publish(redis_channel, json).await;
        if let Err(e) = res {
            warn!("publish to {} failed: {}", redis_channel, e);
        }
    }
}
