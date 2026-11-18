pub mod models;
pub mod repo;
pub mod routes;

use shared_common::errors::AppResult;

use crate::authz::ChannelAccess;
use crate::state::AppState;
use models::AppShortcut;

pub async fn in_channel(state: &AppState, access: &ChannelAccess) -> AppResult<Vec<AppShortcut>> {
    if access.is_guest() {
        return Ok(Vec::new());
    }
    Ok(state
        .app_shortcut_repo
        .message_in_channel(access.channel.id)
        .await?
        .into_iter()
        .map(AppShortcut::from)
        .collect())
}
