use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::ChannelContext;
use crate::authz;
use crate::middleware::AuthUser;
use crate::state::AppState;
use crate::{lists, workflows};

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new().routes(routes!(channel_context))
}

#[utoipa::path(get, path = "/channels/{ch_id}/context", tag = "channels",
    responses((status = 200, body = ChannelContext)))]
async fn channel_context(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ch_id): Path<Uuid>,
) -> AppResult<Json<ChannelContext>> {
    let access = authz::require_channel_access(&state, ch_id, auth.user_id).await?;
    let repo = &state.workspace_service.repo;
    let (members, pins, bookmarks, lists, workflow_shortcuts) = tokio::try_join!(
        async { Ok::<_, AppError>(repo.list_channel_members(ch_id).await?) },
        async { Ok(state.message_repo.list_pinned(ch_id).await?) },
        async { Ok(repo.list_channel_bookmarks(ch_id).await?) },
        lists::service::in_channel(&state, &access),
        async {
            if access.can_post() {
                workflows::service::in_channel(&state, ch_id).await
            } else {
                Ok(Vec::new())
            }
        },
    )?;
    Ok(Json(ChannelContext {
        workspace_id: access.channel.workspace_id,
        members,
        pins,
        bookmarks,
        lists,
        workflow_shortcuts,
    }))
}
