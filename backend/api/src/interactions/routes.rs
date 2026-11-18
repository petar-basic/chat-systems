use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use garde::Validate;
use serde_json::{json, Value};
use tracing::warn;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::models::{
    ActionRequest, ActiveApp, AppView, CloseViewRequest, InteractionAnswer, OpenViewRequest,
    StoredView, SubmitOutcome, SubmitViewRequest, Submitted, Trigger, UpdateAppViewRequest,
    ViewActionRequest,
};
use super::repo::MAX_STACK;
use super::{
    action, announce, answered, app_view, app_views, exchange, find_element, interaction_of,
    interactive_app, is_interactive, message_blocks, message_body, new_hash, person, read_view,
    record, response_url, state_values, view_payload, NOT_INTERACTIVE, RESPONSE_URL_TTL_SECS,
    RESPONSE_URL_USES, TRIGGER_TTL_SECS,
};
use crate::app_shortcuts::models::AppShortcut;
use crate::audit::ClientIp;
use crate::authz;
use crate::blocks::{self, models::Composed, models::Layout};
use crate::bots::{models::BotApp, Caller};
use crate::commands::delivery;
use crate::dto::{DataList, StatusResponse};
use crate::messaging::models::Message;
use crate::messaging::routes::remove_message;
use crate::middleware::AuthUser;
use crate::state::AppState;
use crate::threads::routes::announce_reply;
use crate::workspace::models::WorkspaceRole;

pub fn router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(act_on_message))
        .routes(routes!(act_in_app_view))
        .routes(routes!(submit_app_view))
        .routes(routes!(close_app_view))
        .routes(routes!(list_active_apps))
        .routes(routes!(open_app_view))
        .routes(routes!(push_app_view))
        .routes(routes!(update_app_view))
}

pub fn public_router() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new().routes(routes!(answer_interaction))
}

async fn own_view(state: &AppState, view_id: Uuid, user_id: Uuid) -> AppResult<StoredView> {
    state
        .interaction_repo
        .find_view(view_id)
        .await?
        .filter(|view| view.user_id == user_id)
        .ok_or_else(|| AppError::NotFound("View not found".into()))
}

/// Somebody used a button, menu or date picker in a message. It reaches the
/// app that posted the message, if that app has turned interactivity on, as a
/// `block_actions` payload; the app has three seconds to acknowledge it.
#[utoipa::path(post, path = "/messages/{msg_id}/actions", tag = "interactions",
    request_body = ActionRequest,
    responses((status = 200, body = StatusResponse)))]
async fn act_on_message(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(msg_id): Path<Uuid>,
    Json(req): Json<ActionRequest>,
) -> AppResult<Json<StatusResponse>> {
    let message = state
        .message_repo
        .find_by_id(msg_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Message not found".into()))?;
    let access = authz::require_channel_access(&state, message.channel_id, auth.user_id).await?;
    let app_id = state
        .interaction_repo
        .app_of_message(msg_id)
        .await?
        .ok_or_else(|| AppError::Conflict(NOT_INTERACTIVE.into()))?;
    let app = interactive_app(&state, app_id).await?;
    let layout = Layout::of(&message.metadata);
    let element = find_element(message_blocks(&layout), &req.block_id, &req.action_id)
        .ok_or_else(|| AppError::NotFound("That element is no longer in the message".into()))?;
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
    let interaction_id = Uuid::new_v4();
    state
        .interaction_repo
        .create_interaction(
            interaction_id,
            app.id,
            auth.user_id,
            msg_id,
            RESPONSE_URL_USES,
            RESPONSE_URL_TTL_SECS,
        )
        .await?;
    let name = state.interaction_repo.display_name(auth.user_id).await?;
    let payload = json!({
        "type": "block_actions",
        "app_id": app.id,
        "workspace_id": app.workspace_id,
        "user": person(auth.user_id, name),
        "trigger_id": trigger_id,
        "container": {
            "type": "message",
            "message_id": msg_id,
            "channel_id": message.channel_id,
        },
        "channel": { "id": message.channel_id, "name": access.channel.name },
        "message": message_body(&message, &layout),
        "response_url": response_url(&state, interaction_id),
        "actions": [chosen],
    });
    record(
        &state,
        &ip,
        auth.user_id,
        &app,
        json!({ "type": "block_actions", "action_id": req.action_id, "message_id": msg_id }),
    )
    .await;
    answered(exchange(&state, &app, &payload).await?)?;
    Ok(Json(StatusResponse::new("ok")))
}

/// A button, menu or date picker inside a view. The payload carries the view
/// with what its inputs hold now, and a `trigger_id` that can push a view.
#[utoipa::path(post, path = "/views/{view_id}/actions", tag = "interactions",
    request_body = ViewActionRequest,
    responses((status = 200, body = StatusResponse)))]
