use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::{get, patch};
use axum::{Json, Router};
use uuid::Uuid;

use shared_common::errors::AppResult;

use super::repo::UpdateRetentionRequest;
use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::authz;
use crate::middleware::AuthUser;
use crate::state::AppState;
use crate::workspace::models::WorkspaceRole;

pub fn router(state: Arc<AppState>) -> Router {
    let routes = Router::new()
        .route("/workspaces/{ws_id}/retention", get(get_policy))
        .route("/workspaces/{ws_id}/retention", patch(update_policy));

    crate::protected(state, routes)
}

async fn get_policy(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
    let policy = state.retention_repo.get(ws_id).await?;
    Ok(Json(serde_json::json!({ "policy": policy })))
}

/// Owner-only, and audited with both ends of the change. Shortening retention
/// destroys data on the next nightly pass and there is no undo, so the record of
/// who asked for it matters more than usual.
async fn update_policy(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<UpdateRetentionRequest>,
) -> AppResult<Json<serde_json::Value>> {
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Owner).await?;

    let before = state.retention_repo.get(ws_id).await?;
    let policy = state
        .retention_repo
        .upsert(ws_id, &req, auth.user_id)
        .await?;

    audit::record(
        &state,
        AuditEntry::new(AuditAction::RetentionChanged, auth.user_id)
            .workspace(ws_id)
            .resource(ws_id)
            .ip(&ip)
            .details(serde_json::json!({
                "from": before,
                "to": policy,
            })),
    )
    .await;

    Ok(Json(serde_json::json!({ "policy": policy })))
}
