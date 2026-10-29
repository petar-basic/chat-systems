use uuid::Uuid;

use shared_common::errors::AppResult;

use crate::state::AppState;

pub async fn set_away(state: &AppState, user_id: Uuid, away: bool) -> AppResult<()> {
    state.profile_repo.set_away(user_id, away).await?;
    announce(state, user_id).await
}

pub async fn toggle_away(state: &AppState, user_id: Uuid) -> AppResult<bool> {
    let away = state.profile_repo.toggle_away(user_id).await?;
    announce(state, user_id).await?;
    Ok(away)
}

async fn announce(state: &AppState, user_id: Uuid) -> AppResult<()> {
    state
        .publisher
        .publish(
            "presence.recompute",
            serde_json::json!({ "user_id": user_id }),
        )
        .await
}
