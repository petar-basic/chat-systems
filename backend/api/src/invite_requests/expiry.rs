use std::sync::Arc;
use std::time::Duration;

use tracing::{info, warn};

use crate::state::AppState;

const CHECK_EVERY: Duration = Duration::from_secs(600);
const EXPIRE_PER_RUN: i64 = 200;

pub async fn start(state: Arc<AppState>) {
    info!("Invite request expiry started");
    loop {
        tokio::time::sleep(CHECK_EVERY).await;
        run(&state).await;
    }
}

pub async fn run(state: &AppState) {
    match super::service::expire_due(state, EXPIRE_PER_RUN).await {
        Ok(0) => {}
        Ok(expired) => info!(expired, "Expired invite requests nobody answered"),
        Err(e) => warn!("could not expire invite requests: {e}"),
    }
}
