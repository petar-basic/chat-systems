use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde_json::json;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::{
    AppWorkflowStep, AppWorkflowSteps, CompleteStepRequest, FailStepRequest,
    SaveWorkflowStepsRequest, StepProvider, ERROR_MAX_CHARS,
};
use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::authz;
use crate::bots::models::{BotApp, Scope};
use crate::bots::Caller;
use crate::dto::{DataList, StatusResponse};
use crate::middleware::AuthUser;
use crate::state::AppState;
use crate::workflows::service::{require_builder, settle_app_step};
use crate::workspace::models::WorkspaceRole;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(list_app_workflow_steps, save_app_workflow_steps))
        .routes(routes!(save_own_workflow_steps))
        .routes(routes!(workflow_step_apps))
        .routes(routes!(complete_workflow_step))
        .routes(routes!(fail_workflow_step))
}

async fn app_as_admin(state: &AppState, app_id: Uuid, user_id: Uuid) -> AppResult<BotApp> {
    let app = state
        .bot_repo
        .find(app_id)
        .await?
        .ok_or_else(|| AppError::NotFound("App not found".into()))?;
    authz::require_workspace_role(state, app.workspace_id, user_id, &WorkspaceRole::Admin).await?;
    Ok(app)
}

fn bot_app(caller: &Caller, what: &str) -> AppResult<Uuid> {
    caller
        .bot()
        .map(|bot| bot.app_id)
        .ok_or_else(|| AppError::Forbidden(format!("Only an app's bot token {what}")))
}

fn listed(rows: Vec<super::models::AppWorkflowStepRow>) -> AppWorkflowSteps {
    AppWorkflowSteps {
        steps: rows.into_iter().map(AppWorkflowStep::from).collect(),
    }
}

async fn replace(
    state: &AppState,
    app: &BotApp,
    actor: Uuid,
    ip: &ClientIp,
    req: &SaveWorkflowStepsRequest,
) -> AppResult<AppWorkflowSteps> {
    req.check()?;
    let saved = state
        .app_workflow_step_repo
        .replace(app.id, &req.steps)
        .await?;
    audit::record(
        state,
        AuditEntry::new(AuditAction::BotAppUpdated, actor)
            .workspace(app.workspace_id)
            .resource(app.id)
            .ip(ip)
            .details(json!({
                "workflow_steps": saved.iter().map(|s| s.callback_id.as_str()).collect::<Vec<_>>(),
            })),
    )
    .await;
    Ok(listed(saved))
}

#[utoipa::path(get, path = "/bot-apps/{app_id}/workflow-steps", tag = "bot-apps",
    responses((status = 200, body = AppWorkflowSteps)))]
async fn list_app_workflow_steps(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(app_id): Path<Uuid>,
) -> AppResult<Json<AppWorkflowSteps>> {
    let app = app_as_admin(&state, app_id, auth.user_id).await?;
    Ok(Json(listed(
        state.app_workflow_step_repo.list(app.id).await?,
    )))
}

/// Replaces the workflow steps an app offers. A step keeps its id while its
/// `callback_id` stays the same; a workflow that uses a removed step fails
/// when it gets there.
#[utoipa::path(put, path = "/bot-apps/{app_id}/workflow-steps", tag = "bot-apps",
    request_body = SaveWorkflowStepsRequest,
    responses((status = 200, body = AppWorkflowSteps)))]
async fn save_app_workflow_steps(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(app_id): Path<Uuid>,
    Json(req): Json<SaveWorkflowStepsRequest>,
) -> AppResult<Json<AppWorkflowSteps>> {
    let app = app_as_admin(&state, app_id, auth.user_id).await?;
    Ok(Json(replace(&state, &app, auth.user_id, &ip, &req).await?))
}

/// The app replaces the workflow steps it offers, as an admin would on its
/// page.
#[utoipa::path(put, path = "/bot-apps/me/workflow-steps", tags = ["bot-apps", "bot"],
    request_body = SaveWorkflowStepsRequest,
    security(("bot" = ["workflow.steps:execute"])),
    responses((status = 200, body = AppWorkflowSteps)))]
async fn save_own_workflow_steps(
    State(state): State<Arc<AppState>>,
    caller: Caller,
    ip: ClientIp,
    Json(req): Json<SaveWorkflowStepsRequest>,
) -> AppResult<Json<AppWorkflowSteps>> {
    let app_id = bot_app(&caller, "defines workflow steps")?;
    let auth = caller.scoped(Scope::WorkflowStepsExecute)?;
    let app = state
        .bot_repo
        .find(app_id)
        .await?
        .ok_or_else(|| AppError::NotFound("App not found".into()))?;
    Ok(Json(replace(&state, &app, auth.user_id, &ip, &req).await?))
}

/// The apps of the workspace that offer workflow steps, with their steps.
/// An app with a `problem` can't run them now, and the builder warns about
/// workflows that use them.
#[utoipa::path(get, path = "/workspaces/{ws_id}/workflow-step-apps", tag = "workflows",
    responses((status = 200, body = DataList<StepProvider>)))]
async fn workflow_step_apps(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<DataList<StepProvider>>> {
    require_builder(&state, ws_id, auth.user_id).await?;
    Ok(Json(super::providers(&state, ws_id).await?.into()))
}

/// The app finishes a step that `function_executed` asked it to run, with a
/// value for each output the step declares. The workflow then goes on.
#[utoipa::path(post, path = "/workflow-executions/{execution_id}/complete",
    tags = ["workflows", "bot"],
    request_body = CompleteStepRequest,
    security(("bot" = ["workflow.steps:execute"])),
    responses((status = 200, body = StatusResponse)))]
async fn complete_workflow_step(
    State(state): State<Arc<AppState>>,
    caller: Caller,
    Path(execution_id): Path<Uuid>,
    Json(req): Json<CompleteStepRequest>,
) -> AppResult<Json<StatusResponse>> {
    let app_id = bot_app(&caller, "finishes a workflow step")?;
    caller.scoped(Scope::WorkflowStepsExecute)?;
    settle_app_step(&state, app_id, execution_id, Ok(&req.outputs)).await?;
    Ok(Json(StatusResponse::new("ok")))
}

/// The app fails a step that `function_executed` asked it to run. Its
/// message shows in the run history, and the steps after it don't run.
#[utoipa::path(post, path = "/workflow-executions/{execution_id}/fail",
    tags = ["workflows", "bot"],
    request_body = FailStepRequest,
    security(("bot" = ["workflow.steps:execute"])),
    responses((status = 200, body = StatusResponse)))]
async fn fail_workflow_step(
    State(state): State<Arc<AppState>>,
    caller: Caller,
    Path(execution_id): Path<Uuid>,
    Json(req): Json<FailStepRequest>,
) -> AppResult<Json<StatusResponse>> {
    let app_id = bot_app(&caller, "fails a workflow step")?;
    caller.scoped(Scope::WorkflowStepsExecute)?;
    let error = req.error.trim();
    if error.is_empty() {
        return Err(AppError::Validation(
            "error: say why the step failed".into(),
        ));
    }
    if error.chars().count() > ERROR_MAX_CHARS {
        return Err(AppError::Validation(format!(
            "error: at most {ERROR_MAX_CHARS} characters"
        )));
    }
    settle_app_step(&state, app_id, execution_id, Err(error)).await?;
    Ok(Json(StatusResponse::new("ok")))
}
