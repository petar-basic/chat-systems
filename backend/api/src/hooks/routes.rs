use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::{delete, get, post};
use axum::{middleware, Json, Router};
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::*;
use super::repo::NewReminder;
use crate::middleware::{auth_middleware, AuthUser};
use crate::state::AppState;
use crate::workspace::models::WorkspaceRole;

pub fn router(state: Arc<AppState>) -> Router {
    let routes = Router::new()
        .route("/workspaces/:ws_id/hooks", get(list_hooks))
        .route("/workspaces/:ws_id/hooks", post(create_hook))
        .route("/hooks/:hook_id", get(get_hook))
        .route("/hooks/:hook_id", delete(delete_hook))
        .route("/workspaces/:ws_id/reminders", get(list_reminders))
        .route("/workspaces/:ws_id/reminders", post(create_reminder))
        .layer(middleware::from_fn(auth_middleware));

    Router::new().merge(routes).with_state(state)
}

async fn list_hooks(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    require_ws_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
    let hooks = state
        .hook_repo
        .list_hooks(ws_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
    let mut data = serde_json::to_value(&hooks).map_err(|e| AppError::Internal(e.to_string()))?;
    if let Some(arr) = data.as_array_mut() {
        for hook in arr.iter_mut() {
            redact_secrets(hook.get_mut("config"));
        }
    }
    Ok(Json(serde_json::json!({ "data": data })))
}

async fn create_hook(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<CreateHookRequest>,
) -> AppResult<Json<Hook>> {
    require_ws_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;
    let config = req.config.unwrap_or(serde_json::json!({}));
    let hook = state
        .hook_repo
        .create_hook(
            ws_id,
            auth.user_id,
            &req.hook_type,
            &req.name,
            req.description.as_deref(),
            &config,
        )
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(Json(hook))
}

async fn get_hook(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(hook_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let hook = state
        .hook_repo
        .find_hook_by_id(hook_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("Hook not found".into()))?;
    require_ws_role(
        &state,
        hook.workspace_id,
        auth.user_id,
        &WorkspaceRole::Admin,
    )
    .await?;
    let mut value = serde_json::to_value(&hook).map_err(|e| AppError::Internal(e.to_string()))?;
    redact_secrets(value.get_mut("config"));
    Ok(Json(value))
}

async fn delete_hook(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(hook_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let hook = state
        .hook_repo
        .find_hook_by_id(hook_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("Hook not found".into()))?;
    require_ws_role(
        &state,
        hook.workspace_id,
        auth.user_id,
        &WorkspaceRole::Admin,
    )
    .await?;
    state
        .hook_repo
        .delete_hook(hook_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(Json(serde_json::json!({ "status": "deleted" })))
}

async fn list_reminders(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    require_ws_role(&state, ws_id, auth.user_id, &WorkspaceRole::Member).await?;
    let reminders = state
        .hook_repo
        .list_reminders(ws_id, auth.user_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(Json(serde_json::json!({ "data": reminders })))
}

async fn create_reminder(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<CreateReminderRequest>,
) -> AppResult<Json<Reminder>> {
    let member = require_ws_role(&state, ws_id, auth.user_id, &WorkspaceRole::Member).await?;
    if req.target_user_id != auth.user_id {
        if !member.role.has_at_least(&WorkspaceRole::Admin) {
            return Err(AppError::Forbidden(
                "Cannot create reminders for other users".into(),
            ));
        }
        state
            .workspace_service
            .repo
            .get_member(ws_id, req.target_user_id)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
            .ok_or_else(|| {
                AppError::Forbidden("Target user is not a member of this workspace".into())
            })?;
    }
    let reminder = state
        .hook_repo
        .create_reminder(NewReminder {
            workspace_id: ws_id,
            created_by: auth.user_id,
            target_user_id: req.target_user_id,
            channel_id: req.channel_id,
            message_id: req.message_id,
            content: &req.content,
            remind_at: req.remind_at,
        })
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(Json(reminder))
}

async fn require_ws_role(
    state: &AppState,
    ws_id: Uuid,
    user_id: Uuid,
    min: &WorkspaceRole,
) -> AppResult<crate::workspace::models::WorkspaceMember> {
    let member = state
        .workspace_service
        .repo
        .get_member(ws_id, user_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| AppError::Forbidden("Not a member of this workspace".into()))?;
    if !member.role.has_at_least(min) {
        return Err(AppError::Forbidden(format!(
            "Requires at least {:?} role",
            min
        )));
    }
    Ok(member)
}

fn redact_secrets(value: Option<&mut serde_json::Value>) {
    if let Some(v) = value {
        redact_value(v);
    }
}

fn redact_value(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::Object(map) => {
            for (key, val) in map.iter_mut() {
                if is_secret_key(key) {
                    *val = serde_json::Value::String("***".to_string());
                } else {
                    redact_value(val);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr.iter_mut() {
                redact_value(item);
            }
        }
        _ => {}
    }
}

fn is_secret_key(key: &str) -> bool {
    let lk = key.to_lowercase();
    [
        "secret",
        "token",
        "password",
        "apikey",
        "api_key",
        "credential",
        "bearer",
        "authorization",
    ]
    .iter()
    .any(|needle| lk.contains(needle))
        || lk == "key"
}
