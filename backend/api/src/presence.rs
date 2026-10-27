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

pub async fn is_active(conn: &mut redis::aio::ConnectionManager, user_id: Uuid) -> bool {
    use redis::AsyncCommands;

    let expiries: redis::RedisResult<Vec<i64>> = conn.hvals(format!("active:user:{user_id}")).await;
    match expiries {
        Ok(expiries) => {
            let now = now_secs();
            expiries.into_iter().any(|expires_at| expires_at > now)
        }
        Err(e) => {
            tracing::warn!("activity lookup failed: {}", e);
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