async fn act_in_app_view(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(view_id): Path<Uuid>,
    Json(req): Json<ViewActionRequest>,
) -> AppResult<Json<StatusResponse>> {
    let stored = own_view(&state, view_id, auth.user_id).await?;
    let app = interactive_app(&state, stored.app_id).await?;
    let view = app_view(&state, &stored).await?;
    let element = find_element(&view.view.blocks, &req.block_id, &req.action_id)
        .ok_or_else(|| AppError::NotFound("That element is no longer in the view".into()))?;
    let chosen = action(
        &state,
        stored.workspace_id,
        auth.user_id,
        &req.block_id,
        element,
        req.value.as_deref(),
    )
    .await?;
    let (view_state, _) = state_values(
        &state,
        &view.view,
        stored.workspace_id,
        auth.user_id,
        &req.values,
    )
    .await;
    let trigger_id = state
        .interaction_repo
        .create_trigger(
            app.id,
            auth.user_id,
            Some(stored.stack_id),
            None,
            TRIGGER_TTL_SECS,
        )
        .await?;
    let name = state.interaction_repo.display_name(auth.user_id).await?;
    let payload = json!({
        "type": "block_actions",
        "app_id": app.id,
        "workspace_id": app.workspace_id,
        "user": person(auth.user_id, name),
        "trigger_id": trigger_id,
        "container": { "type": "view", "view_id": view_id },
        "view": view_payload(&view, view_state),
        "actions": [chosen],
    });
    record(
        &state,
        &ip,
        auth.user_id,
        &app,
        json!({ "type": "block_actions", "action_id": req.action_id, "view_id": view_id }),
    )
    .await;
    answered(exchange(&state, &app, &payload).await?)?;
    Ok(Json(StatusResponse::new("ok")))
}

async fn answered_view(state: &AppState, workspace_id: Uuid, answer: &Value) -> AppResult<Value> {
    let raw = answer
        .get("view")
        .ok_or_else(|| AppError::BadRequest("The app answered without a view".into()))?;
    let view = blocks::compose_view(state, workspace_id, raw)
        .await
        .map_err(|e| match e {
            AppError::Validation(why) => {
                AppError::BadRequest(format!("The app answered with {why}"))
            }
            other => other,
        })?;
    serde_json::to_value(view).map_err(|e| AppError::Internal(e.to_string()))
}

async fn follow_submission(
    state: &AppState,
    stored: &StoredView,
    answer: &Value,
) -> AppResult<SubmitOutcome> {
    let done = |outcome| SubmitOutcome {
        outcome,
        errors: HashMap::new(),
    };
    match answer.get("response_action").and_then(Value::as_str) {
        Some("errors") => Ok(SubmitOutcome {
            outcome: Submitted::Errors,
            errors: answer
                .get("errors")
                .and_then(Value::as_object)
                .map(|errors| {
                    errors
                        .iter()
                        .filter_map(|(block_id, why)| {
                            Some((block_id.clone(), why.as_str()?.to_string()))
                        })
                        .collect()
                })
                .unwrap_or_default(),
        }),
        Some("update") => {
            let view = answered_view(state, stored.workspace_id, answer).await?;
            state
                .interaction_repo
                .replace(stored.id, &view, &new_hash(), None)
                .await?
                .ok_or_else(|| AppError::NotFound("View not found".into()))?;
            Ok(done(Submitted::Updated))
        }
        Some("push") => {
            let view = answered_view(state, stored.workspace_id, answer).await?;
            state
                .interaction_repo
                .push(stored.stack_id, &view, &new_hash())
                .await?
                .ok_or_else(|| {
                    AppError::BadRequest(format!(
                        "The app answered with one view too many; at most {MAX_STACK} are stacked"
                    ))
                })?;
            Ok(done(Submitted::Pushed))
        }
        Some("clear") => {
            state
                .interaction_repo
                .close_from(stored.stack_id, 0)
                .await?;
            Ok(done(Submitted::Cleared))
        }
        _ => {
            state
                .interaction_repo
                .close_from(stored.stack_id, stored.position)
                .await?;
            Ok(done(Submitted::Closed))
        }
    }
}

