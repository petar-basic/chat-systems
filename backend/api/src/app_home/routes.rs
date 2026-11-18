use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde_json::json;
use tracing::warn;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};
use shared_events::Event;

use super::models::{AppHome, HomeOpenedRequest, HomeTab, PublishHomeRequest, StoredHome};
use super::repo::NewHome;
use crate::app_events::{self, repo::NewDelivery};
use crate::audit::ClientIp;
use crate::authz;
use crate::blocks;
use crate::bots::models::{AppEvent, BotApp};
use crate::bots::Caller;
use crate::dto::StatusResponse;
use crate::interactions::models::ActionRequest;
use crate::interactions::{
    action, answered, exchange, find_element, is_interactive, new_hash, person, record,
    NOT_INTERACTIVE, TRIGGER_TTL_SECS,
};
use crate::middleware::AuthUser;
use crate::state::AppState;
use crate::workspace::models::ChannelType;

const NO_HOME: &str = "This app has no Home tab";

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(get_app_home))
        .routes(routes!(app_home_opened))
        .routes(routes!(act_in_app_home))
        .routes(routes!(publish_app_home))
}

async fn home_app(state: &AppState, app_id: Uuid, user_id: Uuid) -> AppResult<BotApp> {
    let app = state
        .bot_repo
        .find(app_id)
        .await?
        .filter(|app| app.home_enabled && app.disabled_at.is_none())
        .ok_or_else(|| AppError::NotFound(NO_HOME.into()))?;
    authz::require_workspace_member(state, app.workspace_id, user_id).await?;
    Ok(app)
}

fn read_home(stored: StoredHome) -> AppResult<AppHome> {
    Ok(AppHome {
        id: stored.id,
        app_id: stored.app_id,
        hash: stored.hash,
        view: serde_json::from_value(stored.view).map_err(|e| AppError::Internal(e.to_string()))?,
    })
}

async fn own_home(state: &AppState, app_id: Uuid, user_id: Uuid) -> AppResult<Option<AppHome>> {
    state
        .app_home_repo
        .find(app_id, user_id)
        .await?
        .map(read_home)
        .transpose()
}

async fn announce(state: &AppState, workspace_id: Uuid, user_id: Uuid, home: &AppHome) {
    let payload = json!({
        "user_id": user_id,
        "workspace_id": workspace_id,
        "app_id": home.app_id,
        "home": home,
    });
    if let Err(e) = state.publisher.publish("views.home_changed", payload).await {
        warn!(%user_id, "failed to send a Home tab: {e}");
    }
}

/// Your Home tab in a conversation with an app: what the app last published
/// for you, and nothing until it has.
#[utoipa::path(get, path = "/apps/{app_id}/home", tag = "interactions",
    responses((status = 200, body = HomeTab)))]
async fn get_app_home(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(app_id): Path<Uuid>,
) -> AppResult<Json<HomeTab>> {
    let app = home_app(&state, app_id, auth.user_id).await?;
    Ok(Json(HomeTab {
        app_id: app.id,
        home: own_home(&state, app.id, auth.user_id).await?,
    }))
}

/// Somebody opened the Home tab in their conversation with the app. An app
/// that subscribes to `app_home_opened` hears about it, with the Home it last
/// published for them, and can publish a fresh one.
#[utoipa::path(post, path = "/apps/{app_id}/home/opened", tag = "interactions",
    request_body = HomeOpenedRequest,
    responses((status = 200, body = StatusResponse)))]
async fn app_home_opened(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(app_id): Path<Uuid>,
    Json(req): Json<HomeOpenedRequest>,
) -> AppResult<Json<StatusResponse>> {
    let app = home_app(&state, app_id, auth.user_id).await?;
    let access = authz::require_channel_access(&state, req.channel_id, auth.user_id).await?;
    let with_app = matches!(access.channel.channel_type, ChannelType::Dm)
        && state
            .workspace_service
            .repo
            .get_channel_member(req.channel_id, app.bot_user_id)
            .await?
            .is_some();
    if !with_app {
        return Err(AppError::Validation(
            "channel_id: not your conversation with this app".into(),
        ));
    }
    let listens = app.events.contains(&AppEvent::AppHomeOpened)
        && (app.socket_mode || app.request_url.is_some());
    if !listens {
        return Ok(Json(StatusResponse::new("ok")));
    }

    let home = own_home(&state, app.id, auth.user_id).await?;
    let source = Event::new("app.home_opened", json!({}));
    let body = json!({
        "channel_id": req.channel_id,
        "user_id": auth.user_id,
        "tab": "home",
        "view": home,
    });
    let envelope = app_events::envelope(&source, app.workspace_id, AppEvent::AppHomeOpened, &body);
    state
        .app_event_repo
        .enqueue(NewDelivery {
            app_id: app.id,
            event_id: app_events::event_id(source.id, AppEvent::AppHomeOpened),
            channel_id: req.channel_id,
            kind: app_events::KIND_EVENT,
            body: &envelope,
            ttl_secs: app_events::EVENT_TTL_SECS,
        })
        .await?;
    Ok(Json(StatusResponse::new("queued")))
}

