use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::Json;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::repo::{RetentionPolicy, UpdateRetentionRequest};
use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::authz;
use crate::middleware::AuthUser;
use crate::state::AppState;
use crate::workspace::models::WorkspaceRole;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(get_policy, update_policy))
        .routes(routes!(preview_policy))
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct PolicyResponse {
    pub policy: Option<RetentionPolicy>,
}

#[derive(Debug, serde::Deserialize, utoipa::IntoParams)]
pub struct PreviewQuery {
    pub message_days: Option<i32>,
    pub file_days: Option<i32>,
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct RetentionPreview {
    pub messages: i64,
    pub files: i64,
}

/// What the next pass would remove under the periods asked about, counted by
/// the queries the purge itself uses. Shown before the owner saves.
#[utoipa::path(get, path = "/workspaces/{ws_id}/retention/preview", tag = "retention", params(PreviewQuery), responses((status = 200, body = RetentionPreview)))]
async fn preview_policy(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Query(query): Query<PreviewQuery>,
) -> AppResult<Json<RetentionPreview>> {
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Owner).await?;
    if [query.message_days, query.file_days]
        .into_iter()
        .flatten()
        .any(|days| days < 1)
    {
        return Err(AppError::BadRequest(
            "A retention period is at least one day".into(),
        ));
    }
    let messages = match query.message_days {
        Some(days) => {
            state
                .retention_repo
                .count_messages_past(ws_id, days)
                .await?
        }
        None => 0,
    };
    let files = match query.file_days {
        Some(days) => state.retention_repo.count_files_past(ws_id, days).await?,
        None => 0,
    };
    Ok(Json(RetentionPreview { messages, files }))
}

#[utoipa::path(get, path = "/workspaces/{ws_id}/retention", tag = "retention", responses((status = 200, body = PolicyResponse)))]
async fn get_policy(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<PolicyResponse>> {
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
    let policy = state.retention_repo.get(ws_id).await?;
    Ok(Json(PolicyResponse { policy }))
}

/// Owner-only, and audited with both ends of the change. Shortening retention
/// destroys data on the next nightly pass and there is no undo, so the record of
/// who asked for it matters more than usual.
#[utoipa::path(patch, path = "/workspaces/{ws_id}/retention", tag = "retention", request_body = UpdateRetentionRequest, responses((status = 200, body = PolicyResponse)))]
async fn update_policy(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<UpdateRetentionRequest>,
) -> AppResult<Json<PolicyResponse>> {
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Owner).await?;

    let before = state.retention_repo.get(ws_id).await?;
    let policy = state
        .retention_repo
        .upsert(ws_id, &req, auth.user_id)
        .await?;
    let policy = Some(policy);

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

    Ok(Json(PolicyResponse { policy }))
}