/// Submits the view on top of the stack. A required input left empty is
/// refused here without asking the app; otherwise the app's answer decides:
/// nothing closes the view, `errors` shows its messages under the inputs,
/// `update` and `push` change the stack, and `clear` closes all of it.
#[utoipa::path(post, path = "/views/{view_id}/submit", tag = "interactions",
    request_body = SubmitViewRequest,
    responses((status = 200, body = SubmitOutcome)))]
async fn submit_app_view(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(view_id): Path<Uuid>,
    Json(req): Json<SubmitViewRequest>,
) -> AppResult<Json<SubmitOutcome>> {
    let stored = own_view(&state, view_id, auth.user_id).await?;
    let stack = state.interaction_repo.stack(stored.stack_id).await?;
    if stack.last().map(|top| top.id) != Some(view_id) {
        return Err(AppError::Conflict(
            "Only the view on top can be submitted".into(),
        ));
    }
    let app = interactive_app(&state, stored.app_id).await?;
    let views = app_views(&stack)?;
    let view = views
        .last()
        .ok_or_else(|| AppError::NotFound("View not found".into()))?;
    let (view_state, errors) = state_values(
        &state,
        &view.view,
        stored.workspace_id,
        auth.user_id,
        &req.values,
    )
    .await;
    if !errors.is_empty() {
        return Ok(Json(SubmitOutcome {
            outcome: Submitted::Errors,
            errors,
        }));
    }
    let trigger_id = state
        .interaction_repo
        .create_trigger(
            app.id,
            auth.user_id,
            Some(stored.stack_id),
            None,
            TRIGGER_TTL_SECS,
        )
        .await?;
    let name = state.interaction_repo.display_name(auth.user_id).await?;
    let payload = json!({
        "type": "view_submission",
        "app_id": app.id,
        "workspace_id": app.workspace_id,
        "user": person(auth.user_id, name),
        "trigger_id": trigger_id,
        "view": view_payload(view, view_state),
    });
    record(
        &state,
        &ip,
        auth.user_id,
        &app,
        json!({
            "type": "view_submission",
            "callback_id": view.view.callback_id,
            "view_id": view_id,
        }),
    )
    .await;
    let answer = answered(exchange(&state, &app, &payload).await?)?.unwrap_or(Value::Null);
    let outcome = follow_submission(&state, &stored, &answer).await?;
    announce(&state, auth.user_id, stored.workspace_id, stored.stack_id).await;
    Ok(Json(outcome))
}

/// Goes back one view, or with `all` closes the stack. When the view asked
/// for it with `notify_on_close`, the app is told with `view_closed`, but
/// nobody waits for its answer.
#[utoipa::path(post, path = "/views/{view_id}/close", tag = "interactions",
    request_body = CloseViewRequest,
    responses((status = 200, body = StatusResponse)))]
