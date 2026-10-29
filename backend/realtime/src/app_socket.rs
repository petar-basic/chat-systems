use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::ws::{CloseFrame, Message, WebSocket};
use futures_util::StreamExt;
use sqlx::PgPool;
use tokio::sync::{mpsc, Notify};
use tracing::{info, warn};
use uuid::Uuid;

use crate::connection_manager::WRITER_CHANNEL_CAP;

pub const ACK_WAIT_SECS: f64 = 3.0;
const BATCH: i64 = 100;
const IDLE: Duration = Duration::from_millis(250);
const CHECK_EVERY: Duration = Duration::from_secs(2);
const HEARTBEAT: Duration = Duration::from_secs(20);
const PONG_TIMEOUT: Duration = Duration::from_secs(60);
const INBOUND_PER_SEC: u32 = 200;
const REVOKED_CLOSE_CODE: u16 = 4003;
const FLOOD_CLOSE_CODE: u16 = 4002;

struct Socket {
    id: Uuid,
    tx: mpsc::Sender<Message>,
}

pub struct AppSockets {
    db: PgPool,
    sockets: Mutex<HashMap<Uuid, Vec<Socket>>>,
    next: AtomicUsize,
    wake: Notify,
}

pub struct Admitted {
    pub socket_id: Uuid,
    pub app_id: Uuid,
}

struct Claimed {
    id: Uuid,
    app_id: Uuid,
    kind: String,
    body: serde_json::Value,
    attempts: i32,
}

fn text(value: &serde_json::Value) -> Message {
    Message::Text(value.to_string().into())
}

impl AppSockets {
    pub fn new(db: PgPool) -> Self {
        Self {
            db,
            sockets: Mutex::new(HashMap::new()),
            next: AtomicUsize::new(0),
            wake: Notify::new(),
        }
    }

    pub async fn admit(&self, ticket: &str) -> Option<Admitted> {
        let row = sqlx::query!(
            r"UPDATE app_sockets s
                 SET ticket_hash = NULL, connected_at = NOW(), seen_at = NOW()
                FROM bot_apps b
               WHERE s.ticket_hash = encode(sha256(convert_to($1, 'UTF8')), 'hex')
                 AND s.connected_at IS NULL
                 AND s.opened_at > NOW() - INTERVAL '30 seconds'
                 AND b.id = s.app_id AND b.socket_mode AND b.app_token_hash IS NOT NULL
              RETURNING s.id, s.app_id",
            ticket
        )
        .fetch_optional(&self.db)
        .await
        .inspect_err(|e| warn!("app socket: failed to check a ticket: {e}"))
        .ok()??;
        Some(Admitted {
            socket_id: row.id,
            app_id: row.app_id,
        })
    }

    pub fn add(&self, app_id: Uuid, socket_id: Uuid, tx: mpsc::Sender<Message>) {
        if let Ok(mut sockets) = self.sockets.lock() {
            sockets
                .entry(app_id)
                .or_default()
                .push(Socket { id: socket_id, tx });
        }
        self.wake.notify_one();
    }

    pub async fn remove(&self, app_id: Uuid, socket_id: Uuid) {
        if let Ok(mut sockets) = self.sockets.lock() {
            if let Some(list) = sockets.get_mut(&app_id) {
                list.retain(|s| s.id != socket_id);
                if list.is_empty() {
                    sockets.remove(&app_id);
                }
            }
        }
        if let Err(e) = sqlx::query!("DELETE FROM app_sockets WHERE id = $1", socket_id)
            .execute(&self.db)
            .await
        {
            warn!("app socket: failed to forget socket {socket_id}: {e}");
        }
    }

    fn app_ids(&self) -> Vec<Uuid> {
        self.sockets
            .lock()
            .map(|s| s.keys().copied().collect())
            .unwrap_or_default()
    }

    fn socket_ids(&self) -> Vec<Uuid> {
        self.sockets
            .lock()
            .map(|s| s.values().flatten().map(|socket| socket.id).collect())
            .unwrap_or_default()
    }

    fn send_to_app(&self, app_id: Uuid, frame: Message) -> bool {
        let Ok(sockets) = self.sockets.lock() else {
            return false;
        };
        let Some(list) = sockets.get(&app_id).filter(|l| !l.is_empty()) else {
            return false;
        };
        let start = self.next.fetch_add(1, Ordering::Relaxed);
        (0..list.len()).any(|offset| {
            list[(start + offset) % list.len()]
                .tx
                .try_send(frame.clone())
                .is_ok()
        })
    }

