use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use garde::Validate;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::{is_unique_violation, AppError, AppResult};

use super::models::{BotApp, BotAppWithToken, CreateBotAppRequest, UpdateBotAppRequest};
use super::repo::{BotAppChanges, NewBotApp};
use super::{generate_token, hash_token, Caller};
use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::authz;
use crate::dto::{DataList, StatusResponse};
use crate::middleware::AuthUser;
use crate::state::AppState;
use crate::workspace::models::WorkspaceRole;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(list_bot_apps, create_bot_app))
        .routes(routes!(this_bot_app))
        .routes(routes!(get_bot_app, update_bot_app, remove_bot_app))
        .routes(routes!(rotate_bot_token))
}

fn name_taken(e: sqlx::Error) -> AppError {
    if is_unique_violation(&e) {
        AppError::Conflict("This workspace already has an app with that name".into())
    } else {
        e.into()
    }
}

async fn find_app(state: &AppState, app_id: Uuid) -> AppResult<BotApp> {
    state
        .bot_repo
        .find(app_id)
        .await?
        .ok_or_else(|| AppError::NotFound("App not found".into()))
}

async fn find_app_as_admin(state: &AppState, app_id: Uuid, user_id: Uuid) -> AppResult<BotApp> {
    let app = find_app(state, app_id).await?;
    authz::require_workspace_role(state, app.workspace_id, user_id, &WorkspaceRole::Admin).await?;
    Ok(app)
}

#[utoipa::path(get, path = "/workspaces/{ws_id}/bot-apps", tag = "bot-apps",
    responses((status = 200, body = DataList<BotApp>)))]
async fn list_bot_apps(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<DataList<BotApp>>> {
    authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    Ok(Json(state.bot_repo.list(ws_id).await?.into()))
}

#[utoipa::path(post, path = "/workspaces/{ws_id}/bot-apps", tag = "bot-apps",
    request_body = CreateBotAppRequest,
    responses((status = 200, body = BotAppWithToken)))]
async fn create_bot_app(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(ws_id): Path<Uuid>,
    Json(req): Json<CreateBotAppRequest>,
) -> AppResult<Json<BotAppWithToken>> {
    req.validate()?;
    authz::require_workspace_role(&state, ws_id, auth.user_id, &WorkspaceRole::Admin).await?;

    let token = generate_token();
    let token_hash = hash_token(&token);
    let mut tx = state.pool.begin().await?;
    let app = state
        .bot_repo
        .create_in(
            &mut tx,
            NewBotApp {
                workspace_id: ws_id,
                created_by: auth.user_id,
                name: req.name.trim(),
                description: req.description.as_deref().filter(|d| !d.trim().is_empty()),
                icon_url: req.icon_url.as_deref().filter(|u| !u.is_empty()),
                scopes: &req.scopes,
                token_hash: &token_hash,
            },
        )
        .await
        .map_err(name_taken)?;
    tx.commit().await?;

    audit::record(
        &state,
        AuditEntry::new(AuditAction::BotAppCreated, auth.user_id)
            .workspace(ws_id)
            .resource(app.id)
            .ip(&ip)
            .details(serde_json::json!({ "name": app.name, "scopes": app.scopes })),
    )
    .await;

    Ok(Json(BotAppWithToken { app, token }))
}

#[utoipa::path(get, path = "/bot-apps/me", tags = ["bot-apps", "bot"],
    security(("bot" = [])),
    responses((status = 200, body = BotApp)))]
async fn this_bot_app(
    State(state): State<Arc<AppState>>,
    caller: Caller,
) -> AppResult<Json<BotApp>> {
    let bot = caller
        .bot()
        .ok_or_else(|| AppError::Forbidden("Only a bot token has an app".into()))?;
    Ok(Json(find_app(&state, bot.app_id).await?))
}

#[utoipa::path(get, path = "/bot-apps/{app_id}", tag = "bot-apps",
    responses((status = 200, body = BotApp)))]
async fn get_bot_app(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(app_id): Path<Uuid>,
) -> AppResult<Json<BotApp>> {
    let app = find_app(&state, app_id).await?;
    authz::require_workspace_member(&state, app.workspace_id, auth.user_id).await?;
    Ok(Json(app))
}

#[utoipa::path(patch, path = "/bot-apps/{app_id}", tag = "bot-apps",
    request_body = UpdateBotAppRequest,
    responses((status = 200, body = BotApp)))]
async fn update_bot_app(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(app_id): Path<Uuid>,
    Json(req): Json<UpdateBotAppRequest>,
) -> AppResult<Json<BotApp>> {
    req.validate()?;
    find_app_as_admin(&state, app_id, auth.user_id).await?;

    let app = state
        .bot_repo
        .update(
            app_id,
            BotAppChanges {
                name: req.name.as_deref().map(str::trim),
                description: req.description.as_deref().map(str::trim),
                icon_url: req.icon_url.as_deref(),
                scopes: req.scopes.as_deref(),
            },
        )
        .await
        .map_err(name_taken)?
        .ok_or_else(|| AppError::NotFound("App not found".into()))?;

    audit::record(
        &state,
        AuditEntry::new(AuditAction::BotAppUpdated, auth.user_id)
            .workspace(app.workspace_id)
            .resource(app.id)
            .ip(&ip)
            .details(serde_json::json!({ "name": app.name, "scopes": app.scopes })),
    )
    .await;

    Ok(Json(app))
}

#[utoipa::path(post, path = "/bot-apps/{app_id}/rotate", tag = "bot-apps",
    responses((status = 200, body = BotAppWithToken)))]
async fn rotate_bot_token(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(app_id): Path<Uuid>,
) -> AppResult<Json<BotAppWithToken>> {
    let app = find_app_as_admin(&state, app_id, auth.user_id).await?;

    let token = generate_token();
    if !state.bot_repo.rotate(app_id, &hash_token(&token)).await? {
        return Err(AppError::NotFound("App not found".into()));
    }

    audit::record(
        &state,
        AuditEntry::new(AuditAction::BotAppTokenRotated, auth.user_id)
            .workspace(app.workspace_id)
            .resource(app.id)
            .ip(&ip),
    )
    .await;

    Ok(Json(BotAppWithToken { app, token }))
}

#[utoipa::path(delete, path = "/bot-apps/{app_id}", tag = "bot-apps",
    responses((status = 200, body = StatusResponse)))]
async fn remove_bot_app(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(app_id): Path<Uuid>,
) -> AppResult<Json<StatusResponse>> {
    let app = find_app_as_admin(&state, app_id, auth.user_id).await?;

    let mut tx = state.pool.begin().await?;
    if !state.bot_repo.remove_in(&mut tx, app_id).await? {
        return Err(AppError::NotFound("App not found".into()));
    }
    state
        .workspace_service
        .repo
        .remove_member_in(&mut tx, app.workspace_id, app.bot_user_id)
        .await?;
    tx.commit().await?;

    audit::record(
        &state,
        AuditEntry::new(AuditAction::BotAppRemoved, auth.user_id)
            .workspace(app.workspace_id)
            .resource(app.id)
            .ip(&ip)
            .details(serde_json::json!({ "name": app.name })),
    )
    .await;

    Ok(Json(StatusResponse::new("deleted")))
}