async fn close_app_view(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    ip: ClientIp,
    Path(view_id): Path<Uuid>,
    Json(req): Json<CloseViewRequest>,
) -> AppResult<Json<StatusResponse>> {
    let stored = own_view(&state, view_id, auth.user_id).await?;
    let view = read_view(&stored)?;
    let cleared = req.all || view.clear_on_close;
    let closing = if view.notify_on_close {
        Some(app_view(&state, &stored).await?)
    } else {
        None
    };
    let from = if cleared { 0 } else { stored.position };
    state
        .interaction_repo
        .close_from(stored.stack_id, from)
        .await?;
    announce(&state, auth.user_id, stored.workspace_id, stored.stack_id).await;

    let app = state
        .bot_repo
        .find(stored.app_id)
        .await?
        .filter(is_interactive);
    if let (Some(closing), Some(app)) = (closing, app) {
        let name = state.interaction_repo.display_name(auth.user_id).await?;
        let payload = json!({
            "type": "view_closed",
            "app_id": app.id,
            "workspace_id": app.workspace_id,
            "user": person(auth.user_id, name),
            "view": view_payload(&closing, json!({ "values": {} })),
            "is_cleared": cleared,
        });
        record(
            &state,
            &ip,
            auth.user_id,
            &app,
            json!({
                "type": "view_closed",
                "callback_id": closing.view.callback_id,
                "view_id": view_id,
            }),
        )
        .await;
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(e) = exchange(&state, &app, &payload).await {
                warn!(app_id = %app.id, "failed to send view_closed: {e}");
            }
        });
    }
    Ok(Json(StatusResponse::new("closed")))
}

/// The workspace's apps that are on: which of them draw live buttons and
/// menus in their messages (`interactive`), which have a Home tab, and their
/// global shortcuts. Messages count as an app's when its bot user posted them
/// or they came through one of its hooks or commands.
#[utoipa::path(get, path = "/workspaces/{ws_id}/active-apps", tag = "interactions",
    responses((status = 200, body = DataList<ActiveApp>)))]
async fn list_active_apps(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(ws_id): Path<Uuid>,
) -> AppResult<Json<DataList<ActiveApp>>> {
    let member = authz::require_workspace_member(&state, ws_id, auth.user_id).await?;
    let (rows, shortcuts) = tokio::try_join!(state.interaction_repo.active_apps(ws_id), async {
        if member.role == WorkspaceRole::Guest {
            Ok(Vec::new())
        } else {
            state.app_shortcut_repo.global_in_workspace(ws_id).await
        }
    },)?;
    let mut by_app: HashMap<Uuid, Vec<AppShortcut>> = HashMap::new();
    for row in shortcuts {
        by_app.entry(row.app_id).or_default().push(row.into());
    }
    let apps = rows
        .into_iter()
        .map(|row| ActiveApp {
            shortcuts: by_app.remove(&row.app_id).unwrap_or_default(),
            app_id: row.app_id,
            bot_user_id: row.bot_user_id,
            name: row.name,
            description: row.description,
            icon_url: row.icon_url,
            interactive: row.interactive,
            home: row.home,
            hook_ids: row.hook_ids,
        })
        .collect::<Vec<_>>();
    Ok(Json(apps.into()))
}

fn app_of(caller: &Caller) -> AppResult<Uuid> {
    caller
        .bot()
        .map(|bot| bot.app_id)
        .ok_or_else(|| AppError::Forbidden("Only an app's bot token opens views".into()))
}

async fn spend_trigger(state: &AppState, app_id: Uuid, trigger_id: Uuid) -> AppResult<Trigger> {
    state
        .interaction_repo
        .use_trigger(trigger_id, app_id)
        .await?
        .ok_or_else(|| AppError::Validation("trigger_id: unknown, already used or expired".into()))
}

async fn composed_view(state: &AppState, workspace_id: Uuid, raw: &Value) -> AppResult<Value> {
    let view = blocks::compose_view(state, workspace_id, raw).await?;
    serde_json::to_value(view).map_err(|e| AppError::Internal(e.to_string()))
}

/// Opens a modal for whoever caused `trigger_id`: somebody who used an element
/// or ran one of the app's commands, in the last three seconds. A trigger
/// opens one view.
#[utoipa::path(post, path = "/views/open", tags = ["interactions", "bot"],
    request_body = OpenViewRequest,
    security(("bot" = [])),
    responses((status = 200, body = AppView)))]
