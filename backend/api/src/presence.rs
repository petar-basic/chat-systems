use std::collections::HashSet;

use uuid::Uuid;

pub async fn online_user_ids(conn: &mut redis::aio::ConnectionManager) -> HashSet<Uuid> {
    let mut online = HashSet::new();
    let mut cursor: u64 = 0;

    loop {
        let scan: redis::RedisResult<(u64, Vec<String>)> = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg("presence:*")
            .arg("COUNT")
            .arg(100)
            .query_async(conn)
            .await;

        let (next, keys) = match scan {
            Ok(page) => page,
            Err(e) => {
                tracing::warn!("presence scan failed: {}", e);
                return online;
            }
        };

        for key in keys {
            if let Some(rest) = key.strip_prefix("presence:") {
                if let Some(Ok(uid)) = rest.split(':').next().map(Uuid::parse_str) {
                    online.insert(uid);
                }
            }
        }

        cursor = next;
        if cursor == 0 {
            return online;
        }
    }
}

/// Whether the gateway currently holds a socket for this person in this
/// workspace. One `ZSCORE` against the sorted set the gateway maintains, rather
/// than the keyspace scan `online_user_ids` does -- this runs once per push
/// candidate, on the notification path.
///
/// Entries expire by score, so an entry from a node that died reads as stale
/// rather than as somebody who is still connected.
pub async fn is_online(
    conn: &mut redis::aio::ConnectionManager,
    workspace_id: Uuid,
    user_id: Uuid,
) -> bool {
    use redis::AsyncCommands;

    let key = format!("presence:ws:{workspace_id}");
    let score: redis::RedisResult<Option<i64>> = conn.zscore(&key, user_id.to_string()).await;

    match score {
        Ok(Some(expires_at)) => expires_at > now_secs(),
        Ok(None) => false,
        Err(e) => {
            // Failing open here means sending a push to somebody who is looking
            // at the message. Failing closed means silence when they are not.
            tracing::warn!("presence lookup failed: {}", e);
            false
        }
    }
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
