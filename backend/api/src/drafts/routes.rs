use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use garde::Validate;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::{Draft, SaveDraftRequest};
use super::repo::DraftWrite;
use crate::authz;
use crate::dto::DataList;
use crate::middleware::AuthUser;
use crate::state::AppState;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new().routes(routes!(list_drafts, save_draft))
}

pub async fn announce(state: &AppState, user_id: Uuid, draft: &Draft) {
    let payload = serde_json::json!({
        "user_id": user_id,
        "workspace_id": draft.workspace_id,
        "draft": draft,
    });
    if let Err(e) = state.publisher.publish("draft.updated", payload).await {
        tracing::warn!("failed to announce a draft change: {e}");
    }
}

#[utoipa::path(get, path = "/workspaces/{ws_id}/drafts", tag = "drafts", responses((status = 200, body = DataList<Draft>)))]
async fn list_drafts(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<DataList<Draft>>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    let drafts = state.draft_repo.list(auth.user_id, ws_id).await?;
    Ok(Json(DataList { data: drafts }))
}

#[utoipa::path(put, path = "/workspaces/{ws_id}/drafts", tag = "drafts", request_body = SaveDraftRequest, responses((status = 200, body = Draft)))]
async fn save_draft(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<SaveDraftRequest>,
) -> AppResult<Json<Draft>> {
    req.validate()?;
    require_place(
        &state,
        ws_id,
        auth.user_id,
        req.channel_id,
        req.thread_parent_id,
    )
    .await?;
    let content = if req.content.trim().is_empty() {
        ""
    } else {
        req.content.as_str()
    };
    let draft = state
        .draft_repo
        .save_if_newer(DraftWrite {
            workspace_id: ws_id,
            user_id: auth.user_id,
            channel_id: req.channel_id,
            thread_parent_id: req.thread_parent_id,
            content,
            files_left_behind: req.files_left_behind && !content.is_empty(),
            updated_at: req.updated_at,
        })
        .await?;
    announce(&state, auth.user_id, &draft).await;
    Ok(Json(draft))
}

async fn require_place(
    state: &AppState,
    ws_id: Uuid,
    user_id: Uuid,
    channel_id: Uuid,
    thread_parent_id: Option<Uuid>,
) -> AppResult<()> {
    let unknown = || AppError::NotFound("Not found".into());
    let access = authz::require_channel_access(state, channel_id, user_id)
        .await
        .map_err(|e| match e {
            AppError::Forbidden(_) | AppError::NotFound(_) => unknown(),
            other => other,
        })?;
    if access.channel.workspace_id != ws_id || access.channel_member.is_none() {
        return Err(unknown());
    }
    if let Some(root_id) = thread_parent_id {
        state
            .message_repo
            .find_root(root_id)
            .await?
            .filter(|root| root.channel_id == channel_id && root.thread_parent_id.is_none())
            .ok_or_else(unknown)?;
    }
    Ok(())
}
