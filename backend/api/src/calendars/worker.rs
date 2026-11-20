use std::sync::Arc;
use std::time::Duration;

use tracing::{info, warn};

use crate::state::AppState;

const TICK_EVERY: Duration = Duration::from_secs(15);

pub async fn start(state: Arc<AppState>) {
    info!("Calendar worker started");
    loop {
        match super::service::tick(&state, chrono::Utc::now()).await {
            Ok(tick) if tick != super::service::Tick::default() => {
                info!(
                    synced = tick.synced,
                    statuses = tick.statuses,
                    presence = tick.presence,
                    reminders = tick.reminders,
                    agendas = tick.agendas,
                    "calendars: tick"
                );
            }
            Ok(_) => {}
            Err(e) => warn!("calendars: tick failed: {e}"),
        }
        tokio::time::sleep(TICK_EVERY).await;
    }
}