async fn open_app_view(
    State(state): State<Arc<AppState>>,
    caller: Caller,
    Json(req): Json<OpenViewRequest>,
) -> AppResult<Json<AppView>> {
    let app = interactive_app(&state, app_of(&caller)?).await?;
    let view = composed_view(&state, app.workspace_id, &req.view).await?;
    let trigger = spend_trigger(&state, app.id, req.trigger_id).await?;
    let opened = state
        .interaction_repo
        .open(
            app.id,
            app.workspace_id,
            trigger.user_id,
            &view,
            &new_hash(),
        )
        .await?;
    announce(&state, trigger.user_id, app.workspace_id, opened.stack_id).await;
    let invocation = match trigger.invocation_id {
        Some(id) => state.command_repo.find(id).await?,
        None => None,
    };
    if let Some(invocation) = invocation {
        delivery::tell(&state, &invocation, "modal", "").await;
    }
    Ok(Json(app_view(&state, &opened).await?))
}

/// Puts a view on top of the open one, with a `trigger_id` from inside it. At
/// most three views are stacked.
#[utoipa::path(post, path = "/views/push", tags = ["interactions", "bot"],
    request_body = OpenViewRequest,
    security(("bot" = [])),
    responses((status = 200, body = AppView)))]
async fn push_app_view(
    State(state): State<Arc<AppState>>,
    caller: Caller,
    Json(req): Json<OpenViewRequest>,
) -> AppResult<Json<AppView>> {
    let app = interactive_app(&state, app_of(&caller)?).await?;
    let view = composed_view(&state, app.workspace_id, &req.view).await?;
    let trigger = spend_trigger(&state, app.id, req.trigger_id).await?;
    let stack_id = trigger.stack_id.ok_or_else(|| {
        AppError::Validation("trigger_id: only a trigger from inside a view can push".into())
    })?;
    let stack = state.interaction_repo.stack(stack_id).await?;
    if stack.first().map(|root| root.app_id) != Some(app.id) {
        return Err(AppError::NotFound("The view has been closed".into()));
    }
    let pushed = state
        .interaction_repo
        .push(stack_id, &view, &new_hash())
        .await?
        .ok_or_else(|| AppError::Validation(format!("At most {MAX_STACK} views are stacked")))?;
    announce(&state, pushed.user_id, pushed.workspace_id, stack_id).await;
    Ok(Json(app_view(&state, &pushed).await?))
}

/// Replaces a view the app opened, found by `view_id` or `external_id`. With
/// `hash`, a view that changed since the app last saw it is left alone.
#[utoipa::path(post, path = "/views/update", tags = ["interactions", "bot"],
    request_body = UpdateAppViewRequest,
    security(("bot" = [])),
    responses((status = 200, body = AppView)))]
async fn update_app_view(
    State(state): State<Arc<AppState>>,
    caller: Caller,
    Json(req): Json<UpdateAppViewRequest>,
) -> AppResult<Json<AppView>> {
    let app = interactive_app(&state, app_of(&caller)?).await?;
    let stored = match (req.view_id, req.external_id.as_deref()) {
        (Some(id), _) => state.interaction_repo.find_view(id).await?,
        (None, Some(external_id)) => {
            state
                .interaction_repo
                .find_external(app.id, external_id)
                .await?
        }
        (None, None) => {
            return Err(AppError::Validation(
                "view_id or external_id is required".into(),
            ))
        }
    }
    .filter(|view| view.app_id == app.id)
    .ok_or_else(|| AppError::NotFound("View not found".into()))?;
    let view = composed_view(&state, app.workspace_id, &req.view).await?;
    let updated = state
        .interaction_repo
        .replace(stored.id, &view, &new_hash(), req.hash.as_deref())
        .await?
        .ok_or_else(|| {
            AppError::Conflict("hash: the view has changed since the app last saw it".into())
        })?;
    announce(
        &state,
        updated.user_id,
        updated.workspace_id,
        updated.stack_id,
    )
    .await;
    Ok(Json(app_view(&state, &updated).await?))
}

async fn replace_message(
    state: &AppState,
    message: &Message,
    workspace_id: Uuid,
    composed: &Composed,
) -> AppResult<()> {
    let mut tx = state.pool.begin().await?;
    state
        .message_repo
        .update_message_in(&mut tx, message.id, &composed.content, message.user_id)
        .await?;
    let updated = state
        .message_repo
        .merge_layout_in(&mut tx, message.id, &composed.metadata().into())
        .await?;
    let staged = state
        .publisher
        .stage_message_updated(&mut tx, &updated, workspace_id)
        .await?;
    tx.commit().await?;
    state.publisher.dispatch(staged).await;
    Ok(())
}

