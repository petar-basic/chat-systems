use std::collections::HashSet;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use garde::Validate;
use serde_json::{json, Value};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::{
    AppShortcut, RunShortcutRequest, SaveShortcutsRequest, ShortcutKind, MAX_PER_KIND,
};
use crate::audit::{self, AuditAction, AuditEntry, ClientIp};
use crate::authz;
use crate::blocks::models::Layout;
use crate::bots::{announce_apps, models::BotApp};
use crate::dto::{DataList, StatusResponse};
use crate::interactions::{
    answered, exchange, is_interactive, message_body, person, record, response_url,
    NOT_INTERACTIVE, RESPONSE_URL_TTL_SECS, RESPONSE_URL_USES, TRIGGER_TTL_SECS,
};
use crate::middleware::AuthUser;
use crate::state::AppState;
use crate::workspace::models::WorkspaceRole;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(list_app_shortcuts, save_app_shortcuts))
        .routes(routes!(run_app_shortcut))
        .routes(routes!(message_shortcuts))
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

fn shortcuts(rows: Vec<super::models::AppShortcutRow>) -> DataList<AppShortcut> {
    rows.into_iter()
        .map(AppShortcut::from)
        .collect::<Vec<_>>()
        .into()
}

#[utoipa::path(get, path = "/bot-apps/{app_id}/shortcuts", tag = "bot-apps",
    responses((status = 200, body = DataList<AppShortcut>)))]
async fn list_app_shortcuts(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(app_id): Path<Uuid>,
) -> AppResult<Json<DataList<AppShortcut>>> {
    let app = app_as_admin(&state, app_id, auth.user_id).await?;
    Ok(Json(shortcuts(state.app_shortcut_repo.list(app.id).await?)))
}

/// Replaces an app's shortcuts. A shortcut keeps its id while its
/// `callback_id` stays the same.
#[utoipa::path(put, path = "/bot-apps/{app_id}/shortcuts", tag = "bot-apps",
    request_body = SaveShortcutsRequest,
    responses((status = 200, body = DataList<AppShortcut>)))]
async fn save_app_shortcuts(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(app_id): Path<Uuid>,
    Json(req): Json<SaveShortcutsRequest>,
) -> AppResult<Json<DataList<AppShortcut>>> {
    req.validate()?;
    let app = app_as_admin(&state, app_id, auth.user_id).await?;
    for kind in [ShortcutKind::Global, ShortcutKind::Message] {
        if req.shortcuts.iter().filter(|s| s.kind == kind).count() > MAX_PER_KIND {
            return Err(AppError::Validation(format!(
                "An app has at most {MAX_PER_KIND} {} shortcuts",
                kind.as_str()
            )));
        }
    }
    let mut seen = HashSet::new();
    if !req
        .shortcuts
        .iter()
        .all(|s| seen.insert(s.callback_id.as_str()))
    {
        return Err(AppError::Validation(
            "Each shortcut needs its own callback ID".into(),
        ));
    }

    let saved = state
        .app_shortcut_repo
        .replace(app.id, &req.shortcuts)
        .await?;
    audit::record(
        &state,
        AuditEntry::new(AuditAction::BotAppUpdated, auth.user_id)
            .workspace(app.workspace_id)
            .resource(app.id)
            .ip(&ip)
            .details(json!({
                "shortcuts": saved.iter().map(|s| s.callback_id.as_str()).collect::<Vec<_>>(),
            })),
    )
    .await;
    announce_apps(&state, app.workspace_id).await;
    Ok(Json(shortcuts(saved)))
}

/// The message shortcuts of the apps in a conversation; a guest gets none.
#[utoipa::path(get, path = "/channels/{ch_id}/message-shortcuts", tag = "interactions",
    responses((status = 200, body = DataList<AppShortcut>)))]
async fn message_shortcuts(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ch_id): Path<Uuid>,
) -> AppResult<Json<DataList<AppShortcut>>> {
    let access = authz::require_channel_access(&state, ch_id, auth.user_id).await?;
    Ok(Json(super::in_channel(&state, &access).await?.into()))
}

