use std::sync::Arc;
use std::time::Duration;

use tracing::{info, warn};

use crate::state::AppState;

const INTERVAL: Duration = Duration::from_secs(60 * 60 * 6);
const WINDOW_HOURS: i64 = 24;

/// A denormalised counter drifts: a transaction that half-committed, a manual
/// database fix, a restore from backup. Recomputing the recent ones on a timer
/// turns "my badge is wrong and nobody knows why" from a bug report into a log
/// line with a number attached.
pub async fn start_unread_reconciler(state: Arc<AppState>) {
    info!("Unread reconciler started");

    loop {
        tokio::time::sleep(INTERVAL).await;

        match state
            .message_repo
            .reconcile_unread_counts(WINDOW_HOURS)
            .await
        {
            Ok(0) => info!("Unread reconciler: no drift"),
            Ok(corrected) => warn!(
                corrected,
                "Unread reconciler corrected drifted unread counters"
            ),
            Err(e) => warn!("Unread reconciler failed: {}", e),
        }
    }
}
