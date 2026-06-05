use std::collections::HashSet;

use axum::extract::ws::Message;
use dashmap::DashMap;
use redis::AsyncCommands;
use sqlx::PgPool;
use tokio::sync::mpsc;
use tracing::warn;
use uuid::Uuid;

pub const WRITER_CHANNEL_CAP: usize = 256;

pub const PRESENCE_TTL_SECS: u64 = 60;

pub type WsSender = mpsc::Sender<Message>;

#[derive(Debug)]
pub struct Connection {
    pub user_id: Uuid,
    pub sender: WsSender,
    pub subscribed_workspaces: HashSet<Uuid>,
    pub subscribed_channels: HashSet<Uuid>,
}

pub struct ConnectionManager {
    connections: DashMap<Uuid, Connection>,
    user_connections: DashMap<Uuid, HashSet<Uuid>>,
    db: PgPool,
    redis: redis::aio::ConnectionManager,
    node_id: Uuid,
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
            node_id: Uuid::new_v4(),
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

    pub fn add_connection(&self, conn_id: Uuid, user_id: Uuid, sender: WsSender) -> bool {
        self.connections.insert(
            conn_id,
            Connection {
                user_id,
                sender,
                subscribed_workspaces: HashSet::new(),
                subscribed_channels: HashSet::new(),
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

    fn enqueue(&self, conn_id: &Uuid, message: &str) -> Option<Uuid> {
        let conn = self.connections.get(conn_id)?;
        match conn.sender.try_send(Message::Text(message.to_string())) {
            Ok(()) => None,
            Err(mpsc::error::TrySendError::Full(_)) => {
                warn!(
                    "backpressure: writer channel full, dropping slow connection conn={} user={}",
                    conn_id, conn.user_id
                );
                Some(conn.user_id)
            }
            Err(mpsc::error::TrySendError::Closed(_)) => Some(conn.user_id),
        }
    }

    fn fan_out<F>(&self, message: &str, predicate: F)
    where
        F: Fn(&Connection) -> bool,
    {
        let targets: Vec<Uuid> = self
            .connections
            .iter()
            .filter(|c| predicate(c.value()))
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

    pub async fn broadcast_to_channel(&self, channel_id: Uuid, message: &str) {
        self.fan_out(message, |c| c.subscribed_channels.contains(&channel_id));
    }

    pub async fn broadcast_to_workspace(&self, workspace_id: Uuid, message: &str) {
        self.fan_out(message, |c| c.subscribed_workspaces.contains(&workspace_id));
    }

    pub async fn broadcast_to_all(&self, message: &str) {
        self.fan_out(message, |_| true);
    }

    pub async fn send_to_user(&self, user_id: Uuid, message: &str) {
        let conn_ids: Vec<Uuid> = match self.user_connections.get(&user_id) {
            Some(conns) => conns.iter().copied().collect(),
            None => return,
        };
        for conn_id in conn_ids {
            if let Some(uid) = self.enqueue(&conn_id, message) {
                self.drop_dead_connection(&conn_id, uid);
            }
        }
    }

    pub fn local_users(&self) -> Vec<Uuid> {
        self.user_connections.iter().map(|e| *e.key()).collect()
    }

    fn presence_key(&self, user_id: &Uuid) -> String {
        format!("presence:{}:{}", user_id, self.node_id)
    }

    async fn scan_keys(
        conn: &mut redis::aio::ConnectionManager,
        pattern: &str,
    ) -> redis::RedisResult<Vec<String>> {
        let mut keys = Vec::new();
        let mut cursor: u64 = 0;
        loop {
            let (next, batch): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(pattern)
                .arg("COUNT")
                .arg(100)
                .query_async(conn)
                .await?;
            keys.extend(batch);
            cursor = next;
            if cursor == 0 {
                break;
            }
        }
        Ok(keys)
    }

    pub async fn presence_set_online(&self, user_id: Uuid) {
        let mut conn = self.redis.clone();
        let key = self.presence_key(&user_id);
        let res: redis::RedisResult<()> = conn.set_ex(&key, "online", PRESENCE_TTL_SECS).await;
        if let Err(e) = res {
            warn!("presence_set_online redis error user={}: {}", user_id, e);
        }
    }

    pub async fn presence_refresh(&self, user_id: Uuid) {
        self.presence_set_online(user_id).await;
    }

    pub async fn presence_clear(&self, user_id: Uuid) -> bool {
        let mut conn = self.redis.clone();
        let key = self.presence_key(&user_id);
        let _: redis::RedisResult<()> = conn.del(&key).await;
        match Self::scan_keys(&mut conn, &format!("presence:{}:*", user_id)).await {
            Ok(keys) => keys.is_empty(),
            Err(e) => {
                warn!("presence_clear scan redis error user={}: {}", user_id, e);
                false
            }
        }
    }

    pub async fn get_online_users(&self) -> Vec<Uuid> {
        let mut conn = self.redis.clone();
        let keys = match Self::scan_keys(&mut conn, "presence:*").await {
            Ok(k) => k,
            Err(e) => {
                warn!("get_online_users SCAN redis error: {}", e);
                return Vec::new();
            }
        };
        let mut set = HashSet::new();
        for key in keys {
            if let Some(rest) = key.strip_prefix("presence:") {
                if let Some(uid_str) = rest.split(':').next() {
                    if let Ok(uid) = uid_str.parse::<Uuid>() {
                        set.insert(uid);
                    }
                }
            }
        }
        set.into_iter().collect()
    }

    pub async fn publish_presence(&self, user_id: Uuid, status: &str) {
        self.publish_event(
            "events:presence",
            "presence.changed",
            serde_json::json!({ "user_id": user_id, "status": status }),
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
