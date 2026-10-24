use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::*;
use super::service;
use crate::audit::ClientIp;
use crate::dto::{DataList, StatusResponse};
use crate::middleware::{AuthUser, PeerAddr};
use crate::rate_limit::{self, LimiterFailure};
use crate::state::AppState;

pub const WEBHOOK_CALLS_PER_MINUTE: u64 = 30;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(list_workflows, create_workflow))
        .routes(routes!(get_workflow, update_workflow, delete_workflow))
        .routes(routes!(publish_workflow))
        .routes(routes!(disable_workflow))
        .routes(routes!(workflow_runs))
}

pub fn public_router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new().routes(routes!(workflow_webhook))
}

#[utoipa::path(get, path = "/workspaces/{ws_id}/workflows", tag = "workflows", responses((status = 200, body = DataList<Workflow>)))]
async fn list_workflows(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<DataList<Workflow>>> {
    let data = service::list(&state, ws_id, auth.user_id).await?;
    Ok(Json(DataList { data }))
}

#[utoipa::path(post, path = "/workspaces/{ws_id}/workflows", tag = "workflows", request_body = SaveWorkflowRequest, responses((status = 200, body = Workflow)))]
async fn create_workflow(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<SaveWorkflowRequest>,
) -> AppResult<Json<Workflow>> {
    Ok(Json(
        service::create(&state, ws_id, auth.user_id, &ip, &req).await?,
    ))
}

#[utoipa::path(get, path = "/workflows/{workflow_id}", tag = "workflows", responses((status = 200, body = Workflow)))]
async fn get_workflow(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(workflow_id): Path<Uuid>,
) -> AppResult<Json<Workflow>> {
    Ok(Json(service::get(&state, workflow_id, auth.user_id).await?))
}

#[utoipa::path(put, path = "/workflows/{workflow_id}", tag = "workflows", request_body = SaveWorkflowRequest, responses((status = 200, body = Workflow)))]
async fn update_workflow(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(workflow_id): Path<Uuid>,
    Json(req): Json<SaveWorkflowRequest>,
) -> AppResult<Json<Workflow>> {
    Ok(Json(
        service::update(&state, workflow_id, auth.user_id, &ip, &req).await?,
    ))
}

#[utoipa::path(delete, path = "/workflows/{workflow_id}", tag = "workflows", responses((status = 200, body = StatusResponse)))]
async fn delete_workflow(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(workflow_id): Path<Uuid>,
) -> AppResult<Json<StatusResponse>> {
    service::delete(&state, workflow_id, auth.user_id, &ip).await?;
    Ok(Json(StatusResponse::new("deleted")))
}

#[utoipa::path(post, path = "/workflows/{workflow_id}/publish", tag = "workflows", responses((status = 200, body = Workflow)))]
async fn publish_workflow(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(workflow_id): Path<Uuid>,
) -> AppResult<Json<Workflow>> {
    Ok(Json(
        service::publish(&state, workflow_id, auth.user_id, &ip).await?,
    ))
}

#[utoipa::path(post, path = "/workflows/{workflow_id}/disable", tag = "workflows", responses((status = 200, body = Workflow)))]
async fn disable_workflow(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(workflow_id): Path<Uuid>,
) -> AppResult<Json<Workflow>> {
    Ok(Json(
        service::disable(&state, workflow_id, auth.user_id, &ip).await?,
    ))
}

#[utoipa::path(get, path = "/workflows/{workflow_id}/runs", tag = "workflows", responses((status = 200, body = DataList<WorkflowRun>)))]
async fn workflow_runs(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(workflow_id): Path<Uuid>,
) -> AppResult<Json<DataList<WorkflowRun>>> {
    let data = service::runs(&state, workflow_id, auth.user_id).await?;
    Ok(Json(DataList { data }))
}

#[utoipa::path(post, path = "/workflows/webhook/{token}", tag = "workflows", request_body = serde_json::Value, responses((status = 202, body = RunAccepted)), security(()))]
async fn workflow_webhook(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
    PeerAddr(peer): PeerAddr,
    headers: HeaderMap,
    body: Option<Json<serde_json::Value>>,
) -> AppResult<(axum::http::StatusCode, Json<RunAccepted>)> {
    let mut conn = state.redis.clone();
    if let Some(ip) = crate::net::client_ip(&headers, peer, &state.config.trusted_proxies) {
        rate_limit::enforce(
            &mut conn,
            &format!("rate_limit:hook_ip:{ip}"),
            120,
            60,
            LimiterFailure::Open,
        )
        .await?;
    }

    let workflow = state
        .workflow_repo
        .find_by_token(&token)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Invalid webhook token".into()))?;
    if !workflow.enabled {
        return Err(AppError::Conflict("This workflow is turned off".into()));
    }
    let Some(Trigger::Webhook { variables }) = workflow.definition().map(|d| d.trigger) else {
        return Err(AppError::Unauthorized("Invalid webhook token".into()));
    };

    rate_limit::enforce(
        &mut conn,
        &format!("rate_limit:workflow:{}", workflow.id),
        WEBHOOK_CALLS_PER_MINUTE,
        60,
        LimiterFailure::Open,
    )
    .await?;

    let sent = body.map(|Json(value)| value).unwrap_or_default();
    let mut values = serde_json::Map::new();
    for name in variables {
        let value = match &sent[&name] {
            serde_json::Value::String(text) => text.clone(),
            serde_json::Value::Null => String::new(),
            other => other.to_string(),
        };
        if value.chars().count() > TEXT_MAX_CHARS {
            return Err(AppError::Validation(format!(
                "\"{name}\" can be at most {TEXT_MAX_CHARS} characters"
            )));
        }
        values.insert(name, serde_json::Value::String(value));
    }

    let request_id = headers
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|key| !key.is_empty() && key.len() <= 200)
        .map(str::to_string)
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let run_id = state
        .workflow_repo
        .queue_run(
            workflow.id,
            workflow.version,
            &format!("webhook:{request_id}"),
            &serde_json::json!({ "webhook": values }),
        )
        .await?;

    Ok((
        axum::http::StatusCode::ACCEPTED,
        Json(RunAccepted {
            status: "accepted",
            run_id,
        }),
    ))
}