/// A button, menu or date picker in your Home tab. It reaches the app as a
/// `block_actions` payload with the Home and a `trigger_id` that opens a
/// modal; the app has three seconds to acknowledge it.
#[utoipa::path(post, path = "/apps/{app_id}/home/actions", tag = "interactions",
    request_body = ActionRequest,
    responses((status = 200, body = StatusResponse)))]
async fn act_in_app_home(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(app_id): Path<Uuid>,
    Json(req): Json<ActionRequest>,
) -> AppResult<Json<StatusResponse>> {
    let app = home_app(&state, app_id, auth.user_id).await?;
    if !is_interactive(&app) {
        return Err(AppError::Conflict(NOT_INTERACTIVE.into()));
    }
    let home = own_home(&state, app.id, auth.user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("The app has not published this Home".into()))?;
    let element = find_element(&home.view.blocks, &req.block_id, &req.action_id)
        .ok_or_else(|| AppError::NotFound("That element is no longer in the Home".into()))?;
    let chosen = action(
        &state,
        app.workspace_id,
        auth.user_id,
        &req.block_id,
        element,
        req.value.as_deref(),
    )
    .await?;
    let trigger_id = state
        .interaction_repo
        .create_trigger(app.id, auth.user_id, None, None, TRIGGER_TTL_SECS)
        .await?;
    let name = state.interaction_repo.display_name(auth.user_id).await?;
    let mut view = serde_json::to_value(&home).map_err(|e| AppError::Internal(e.to_string()))?;
    view["state"] = json!({ "values": {} });
    let payload = json!({
        "type": "block_actions",
        "app_id": app.id,
        "workspace_id": app.workspace_id,
        "user": person(auth.user_id, name),
        "trigger_id": trigger_id,
        "container": { "type": "view", "view_id": home.id },
        "view": view,
        "actions": [chosen],
    });
    record(
        &state,
        &ip,
        auth.user_id,
        &app,
        json!({ "type": "block_actions", "action_id": req.action_id, "home_id": home.id }),
    )
    .await;
    answered(exchange(&state, &app, &payload).await?)?;
    Ok(Json(StatusResponse::new("ok")))
}

/// Publishes one person's Home tab in their conversation with the app. Each
/// person has their own, which nobody else can read, and publishing again
/// replaces it. With `hash`, a Home that changed since the app last saw it is
/// left alone.
#[utoipa::path(post, path = "/views/publish", tags = ["interactions", "bot"],
    request_body = PublishHomeRequest,
    security(("bot" = [])),
    responses((status = 200, body = AppHome)))]
async fn publish_app_home(
    State(state): State<Arc<AppState>>,
    caller: Caller,
    Json(req): Json<PublishHomeRequest>,
) -> AppResult<Json<AppHome>> {
    let app_id = caller
        .bot()
        .map(|bot| bot.app_id)
        .ok_or_else(|| AppError::Forbidden("Only an app's bot token publishes a Home".into()))?;
    let app = state
        .bot_repo
        .find(app_id)
        .await?
        .filter(|app| app.disabled_at.is_none())
        .ok_or_else(|| AppError::Forbidden("The app has been turned off".into()))?;
    if !app.home_enabled {
        return Err(AppError::Conflict("This app's Home tab is off".into()));
    }
    state
        .workspace_service
        .repo
        .get_member(app.workspace_id, req.user_id)
        .await?
        .filter(|member| !member.is_bot)
        .ok_or_else(|| AppError::Validation("user_id: not a person in this workspace".into()))?;
    let view = blocks::compose_home(&state, app.workspace_id, &req.view).await?;
    let view = serde_json::to_value(view).map_err(|e| AppError::Internal(e.to_string()))?;
    let stored = state
        .app_home_repo
        .publish(NewHome {
            app_id: app.id,
            workspace_id: app.workspace_id,
            user_id: req.user_id,
            view: &view,
            hash: &new_hash(),
            expected_hash: req.hash.as_deref(),
        })
        .await?
        .ok_or_else(|| {
            AppError::Conflict("hash: this Home has changed since the app last saw it".into())
        })?;
    let home = read_home(stored)?;
    announce(&state, app.workspace_id, req.user_id, &home).await;
    Ok(Json(home))
}
