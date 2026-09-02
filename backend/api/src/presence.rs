use std::collections::HashSet;

use uuid::Uuid;

/// Everyone the gateway currently holds a socket for in this workspace. One
/// `ZRANGEBYSCORE` over the set the gateway maintains; entries whose heartbeat
/// has lapsed are excluded by score rather than swept.
pub async fn online_user_ids(
    conn: &mut redis::aio::ConnectionManager,
    workspace_id: Uuid,
) -> HashSet<Uuid> {
    let members: redis::RedisResult<Vec<String>> = redis::cmd("ZRANGEBYSCORE")
        .arg(format!("presence:ws:{workspace_id}"))
        .arg(format!("({}", now_secs()))
        .arg("+inf")
        .query_async(conn)
        .await;

    match members {
        Ok(members) => members
            .iter()
            .filter_map(|m| Uuid::parse_str(m).ok())
            .collect(),
        Err(e) => {
            tracing::warn!("presence lookup failed: {}", e);
            HashSet::new()
        }
    }
}

/// Whether the gateway currently holds a socket for this person in this
/// workspace. One `ZSCORE` against the sorted set the gateway maintains; this
/// runs once per push candidate, on the notification path.
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
