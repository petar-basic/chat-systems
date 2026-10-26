use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::ChannelIntegrations;
use crate::authz;
use crate::hooks::models::HookType;
use crate::middleware::AuthUser;
use crate::state::AppState;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new().routes(routes!(channel_integrations))
}

#[utoipa::path(get, path = "/channels/{ch_id}/integrations", tag = "channels",
    responses((status = 200, body = ChannelIntegrations)))]
async fn channel_integrations(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ch_id): Path<Uuid>,
) -> AppResult<Json<ChannelIntegrations>> {
    let access = authz::require_channel_access(&state, ch_id, auth.user_id).await?;
    let ws_id = access.channel.workspace_id;
    let (incoming_webhooks, outgoing_webhooks, slash_commands, workflows) = tokio::try_join!(
        async { Ok::<_, AppError>(state.hook_repo.incoming_for_channel(ws_id, ch_id).await?) },
        async { Ok(state.hook_repo.outgoing_for_channel(ws_id, ch_id).await?) },
        async { Ok(state.hook_repo.commands_for_channel(ws_id, ch_id).await?) },
        async { Ok(state.workflow_repo.touching_channel(ws_id, ch_id).await?) },
    )?;
    Ok(Json(ChannelIntegrations {
        incoming_webhooks,
        outgoing_webhooks,
        slash_commands,
        workflows,
        can_add_webhook: access.can_manage_hook(&HookType::IncomingWebhook),
    }))
}
