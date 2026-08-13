use std::sync::Arc;
use std::time::Duration;

use tracing::{info, warn};

use crate::messaging::stream_group::STREAM_INDEX_KEY;
use crate::state::AppState;

const INTERVAL: Duration = Duration::from_secs(15 * 60);
const MAX_AGE: Duration = Duration::from_secs(60 * 60);

/// `XADD ... MAXLEN` already bounds each stream by length. This bounds it by
/// age, which is the limit that actually matters: a quiet workspace would
/// otherwise keep a week of events under the length cap and hand a reconnecting
/// client a replay of things it stopped caring about long ago.
///
/// It also drops streams for workspaces that have gone silent, so the index the
/// worker reads does not grow for ever.
pub async fn start_stream_trimmer(state: Arc<AppState>) {
    info!("Stream trimmer started");

    loop {
        tokio::time::sleep(INTERVAL).await;

        let mut conn = state.redis.clone();
        let keys: Vec<String> = match redis::cmd("SMEMBERS")
            .arg(STREAM_INDEX_KEY)
            .query_async(&mut conn)
            .await
        {
            Ok(keys) => keys,
            Err(e) => {
                warn!("Stream trimmer: failed to list streams: {}", e);
                continue;
            }
        };

        let cutoff = chrono::Utc::now().timestamp_millis() - MAX_AGE.as_millis() as i64;

        for key in keys {
            let trimmed: redis::RedisResult<i64> = redis::cmd("XTRIM")
                .arg(&key)
                .arg("MINID")
                .arg("~")
                .arg(cutoff)
                .query_async(&mut conn)
                .await;
            if let Err(e) = trimmed {
                warn!("Stream trimmer: XTRIM failed for {}: {}", key, e);
                continue;
            }

            let remaining: redis::RedisResult<i64> =
                redis::cmd("XLEN").arg(&key).query_async(&mut conn).await;
            if matches!(remaining, Ok(0)) {
                let _: redis::RedisResult<()> = redis::cmd("SREM")
                    .arg(STREAM_INDEX_KEY)
                    .arg(&key)
                    .query_async(&mut conn)
                    .await;
            }
        }
    }
}