async fn post_as_app(
    state: &AppState,
    app: &BotApp,
    message: &Message,
    workspace_id: Uuid,
    composed: &Composed,
) -> AppResult<()> {
    let metadata = composed.merged_into(json!({}));
    let mut tx = state.pool.begin().await?;
    let posted = state
        .message_repo
        .create_bot_reply_in(
            &mut tx,
            message.channel_id,
            message.thread_parent_id,
            app.bot_user_id,
            &composed.content,
            &metadata,
            &[],
        )
        .await?;
    let staged = state
        .publisher
        .stage_message_created(&mut tx, &posted, workspace_id, &[])
        .await?;
    tx.commit().await?;
    state.publisher.dispatch(staged).await;
    if let Some(root) = message.thread_parent_id {
        announce_reply(state, root, workspace_id).await;
    }
    Ok(())
}

/// A `block_actions` or `message_action` payload's `response_url`: up to five
/// answers within thirty minutes. `replace_original` and `delete_original` act
/// on the message, when it is the app's own; `in_channel` posts a new message
/// as the app, and anything else is shown only to whoever used it.
#[utoipa::path(post, path = "/interactions/responses/{token}", tag = "interactions",
    request_body = InteractionAnswer,
    security(()),
    responses((status = 200, body = StatusResponse)))]
async fn answer_interaction(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
    Json(req): Json<InteractionAnswer>,
) -> AppResult<Json<StatusResponse>> {
    req.validate()?;
    let id = interaction_of(&state, &token)
        .ok_or_else(|| AppError::NotFound("Unknown response URL".into()))?;
    let Some(used) = state.interaction_repo.use_interaction(id).await? else {
        return Err(AppError::NotFound(
            if state.interaction_repo.interaction_exists(id).await? {
                "This response URL has expired or been used up"
            } else {
                "Unknown response URL"
            }
            .into(),
        ));
    };
    let app = state
        .bot_repo
        .find(used.app_id)
        .await?
        .filter(|app| app.disabled_at.is_none())
        .ok_or_else(|| AppError::Forbidden("The app has been turned off".into()))?;
    let message = state
        .message_repo
        .find_by_id(used.message_id)
        .await?
        .ok_or_else(|| AppError::NotFound("The message has been deleted".into()))?;
    let channel = authz::find_channel(&state, message.channel_id).await?;

    let changes_original = req.delete_original || req.replace_original;
    if changes_original && state.interaction_repo.app_of_message(message.id).await? != Some(app.id)
    {
        return Err(AppError::Forbidden(
            "An app changes only its own messages".into(),
        ));
    }

    if req.delete_original {
        authz::require_not_archived(&channel)?;
        remove_message(&state, &message, channel.workspace_id).await?;
        return Ok(Json(StatusResponse::new("deleted")));
    }

    let composed = blocks::compose(
        &state,
        channel.workspace_id,
        blocks::Input {
            text: req.text.as_deref(),
            blocks: req.blocks.as_ref(),
            attachments: req.attachments.as_ref(),
        },
    )
    .await?;
    if composed.content.trim().is_empty() && composed.layout.is_none() {
        return Err(AppError::Validation(
            "An answer needs text, blocks or attachments".into(),
        ));
    }

    if req.replace_original {
        authz::require_not_archived(&channel)?;
        replace_message(&state, &message, channel.workspace_id, &composed).await?;
        return Ok(Json(StatusResponse::new("replaced")));
    }
    if req.response_type.as_deref() == Some("in_channel") {
        authz::require_not_archived(&channel)?;
        post_as_app(&state, &app, &message, channel.workspace_id, &composed).await?;
        return Ok(Json(StatusResponse::new("posted")));
    }
    let shown = json!({
        "user_id": used.user_id,
        "channel_id": message.channel_id,
        "thread_parent_id": message.thread_parent_id,
        "command": app.name,
        "response_type": "ephemeral",
        "text": composed.content,
    });
    state.publisher.publish("command.response", shown).await?;
    Ok(Json(StatusResponse::new("shown")))
}
