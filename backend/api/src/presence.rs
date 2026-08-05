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