async fn channel_of(state: &AppState, channel_id: Uuid, user_id: Uuid) -> AppResult<Value> {
    let access = authz::require_channel_access(state, channel_id, user_id).await?;
    Ok(json!({ "id": channel_id, "name": access.channel.name }))
}

/// Uses one of an app's shortcuts. A global shortcut reaches the app as a
/// `shortcut` payload, with the conversation it was used from if there was
/// one. A message shortcut, used on a message in a conversation the app is
/// in, reaches it as a `message_action` payload with the message and a
/// `response_url`. Both carry a `trigger_id` that opens a modal, and the app
/// has three seconds to acknowledge them. Guests have no app shortcuts.
#[utoipa::path(post, path = "/app-shortcuts/{shortcut_id}/run", tag = "interactions",
    request_body = RunShortcutRequest,
    responses((status = 200, body = StatusResponse)))]
async fn run_app_shortcut(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(shortcut_id): Path<Uuid>,
    Json(req): Json<RunShortcutRequest>,
) -> AppResult<Json<StatusResponse>> {
    let not_found = || AppError::NotFound("Shortcut not found".into());
    let shortcut = AppShortcut::from(
        state
            .app_shortcut_repo
            .find(shortcut_id)
            .await?
            .ok_or_else(not_found)?,
    );
    let app = state
        .bot_repo
        .find(shortcut.app_id)
        .await?
        .ok_or_else(not_found)?;
    let member = authz::require_workspace_member(&state, app.workspace_id, auth.user_id).await?;
    if member.role == WorkspaceRole::Guest {
        return Err(AppError::Forbidden("Guests have no app shortcuts".into()));
    }
    if !is_interactive(&app) {
        return Err(AppError::Conflict(NOT_INTERACTIVE.into()));
    }

    let name = state.interaction_repo.display_name(auth.user_id).await?;
    let mut payload = json!({
        "type": shortcut.kind.payload_type(),
        "app_id": app.id,
        "workspace_id": app.workspace_id,
        "callback_id": shortcut.callback_id,
        "user": person(auth.user_id, name),
        "action_ts": chrono::Utc::now(),
    });
    let mut details = json!({
        "type": shortcut.kind.payload_type(),
        "callback_id": shortcut.callback_id,
    });
    match shortcut.kind {
        ShortcutKind::Global => {
            payload["channel"] = match req.channel_id {
                Some(channel_id) => channel_of(&state, channel_id, auth.user_id).await?,
                None => Value::Null,
            };
        }
        ShortcutKind::Message => {
            let message_id = req.message_id.ok_or_else(|| {
                AppError::Validation("A message shortcut needs the message it was used on".into())
            })?;
            let message = state
                .message_repo
                .find_by_id(message_id)
                .await?
                .filter(|message| message.deleted_at.is_none())
                .ok_or_else(|| AppError::NotFound("Message not found".into()))?;
            payload["channel"] = channel_of(&state, message.channel_id, auth.user_id).await?;
            let app_in_channel = state
                .workspace_service
                .repo
                .get_channel_member(message.channel_id, app.bot_user_id)
                .await?
                .is_some();
            if !app_in_channel {
                return Err(AppError::Forbidden(
                    "This app is not in that conversation".into(),
                ));
            }
            let interaction_id = Uuid::new_v4();
            state
                .interaction_repo
                .create_interaction(
                    interaction_id,
                    app.id,
                    auth.user_id,
                    message.id,
                    RESPONSE_URL_USES,
                    RESPONSE_URL_TTL_SECS,
                )
                .await?;
            payload["message"] = message_body(&message, &Layout::of(&message.metadata));
            payload["response_url"] = json!(response_url(&state, interaction_id));
            details["message_id"] = json!(message.id);
        }
    }
    let trigger_id = state
        .interaction_repo
        .create_trigger(app.id, auth.user_id, None, None, TRIGGER_TTL_SECS)
        .await?;
    payload["trigger_id"] = json!(trigger_id);
    record(&state, &ip, auth.user_id, &app, details).await;
    answered(exchange(&state, &app, &payload).await?)?;
    Ok(Json(StatusResponse::new("ok")))
}