    pub async fn ack(&self, app_id: Uuid, envelope_id: Uuid) {
        let done = sqlx::query!(
            "WITH done AS (
                 UPDATE app_deliveries SET delivered_at = NOW(), leased_until = NULL
                  WHERE id = $1 AND app_id = $2 AND delivered_at IS NULL
                 RETURNING app_id
             )
             UPDATE bot_apps SET failing_since = NULL, last_failure = NULL
              WHERE id IN (SELECT app_id FROM done) AND failing_since IS NOT NULL",
            envelope_id,
            app_id
        )
        .execute(&self.db)
        .await;
        if let Err(e) = done {
            warn!("app socket: failed to record an ack: {e}");
        }
        self.wake.notify_one();
    }

    pub async fn deliver_once(&self) -> usize {
        let apps = self.app_ids();
        if apps.is_empty() {
            return 0;
        }
        let claimed = sqlx::query_as!(
            Claimed,
            r#"WITH heads AS (
                   SELECT DISTINCT ON (d.app_id, d.channel_id)
                          d.id, d.leased_until, d.expires_at
                     FROM app_deliveries d
                     JOIN bot_apps b ON b.id = d.app_id
                    WHERE d.app_id = ANY($1)
                      AND d.delivered_at IS NULL AND d.failed_at IS NULL
                      AND b.socket_mode
                    ORDER BY d.app_id, d.channel_id, d.seq
               ), due AS (
                   SELECT id FROM heads
                    WHERE expires_at > NOW()
                      AND (leased_until IS NULL OR leased_until < NOW())
                    LIMIT $3
               )
               UPDATE app_deliveries d
                  SET leased_until = NOW() + make_interval(secs => $2),
                      attempts = d.attempts + 1
                 FROM due
                WHERE d.id = due.id
                  AND d.delivered_at IS NULL
                  AND (d.leased_until IS NULL OR d.leased_until < NOW())
               RETURNING d.id, d.app_id, d.kind, d.body, d.attempts"#,
            &apps,
            ACK_WAIT_SECS,
            BATCH
        )
        .fetch_all(&self.db)
        .await;
        let claimed = match claimed {
            Ok(claimed) => claimed,
            Err(e) => {
                warn!("app socket: failed to claim deliveries: {e}");
                return 0;
            }
        };
        let count = claimed.len();
        for delivery in claimed {
            let frame = serde_json::json!({
                "envelope_id": delivery.id,
                "type": delivery.kind,
                "retry_attempt": delivery.attempts - 1,
                "payload": delivery.body,
            });
            if !self.send_to_app(delivery.app_id, text(&frame)) {
                metrics::counter!("realtime_app_frames_unsent_total").increment(1);
            }
        }
        count
    }

    pub async fn check_alive(&self) {
        let ids = self.socket_ids();
        if ids.is_empty() {
            return;
        }
        let alive = match sqlx::query_scalar!(
            "UPDATE app_sockets SET seen_at = NOW() WHERE id = ANY($1) RETURNING id",
            &ids
        )
        .fetch_all(&self.db)
        .await
        {
            Ok(alive) => alive,
            Err(e) => {
                warn!("app socket: failed to check sockets: {e}");
                return;
            }
        };
        let Ok(mut sockets) = self.sockets.lock() else {
            return;
        };
        for list in sockets.values_mut() {
            list.retain(|socket| {
                if alive.contains(&socket.id) {
                    return true;
                }
                let _ = socket.tx.try_send(text(&serde_json::json!({
                    "type": "disconnect",
                    "reason": "token_revoked",
                })));
                let _ = socket.tx.try_send(Message::Close(Some(CloseFrame {
                    code: REVOKED_CLOSE_CODE,
                    reason: "token revoked".into(),
                })));
                false
            });
        }
        sockets.retain(|_, list| !list.is_empty());
    }

    pub async fn run(self: Arc<Self>) {
        info!("app socket delivery started");
        let mut checked = Instant::now();
        loop {
            if checked.elapsed() >= CHECK_EVERY {
                self.check_alive().await;
                checked = Instant::now();
            }
            if self.deliver_once().await == 0 {
                tokio::select! {
                    () = self.wake.notified() => {}
                    () = tokio::time::sleep(IDLE) => {}
                }
            }
        }
    }
}

fn envelope_id(text: &str) -> Option<Uuid> {
    serde_json::from_str::<serde_json::Value>(text)
        .ok()?
        .get("envelope_id")?
        .as_str()?
        .parse()
        .ok()
}

pub async fn handle(socket: WebSocket, admitted: Admitted, apps: Arc<AppSockets>) {
    let Admitted { socket_id, app_id } = admitted;
    let (sink, mut receiver) = socket.split();
    let (tx, rx) = mpsc::channel::<Message>(WRITER_CHANNEL_CAP);
    let writer = crate::ws_handler::spawn_writer(sink, rx);

    let _ = tx.try_send(text(&serde_json::json!({
        "type": "hello",
        "app_id": app_id,
        "connection_id": socket_id,
    })));
    apps.add(app_id, socket_id, tx.clone());
    info!("app socket connected: app={app_id} socket={socket_id}");

    let mut heartbeat = tokio::time::interval(HEARTBEAT);
    heartbeat.tick().await;
    let mut last_pong = Instant::now();
    let mut window = Instant::now();
    let mut inbound = 0u32;

    loop {
        tokio::select! {
            message = receiver.next() => match message {
                Some(Ok(Message::Text(body))) => {
                    last_pong = Instant::now();
                    if window.elapsed() >= Duration::from_secs(1) {
                        window = Instant::now();
                        inbound = 0;
                    }
                    inbound += 1;
                    if inbound > INBOUND_PER_SEC {
                        let _ = tx.try_send(Message::Close(Some(CloseFrame {
                            code: FLOOD_CLOSE_CODE,
                            reason: "too many messages".into(),
                        })));
                        break;
                    }
                    if let Some(id) = envelope_id(body.as_str()) {
                        apps.ack(app_id, id).await;
                    }
                }
                Some(Ok(Message::Pong(_) | Message::Ping(_))) => last_pong = Instant::now(),
                Some(Ok(Message::Close(_)) | Err(_)) | None => break,
                Some(Ok(_)) => {}
            },
            _ = heartbeat.tick() => {
                if last_pong.elapsed() > PONG_TIMEOUT {
                    break;
                }
                if tx.try_send(Message::Ping(Vec::new().into())).is_err() {
                    break;
                }
            }
        }
    }

    apps.remove(app_id, socket_id).await;
    drop(tx);
    let _ = writer.await;
    info!("app socket closed: app={app_id} socket={socket_id}");
}
